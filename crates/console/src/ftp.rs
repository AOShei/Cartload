//! FTP client for bulk transfers (busybox `ftpd -Sw /` on port 21, same credentials as SSH).
//!
//! hakchi2 CE uploads a sync through FTP (`MKD` for every directory, then one `STOR` per
//! file) and fixes modification times with a shell script afterwards. The server does not
//! check the password and serves the root directory, so remote paths are absolute device paths.

use std::{net::SocketAddr, path::Path, time::Duration};

use suppaftp::{FtpError, Mode, Status, tokio::AsyncFtpStream, types::FileType};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tracing::debug;

use crate::{Result, session::PASSWORD, session::USERNAME};

/// FTP port on the console.
pub const FTP_PORT: u16 = 21;

/// Bytes handed to the data connection per write.
const CHUNK: usize = 256 * 1024;

/// Upload progress, in bytes of the current file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Bytes sent so far.
    pub done: u64,
    /// Size of the file.
    pub total: u64,
}

/// An FTP control connection to the console.
pub struct Ftp {
    stream: AsyncFtpStream,
    address: SocketAddr,
}

impl std::fmt::Debug for Ftp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ftp")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// Every prefix of an absolute path, shortest first, for `mkdir -p` style creation:
/// `/a/b/c` gives `/a`, `/a/b`, `/a/b/c`.
pub fn path_prefixes(path: &str) -> Vec<String> {
    let mut prefixes = Vec::new();
    let mut current = String::new();
    for component in path.split('/').filter(|c| !c.is_empty()) {
        current.push('/');
        current.push_str(component);
        prefixes.push(current.clone());
    }
    prefixes
}

impl Ftp {
    /// Connects and logs in as `root` with an empty password.
    pub async fn connect(address: SocketAddr, connect_timeout: Duration) -> Result<Self> {
        Self::connect_as(address, USERNAME, PASSWORD, connect_timeout).await
    }

    /// Connects and logs in with the given credentials, binary mode, passive transfers.
    pub async fn connect_as(
        address: SocketAddr,
        user: &str,
        password: &str,
        connect_timeout: Duration,
    ) -> Result<Self> {
        let mut stream = AsyncFtpStream::connect_timeout(address, connect_timeout).await?;
        stream.login(user, password).await?;
        stream.transfer_type(FileType::Binary).await?;
        stream.set_mode(Mode::Passive);
        debug!(%address, welcome = ?stream.get_welcome_msg(), "FTP session established");
        Ok(Self { stream, address })
    }

    /// Address of the server.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The server's welcome line.
    pub fn welcome(&self) -> Option<&str> {
        self.stream.get_welcome_msg()
    }

    /// Creates a directory. Returns `false` if it already existed (reply 550), like hakchi2 CE
    /// which ignores that reply.
    pub async fn mkdir(&mut self, path: &str) -> Result<bool> {
        match self.stream.mkdir(path).await {
            Ok(()) => Ok(true),
            Err(FtpError::UnexpectedResponse(reply)) if reply.status == Status::FileUnavailable => {
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Creates a directory and all its parents.
    pub async fn mkdir_p(&mut self, path: &str) -> Result<()> {
        for prefix in path_prefixes(path) {
            self.mkdir(&prefix).await?;
        }
        Ok(())
    }

    /// Uploads `data` to `remote_path`, reporting progress after every chunk.
    pub async fn upload(
        &mut self,
        remote_path: &str,
        data: &[u8],
        progress: impl FnMut(Progress),
    ) -> Result<u64> {
        self.upload_reader(remote_path, data, data.len() as u64, progress)
            .await
    }

    /// Uploads everything `reader` yields to `remote_path`. `total` is only used for the
    /// progress reports.
    pub async fn upload_reader<R: AsyncRead + Unpin>(
        &mut self,
        remote_path: &str,
        mut reader: R,
        total: u64,
        mut progress: impl FnMut(Progress),
    ) -> Result<u64> {
        let mut transfer = self.stream.put_with_stream(remote_path).await?;
        let mut buffer = vec![0u8; CHUNK];
        let mut done = 0u64;
        progress(Progress { done, total });
        loop {
            let read = reader.read(&mut buffer).await?;
            if read == 0 {
                break;
            }
            transfer.write_all(&buffer[..read]).await?;
            done += read as u64;
            progress(Progress { done, total });
        }
        transfer.flush().await?;
        transfer.finish().await?;
        debug!(remote_path, bytes = done, "uploaded");
        Ok(done)
    }

    /// Uploads a local file to `remote_path`.
    pub async fn upload_file(
        &mut self,
        local: &Path,
        remote_path: &str,
        progress: impl FnMut(Progress),
    ) -> Result<u64> {
        let file = tokio::fs::File::open(local).await?;
        let total = file.metadata().await?.len();
        self.upload_reader(remote_path, file, total, progress).await
    }

    /// `LIST`: long-format directory listing lines.
    pub async fn list(&mut self, path: Option<&str>) -> Result<Vec<String>> {
        Ok(self.stream.list(path).await?)
    }

    /// `NLST`: file names only.
    pub async fn names(&mut self, path: Option<&str>) -> Result<Vec<String>> {
        Ok(self.stream.nlst(path).await?)
    }

    /// `SIZE` of a remote file.
    pub async fn size(&mut self, path: &str) -> Result<u64> {
        Ok(self.stream.size(path).await? as u64)
    }

    /// Changes the working directory.
    pub async fn cwd(&mut self, path: &str) -> Result<()> {
        Ok(self.stream.cwd(path).await?)
    }

    /// Current working directory.
    pub async fn pwd(&mut self) -> Result<String> {
        Ok(self.stream.pwd().await?)
    }

    /// Renames a remote file (`RNFR` / `RNTO`), replacing `to` if it exists.
    pub async fn rename(&mut self, from: &str, to: &str) -> Result<()> {
        Ok(self.stream.rename(from, to).await?)
    }

    /// Deletes a remote file.
    pub async fn remove_file(&mut self, path: &str) -> Result<()> {
        Ok(self.stream.rm(path).await?)
    }

    /// Removes an empty remote directory.
    pub async fn remove_dir(&mut self, path: &str) -> Result<()> {
        Ok(self.stream.rmdir(path).await?)
    }

    /// Sends `QUIT` and closes the connection.
    pub async fn quit(mut self) -> Result<()> {
        Ok(self.stream.quit().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_cover_every_parent() {
        assert_eq!(
            path_prefixes("/var/lib/hakchi/games/snes-eur"),
            [
                "/var",
                "/var/lib",
                "/var/lib/hakchi",
                "/var/lib/hakchi/games",
                "/var/lib/hakchi/games/snes-eur"
            ]
        );
        assert_eq!(
            path_prefixes("relative/dir/"),
            ["/relative", "/relative/dir"]
        );
        assert!(path_prefixes("/").is_empty());
        assert!(path_prefixes("").is_empty());
    }

    #[test]
    fn progress_is_plain_data() {
        let progress = Progress { done: 1, total: 2 };
        assert_eq!(progress, Progress { done: 1, total: 2 });
    }

    #[tokio::test]
    async fn connect_refused_is_an_error() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("addr");
        drop(listener);
        assert!(Ftp::connect(address, Duration::from_secs(2)).await.is_err());
    }
}
