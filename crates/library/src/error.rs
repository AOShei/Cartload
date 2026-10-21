//! Error type for the crate.

use std::path::PathBuf;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file system operation failed.
    #[error("could not {action} {}: {source}", path.display())]
    Io {
        /// What was attempted (`read`, `write`, `create`, ...).
        action: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The file extension is not one of the ROM or archive types the library imports.
    #[error(
        "{}: unsupported file type; expected .sfc, .smc, .fig, .swc, .nes or a .zip of those",
        .0.display()
    )]
    UnsupportedFile(PathBuf),

    /// A Mega Drive ROM was given; that console is not supported yet.
    #[error(
        "{}: Mega Drive / Genesis games are not supported yet (the Mega Drive Mini uses a \
         different game format and menu layout)",
        .0.display()
    )]
    MegaDriveUnsupported(PathBuf),

    /// The ROM could not be understood.
    #[error("{name}: {reason}")]
    InvalidRom {
        /// File name of the ROM.
        name: String,
        /// What is wrong with it.
        reason: String,
    },

    /// A zip archive could not be read.
    #[error("{}: {source}", path.display())]
    Zip {
        /// The archive.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: zip::result::ZipError,
    },

    /// A 7z archive could not be written or read.
    #[error("7z {name}: {message}")]
    Archive {
        /// The archive or entry name.
        name: String,
        /// What went wrong.
        message: String,
    },

    /// A file is larger than what the console's extractor can handle.
    #[error(
        "{name} is {size} bytes, above the {} bytes hakchi2 CE compresses (tiny7zx holds the \
         whole file in the console's RAM); compression is off for this game",
        crate::archive::MAX_SIZE
    )]
    TooLargeToCompress {
        /// The file.
        name: String,
        /// Its size.
        size: u64,
    },

    /// Compressed games are in the plan but the console has no extractor.
    #[error(
        "{games} compressed game(s) need the tiny7zx mod on the console and `which tiny7zx` \
         found none; install hakchi2 CE's base mods (cartload-flow base-mods) or turn \
         compression off (cartload-library compress --off)"
    )]
    MissingExtractor {
        /// Compressed games in the plan.
        games: usize,
    },

    /// A zip archive contains no ROM.
    #[error("{}: the archive contains no .sfc, .smc, .fig, .swc or .nes file", .0.display())]
    EmptyArchive(PathBuf),

    /// JSON (de)serialisation failed.
    #[error("{}: {source}", path.display())]
    Json {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: serde_json::Error,
    },

    /// The hash database could not be parsed.
    #[error("hash database: {0}")]
    HashDb(String),

    /// A download failed.
    #[cfg(feature = "fetch")]
    #[error("downloading {url}: {source}")]
    Download {
        /// The URL.
        url: String,
        /// The underlying error.
        #[source]
        source: reqwest::Error,
    },

    /// The server answered with an error status.
    #[cfg(feature = "fetch")]
    #[error("downloading {url}: HTTP status {status}")]
    HttpStatus {
        /// The URL.
        url: String,
        /// The status code.
        status: u16,
    },

    /// The console refused or failed a command.
    #[error(transparent)]
    Console(#[from] cartload_console::Error),

    /// The games do not fit in the console's free space.
    #[error(
        "not enough space on the console: the games need {needed} bytes, {available} bytes are \
         available"
    )]
    NotEnoughSpace {
        /// Bytes the collection takes (4 KiB blocks).
        needed: u64,
        /// Bytes the console has for games.
        available: u64,
    },

    /// A game code is malformed.
    #[error("{0:?} is not a game code (CLV-X-XXXXX)")]
    InvalidCode(String),

    /// No game with this code is in the library.
    #[error("no game {0} in the library")]
    NoSuchGame(String),

    /// A menu structure specification could not be parsed.
    #[error("{0}")]
    InvalidStructure(String),

    /// The console type is needed (for the games directory name) but not known.
    #[error("the console type is not known; connect a console or pass --console")]
    UnknownConsole,

    /// An image could not be decoded or encoded.
    #[error("image: {0}")]
    Image(String),

    /// No TheGamesDB API key is available.
    #[error(
        "no TheGamesDB API key: set the {} environment variable or store one with \
         `cartload-library key --set <KEY>`",
        crate::art::KEY_ENV
    )]
    NoApiKey,

    /// TheGamesDB rejected the API key.
    #[error("TheGamesDB rejected the API key (HTTP {status}: {message})")]
    InvalidApiKey {
        /// HTTP status (401 or 403).
        status: u16,
        /// The API's status text.
        message: String,
    },

    /// The monthly request allowance of the API key is used up.
    #[error("TheGamesDB monthly allowance used up{}", match refresh_in_secs {
        Some(secs) => format!("; it refreshes in {}", crate::art::human_duration(*secs)),
        None => String::new(),
    })]
    AllowanceExhausted {
        /// Seconds until the allowance refreshes, when the API said.
        refresh_in_secs: Option<u64>,
    },

    /// TheGamesDB answered with an error or something unexpected.
    #[error("TheGamesDB {endpoint}: HTTP {status}: {message}")]
    Api {
        /// The route (never the key).
        endpoint: String,
        /// The HTTP status.
        status: u16,
        /// The API's status text or what was wrong with the body.
        message: String,
    },

    /// A request failed before an answer came back.
    #[error("{url}: {message}")]
    Transport {
        /// The URL without the key.
        url: String,
        /// What went wrong.
        message: String,
    },

    /// Something in the menu or library is inconsistent.
    #[error("{0}")]
    Invalid(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Builds an [`Error::Io`].
pub(crate) fn io_error(
    action: &'static str,
    path: &std::path::Path,
    source: std::io::Error,
) -> Error {
    Error::Io {
        action,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_path() {
        let error = io_error(
            "read",
            std::path::Path::new("/x/game.json"),
            std::io::Error::other("boom"),
        );
        assert_eq!(error.to_string(), "could not read /x/game.json: boom");
        assert_eq!(
            Error::NoSuchGame("CLV-U-AAAAA".into()).to_string(),
            "no game CLV-U-AAAAA in the library"
        );
    }
}
