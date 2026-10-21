//! SSH session with the console (dropbear on port 22, `root`, empty password).

use std::{
    borrow::Cow,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use russh::{
    ChannelMsg, Disconnect, Preferred,
    client::{self, Handle},
    kex,
    keys::{HashAlg, PublicKeyOrCertificate},
    mac,
};
use tokio::time::timeout;
use tracing::{debug, info};

use crate::{Error, Ftp, Result, Shell, ftp::FTP_PORT, parse::sh_quote};

/// User the hakchi kernel accepts over SSH and FTP.
pub const USERNAME: &str = "root";
/// Password for [`USERNAME`]: empty.
pub const PASSWORD: &str = "";

/// Connection and command settings.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// SSH user.
    pub user: String,
    /// SSH password.
    pub password: String,
    /// Time allowed for the TCP connect, key exchange and authentication, each.
    pub connect_timeout: Duration,
    /// Time allowed for one command; `None` waits forever.
    pub command_timeout: Option<Duration>,
    /// Interval of protocol keepalives on an idle link; three unanswered close the session.
    pub keepalive_interval: Option<Duration>,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            user: USERNAME.to_string(),
            password: PASSWORD.to_string(),
            connect_timeout: Duration::from_secs(5),
            command_timeout: Some(Duration::from_secs(60)),
            keepalive_interval: Some(Duration::from_secs(15)),
        }
    }
}

/// What a remote command produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecOutput {
    /// Exit status, if the server reported one.
    pub exit_code: Option<u32>,
    /// Bytes written to stdout.
    pub stdout: Vec<u8>,
    /// Bytes written to stderr.
    pub stderr: Vec<u8>,
}

impl ExecOutput {
    /// Whether the command exited with status 0.
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// stdout as text (lossy).
    pub fn stdout_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// stderr as text (lossy).
    pub fn stderr_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }

    /// stdout as text with surrounding whitespace removed, like hakchi2 CE's `ExecuteSimple`.
    pub fn stdout_trimmed(&self) -> String {
        self.stdout_lossy().trim().to_string()
    }

    /// stderr as text with surrounding whitespace removed.
    pub fn stderr_trimmed(&self) -> String {
        self.stderr_lossy().trim().to_string()
    }

    /// Turns a non-zero (or missing) exit status into [`Error::CommandFailed`].
    pub fn checked(self, command: &str) -> Result<Self> {
        if self.success() {
            Ok(self)
        } else {
            Err(Error::CommandFailed {
                command: command.to_string(),
                exit_code: self.exit_code,
                stderr: self.stderr_trimmed(),
            })
        }
    }
}

/// The server's host key, as seen during key exchange.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostKey {
    /// Key algorithm (`ecdsa-sha2-nistp256`, `ssh-rsa`, ...).
    pub algorithm: String,
    /// SHA-256 fingerprint.
    pub fingerprint: String,
}

struct Client {
    host_key: Arc<Mutex<Option<HostKey>>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    // The console generates its host key in RAM at every boot (dropbear -R on a ramdisk), so
    // there is nothing to pin. Record what was seen for diagnostics and accept it.
    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let host_key = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => HostKey {
                algorithm: key.algorithm().to_string(),
                fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            },
            PublicKeyOrCertificate::Certificate(certificate) => HostKey {
                algorithm: certificate.algorithm().to_string(),
                fingerprint: certificate
                    .public_key()
                    .fingerprint(HashAlg::Sha256)
                    .to_string(),
            },
        };
        debug!(
            algorithm = host_key.algorithm,
            fingerprint = host_key.fingerprint,
            "server host key"
        );
        if let Ok(mut slot) = self.host_key.lock() {
            *slot = Some(host_key);
        }
        Ok(true)
    }
}

/// An SSH connection to the console.
///
/// Commands run through [`Session::exec`] get their own channel each, so a session can be
/// shared (`&Session`) and commands may run concurrently. Small files go up through
/// [`Session::upload`] (`cat > path` with the bytes on stdin); bulk transfers use the FTP
/// client from [`Session::ftp`].
pub struct Session {
    handle: Handle<Client>,
    address: SocketAddr,
    options: SessionOptions,
    host_key: Arc<Mutex<Option<HostKey>>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("address", &self.address)
            .field("closed", &self.handle.is_closed())
            .finish_non_exhaustive()
    }
}

/// Algorithms offered to the server. russh's defaults plus the SHA-1 based key exchange and
/// MAC that older dropbear builds (the 2018.76 in hakchi v1.0.4 has SHA-2 ones too) may need,
/// at the lowest preference.
fn preferred_algorithms() -> Preferred {
    let mut kex_names = Preferred::DEFAULT.kex.to_vec();
    kex_names.extend([kex::ECDH_SHA2_NISTP256, kex::DH_G14_SHA1]);
    let mut mac_names = Preferred::DEFAULT.mac.to_vec();
    mac_names.push(mac::HMAC_SHA1);
    Preferred {
        kex: Cow::Owned(kex_names),
        mac: Cow::Owned(mac_names),
        ..Preferred::DEFAULT
    }
}

impl Session {
    /// Connects and authenticates as `root` with an empty password, with default options.
    pub async fn connect(address: SocketAddr) -> Result<Self> {
        Self::connect_with(address, SessionOptions::default()).await
    }

    /// Connects and authenticates with the given options.
    pub async fn connect_with(address: SocketAddr, options: SessionOptions) -> Result<Self> {
        let config = Arc::new(client::Config {
            inactivity_timeout: None,
            keepalive_interval: options.keepalive_interval,
            keepalive_max: 3,
            preferred: preferred_algorithms(),
            nodelay: true,
            ..client::Config::default()
        });
        let host_key = Arc::new(Mutex::new(None));
        let client = Client {
            host_key: Arc::clone(&host_key),
        };
        let connect_timeout = options.connect_timeout;
        let timed_out = || Error::ConnectTimeout {
            address,
            timeout: connect_timeout,
        };
        let mut handle = timeout(connect_timeout, client::connect(config, address, client))
            .await
            .map_err(|_| timed_out())??;
        // dropbear grants a blank password through the "none" method (`-B`, allowblankpass)
        // and rejects the same account through the "password" method, so try "none" first
        // and fall back to a real password exchange for servers that want one.
        let auth = timeout(
            connect_timeout,
            handle.authenticate_none(options.user.clone()),
        )
        .await
        .map_err(|_| timed_out())??;
        let auth = if auth.success() {
            debug!("authenticated with the none method (blank password)");
            auth
        } else {
            timeout(
                connect_timeout,
                handle.authenticate_password(options.user.clone(), options.password.clone()),
            )
            .await
            .map_err(|_| timed_out())??
        };
        if !auth.success() {
            let _ = handle
                .disconnect(Disconnect::ByApplication, "authentication failed", "en")
                .await;
            return Err(Error::AuthFailed {
                address,
                user: options.user,
            });
        }
        info!(%address, user = options.user, "SSH session established");
        Ok(Self {
            handle,
            address,
            options,
            host_key,
        })
    }

    /// Address of the console.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Options the session was opened with.
    pub fn options(&self) -> &SessionOptions {
        &self.options
    }

    /// The server's host key, once key exchange has happened.
    pub fn host_key(&self) -> Option<HostKey> {
        self.host_key.lock().ok().and_then(|slot| slot.clone())
    }

    /// Whether the connection has been closed (by either side).
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    /// Runs a shell command and collects its exit status and output.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput> {
        self.exec_with_stdin(command, None).await
    }

    /// Runs a shell command, feeding `stdin` to it (then EOF), and collects the result. With
    /// `None` the command gets an immediate EOF on stdin.
    pub async fn exec_with_stdin(&self, command: &str, stdin: Option<&[u8]>) -> Result<ExecOutput> {
        if self.handle.is_closed() {
            return Err(Error::Closed);
        }
        let run = self.run(command, stdin);
        let output = match self.options.command_timeout {
            Some(limit) => timeout(limit, run)
                .await
                .map_err(|_| Error::CommandTimeout {
                    command: command.to_string(),
                    timeout: limit,
                })??,
            None => run.await?,
        };
        debug!(
            command,
            exit_code = ?output.exit_code,
            stdout = output.stdout.len(),
            stderr = output.stderr.len(),
            "exec"
        );
        Ok(output)
    }

    async fn run(&self, command: &str, stdin: Option<&[u8]>) -> Result<ExecOutput> {
        let channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;
        let (mut reader, writer) = channel.split();
        let send = async {
            if let Some(mut data) = stdin {
                writer.data(&mut data).await?;
            }
            writer.eof().await
        };
        let receive = async {
            let mut output = ExecOutput::default();
            while let Some(message) = reader.wait().await {
                match message {
                    ChannelMsg::Data { data } => output.stdout.extend_from_slice(&data),
                    ChannelMsg::ExtendedData { data, ext: 1 } => {
                        output.stderr.extend_from_slice(&data);
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        output.exit_code = Some(exit_status);
                    }
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            output
        };
        let (sent, output) = tokio::join!(send, receive);
        sent?;
        Ok(output)
    }

    /// Runs a command that must succeed and returns its trimmed stdout (hakchi2 CE's
    /// `ExecuteSimple` with `throwOnNonZero`).
    pub async fn exec_checked(&self, command: &str) -> Result<String> {
        let output = self.exec(command).await?.checked(command)?;
        Ok(output.stdout_trimmed())
    }

    /// Writes `data` to `remote_path` through the shell (`cat > path`). Meant for small files
    /// such as scripts; use [`Session::ftp`] for bulk transfers.
    pub async fn upload(&self, data: &[u8], remote_path: &str) -> Result<()> {
        let command = format!("cat > {}", sh_quote(remote_path));
        self.exec_with_stdin(&command, Some(data))
            .await?
            .checked(&command)?;
        Ok(())
    }

    /// Like [`Session::upload`], then marks the file executable.
    pub async fn upload_executable(&self, data: &[u8], remote_path: &str) -> Result<()> {
        let path = sh_quote(remote_path);
        let command = format!("cat > {path} && chmod +x {path}");
        self.exec_with_stdin(&command, Some(data))
            .await?
            .checked(&command)?;
        Ok(())
    }

    /// Uploads a script to `/tmp/<name>`, runs it and removes it again (hakchi2 CE
    /// `RunTemporaryScript`). Returns the script's result; a non-zero exit is not an error.
    pub async fn run_temporary_script(&self, script: &[u8], name: &str) -> Result<ExecOutput> {
        if name.is_empty() || name.contains('/') || name.contains(char::is_whitespace) {
            return Err(Error::InvalidArgument(format!(
                "temporary script name {name:?} must be a plain file name"
            )));
        }
        let path = sh_quote(&format!("/tmp/{name}"));
        self.exec_with_stdin(&format!("cat > {path}"), Some(script))
            .await?
            .checked(&format!("cat > {path}"))?;
        let result = self.exec(&format!("chmod +x {path} && {path}")).await;
        if let Err(error) = self.exec(&format!("rm -f {path}")).await {
            debug!(%error, "could not remove temporary script");
        }
        result
    }

    /// Opens an FTP connection to the same console for bulk transfers.
    pub async fn ftp(&self) -> Result<Ftp> {
        Ftp::connect_as(
            SocketAddr::new(self.address.ip(), FTP_PORT),
            &self.options.user,
            &self.options.password,
            self.options.connect_timeout,
        )
        .await
    }

    /// Closes the connection.
    pub async fn close(self) -> Result<()> {
        self.handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await?;
        Ok(())
    }
}

impl Shell for Session {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        stdin: Option<&'a [u8]>,
    ) -> impl Future<Output = Result<ExecOutput>> + Send + 'a {
        self.exec_with_stdin(command, stdin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(code: Option<u32>, stdout: &str, stderr: &str) -> ExecOutput {
        ExecOutput {
            exit_code: code,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn exec_output_helpers() {
        let ok = output(Some(0), "  value \n", "");
        assert!(ok.success());
        assert_eq!(ok.stdout_trimmed(), "value");
        assert!(ok.clone().checked("cmd").is_ok());

        let failed = output(Some(1), "", "no such function: nope\n");
        assert!(!failed.success());
        let error = failed.checked("hakchi nope").expect_err("non-zero");
        assert_eq!(
            error.to_string(),
            "command `hakchi nope` failed with exit code 1: no such function: nope"
        );

        let no_status = output(None, "", "");
        assert!(!no_status.success());
        assert!(matches!(
            no_status.checked("x"),
            Err(Error::CommandFailed {
                exit_code: None,
                ..
            })
        ));
    }

    #[test]
    fn defaults_match_the_console() {
        let options = SessionOptions::default();
        assert_eq!(options.user, "root");
        assert_eq!(options.password, "");
        assert!(options.command_timeout.is_some());
    }

    #[test]
    fn preferred_algorithms_keep_defaults_first_and_add_sha1_fallbacks() {
        let preferred = preferred_algorithms();
        assert_eq!(preferred.kex[0], Preferred::DEFAULT.kex[0]);
        assert!(preferred.kex.contains(&kex::CURVE25519_PRE_RFC_8731));
        assert_eq!(preferred.kex.last(), Some(&kex::DH_G14_SHA1));
        assert!(preferred.mac.contains(&mac::HMAC_SHA256));
        assert_eq!(preferred.mac.last(), Some(&mac::HMAC_SHA1));
    }

    #[tokio::test]
    async fn connect_refused_is_an_error_not_a_hang() {
        // Nothing listens on this port of the loopback interface.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("addr");
        drop(listener);
        let result = Session::connect_with(
            address,
            SessionOptions {
                connect_timeout: Duration::from_secs(2),
                ..SessionOptions::default()
            },
        )
        .await;
        assert!(result.is_err());
    }
}
