//! Error type for the crate.

use std::{net::SocketAddr, time::Duration};

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No network interface carries a link-local (169.254.x.x) IPv4 address, so there is no
    /// USB link to probe.
    #[error(
        "no network interface has a 169.254.x.x address, so there is no USB link to the console. \
         Check that the console is running the hakchi kernel (USB 04e8:6863) and that its \
         network interface (hakchi0) has a link-local address (NetworkManager \
         ipv4.method=link-local)"
    )]
    NoLinkLocalInterface,

    /// Nothing answered on any discovery strategy.
    #[error(
        "no console answered at {address} (probed from {probed}). Is it powered on and running \
         the hakchi kernel? A console in FEL mode or on stock firmware does not answer"
    )]
    NotFound {
        /// Address that was probed.
        address: SocketAddr,
        /// Interfaces the probe went out on, or what was tried instead.
        probed: String,
    },

    /// Operating system I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// SSH protocol error.
    #[error("SSH error: {0}")]
    Ssh(#[from] russh::Error),

    /// Connecting or authenticating took too long.
    #[error("connecting to {address} did not complete within {timeout:?}")]
    ConnectTimeout {
        /// Address being connected to.
        address: SocketAddr,
        /// Allowed time.
        timeout: Duration,
    },

    /// The SSH server refused the credentials.
    #[error(
        "SSH authentication as {user:?} with an empty password was refused by {address}; this \
         is not a hakchi kernel"
    )]
    AuthFailed {
        /// Address of the server.
        address: SocketAddr,
        /// User name that was tried.
        user: String,
    },

    /// A remote command ran longer than allowed.
    #[error("command did not finish within {timeout:?}: {command}")]
    CommandTimeout {
        /// The command.
        command: String,
        /// Allowed time.
        timeout: Duration,
    },

    /// A remote command exited with a non-zero status (or without a status).
    #[error("command `{command}` failed with {}{}", exit_code_text(.exit_code), stderr_text(.stderr))]
    CommandFailed {
        /// The command.
        command: String,
        /// Exit code reported by the server, if any.
        exit_code: Option<u32>,
        /// What the command wrote to stderr (trimmed).
        stderr: String,
    },

    /// FTP protocol error.
    #[error("FTP error: {0}")]
    Ftp(#[from] suppaftp::FtpError),

    /// mDNS browsing failed.
    #[error("mDNS error: {0}")]
    Mdns(String),

    /// The console answered something this crate cannot interpret.
    #[error("could not parse {what} from {input:?}")]
    Parse {
        /// What was expected.
        what: &'static str,
        /// What was received.
        input: String,
    },

    /// A caller-supplied value cannot be used safely in a shell command.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// The SSH session has been closed.
    #[error("the SSH session is closed")]
    Closed,
}

fn exit_code_text(code: &Option<u32>) -> String {
    match code {
        Some(code) => format!("exit code {code}"),
        None => "no exit status".to_string(),
    }
}

fn stderr_text(stderr: &str) -> String {
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_failed_mentions_code_and_stderr() {
        let error = Error::CommandFailed {
            command: "hakchi hwid".into(),
            exit_code: Some(1),
            stderr: "boom".into(),
        };
        assert_eq!(
            error.to_string(),
            "command `hakchi hwid` failed with exit code 1: boom"
        );
        let error = Error::CommandFailed {
            command: "true".into(),
            exit_code: None,
            stderr: String::new(),
        };
        assert_eq!(
            error.to_string(),
            "command `true` failed with no exit status"
        );
    }
}
