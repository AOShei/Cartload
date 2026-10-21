//! Error type for the crate.

use std::path::PathBuf;

use crate::EntryKind;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file operation failed.
    #[error("cannot {op} {}: {source}", path.display())]
    Io {
        /// What was attempted (`open`, `create`, `write`, ...).
        op: &'static str,
        /// The file involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The file is not a readable gzip tar archive (wrong format, corrupt or truncated).
    #[error("{} is not a readable gzip tar archive: {source}", path.display())]
    Archive {
        /// The archive.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The archive has no entries at all.
    #[error("{} contains no entries", path.display())]
    Empty {
        /// The archive.
        path: PathBuf,
    },

    /// A requested entry does not exist.
    #[error("{} has no entry {entry}", path.display())]
    MissingEntry {
        /// The archive.
        path: PathBuf,
        /// The normalised entry name.
        entry: String,
    },

    /// A requested entry is not a regular file.
    #[error("{entry} in {} is a {kind}, not a regular file", path.display())]
    NotAFile {
        /// The archive.
        path: PathBuf,
        /// The normalised entry name.
        entry: String,
        /// What the entry is.
        kind: EntryKind,
    },

    /// A required entry is present but empty.
    #[error("{entry} in {} is empty", path.display())]
    EmptyEntry {
        /// The archive.
        path: PathBuf,
        /// The normalised entry name.
        entry: String,
    },

    /// `var/version` is not text or lacks `hakchiVersion`.
    #[error("var/version in {} is invalid: {reason}", path.display())]
    InvalidVersion {
        /// The archive.
        path: PathBuf,
        /// What is wrong with it.
        reason: String,
    },

    /// A base mod's file is not in the cache.
    #[error(
        "base mod {name}: {} is not cached; download it first (cartload-hmod base-mods --fetch)",
        path.display()
    )]
    ModNotCached {
        /// The mod.
        name: String,
        /// The missing file.
        path: PathBuf,
    },

    /// A base mod's file does not have the catalogued size and MD5.
    #[error("base mod {name}: {} should be {expected} but is {actual}", path.display())]
    ModChecksum {
        /// The mod.
        name: String,
        /// The file.
        path: PathBuf,
        /// Expected size and MD5.
        expected: String,
        /// What was found.
        actual: String,
    },

    /// The HTTP client could not be built.
    #[cfg(feature = "fetch")]
    #[error("cannot build the HTTP client: {0}")]
    Client(#[source] reqwest::Error),

    /// A request or body transfer failed. The message includes the cause chain (DNS, connect,
    /// TLS, ...) because reqwest's own top-level message is only "error sending request".
    #[cfg(feature = "fetch")]
    #[error("download from {url} failed: {}", chain(.source))]
    Download {
        /// The URL requested.
        url: String,
        /// The underlying error.
        #[source]
        source: reqwest::Error,
    },

    /// The server answered with an error status.
    #[cfg(feature = "fetch")]
    #[error("download from {url} failed: HTTP status {status}")]
    HttpStatus {
        /// The URL requested.
        url: String,
        /// The status code.
        status: u16,
    },

    /// The body ended before the announced length.
    #[cfg(feature = "fetch")]
    #[error("download from {url} ended after {done} of {total} bytes")]
    Truncated {
        /// The URL requested.
        url: String,
        /// Bytes received.
        done: u64,
        /// Bytes announced by the server.
        total: u64,
    },

    /// The downloaded data is not a usable kernel package.
    #[cfg(feature = "fetch")]
    #[error("download from {url} is not a usable kernel package: {source}")]
    Unusable {
        /// The URL requested.
        url: String,
        /// Why the package was rejected.
        #[source]
        source: Box<Error>,
    },

    /// The destination path has no file name to derive the temporary file name from.
    #[cfg(feature = "fetch")]
    #[error("destination {} has no file name", path.display())]
    BadDestination {
        /// The destination given.
        path: PathBuf,
    },

    /// Every download source failed; the per-source errors are inside.
    #[cfg(feature = "fetch")]
    #[error("all download sources failed: {0}")]
    AllSourcesFailed(Failures),
}

/// An error's message followed by those of its causes, joined with `: `.
#[cfg(feature = "fetch")]
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        let message = next.to_string();
        if !message.is_empty() && !text.ends_with(&message) {
            text.push_str(": ");
            text.push_str(&message);
        }
        cause = next.source();
    }
    text
}

/// The per-source errors behind [`Error::AllSourcesFailed`], in the order the sources were tried.
#[cfg(feature = "fetch")]
#[derive(Debug, Default)]
pub struct Failures(pub Vec<(String, Error)>);

#[cfg(feature = "fetch")]
impl std::fmt::Display for Failures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return f.write_str("no download sources were given");
        }
        for (i, (url, error)) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{url}: {error}")?;
        }
        Ok(())
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
