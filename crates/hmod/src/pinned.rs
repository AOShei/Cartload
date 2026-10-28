//! The one kernel package Cartload installs: hakchi v1.0.4-126 (April 2020), the build hakchi2
//! CE ships and the last hakchi release. Every known copy (CE's GitHub release asset,
//! hakchi.net's `hakchi-latest.hmod`, CE's build output) has the same bytes, checked on
//! 2026-10-28. Pinning it by SHA-256 means a server that disappears, or one that starts serving
//! something else under the same name, cannot change what goes onto a console.
//!
//! With the `bundled` feature the package itself is embedded from `data/hakchi.hmod` at the
//! repository root (origin and license in `data/README.md`), so installing hakchi needs no
//! network at all; [`install_bundled`] writes it out where the path-based [`Package`] reader
//! and the flows can open it.
//!
//! [`Package`]: crate::Package

#[cfg(feature = "bundled")]
use std::path::PathBuf;
use std::{io::Read, path::Path};

use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// `hakchiVersion` of the pinned package.
pub const PINNED_VERSION: &str = "v1.0.4-126";
/// Size of the pinned package in bytes.
pub const PINNED_LEN: u64 = 9_340_382;
/// Lowercase hex SHA-256 of the pinned package.
pub const PINNED_SHA256: &str = "003994b46504959895f40b0b9e91b16951ef4a638feb6312314c8fc4fdbff592";

/// Lowercase hex SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// Lowercase hex SHA-256 of a file, read in chunks.
pub fn file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|source| io_error("open", path, source))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|source| io_error("read", path, source))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Checks that the file at `path` is the pinned package. A file of the wrong size is refused
/// without hashing it.
pub fn check_pinned(path: &Path) -> Result<()> {
    let len = std::fs::metadata(path)
        .map_err(|source| io_error("inspect", path, source))?
        .len();
    let sha256 = if len == PINNED_LEN {
        file_sha256(path)?
    } else {
        String::new()
    };
    if sha256 == PINNED_SHA256 {
        Ok(())
    } else {
        Err(Error::NotPinned {
            path: path.to_path_buf(),
            len,
            sha256,
        })
    }
}

/// The pinned package, embedded in the binary.
#[cfg(feature = "bundled")]
pub const fn bundled() -> &'static [u8] {
    BUNDLED
}

#[cfg(feature = "bundled")]
const BUNDLED: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/hakchi.hmod"
));

/// Makes sure `dest` holds the pinned package, writing the embedded copy when the file is
/// missing or different. Returns whether it wrote. The copy goes to `<dest>.partial` first and
/// is renamed over `dest`, so a reader never sees half a package. Parent directories are
/// created as needed.
#[cfg(feature = "bundled")]
pub fn install_bundled(dest: &Path) -> Result<bool> {
    if dest.is_file() && check_pinned(dest).is_ok() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|source| io_error("create directory", parent, source))?;
    }
    let partial = partial_path(dest)?;
    let written = std::fs::write(&partial, BUNDLED)
        .map_err(|source| io_error("write", &partial, source))
        .and_then(|()| {
            std::fs::rename(&partial, dest).map_err(|source| io_error("rename", &partial, source))
        });
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written.map(|()| true)
}

/// `<dest>.partial` in the same directory, so the final rename stays on one file system.
#[cfg(feature = "bundled")]
fn partial_path(dest: &Path) -> Result<PathBuf> {
    let name = dest.file_name().ok_or_else(|| Error::BadDestination {
        path: dest.to_path_buf(),
    })?;
    let mut partial = name.to_os_string();
    partial.push(".partial");
    Ok(dest.with_file_name(partial))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn io_error(op: &'static str, path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        op,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn sha256_matches_a_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(file_sha256(&path).unwrap(), sha256_hex(b"abc"));
    }

    #[test]
    fn other_files_are_not_the_pinned_package() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hakchi.hmod");
        std::fs::write(&path, crate::test_support::sample()).unwrap();
        let error = check_pinned(&path).unwrap_err();
        assert!(matches!(error, Error::NotPinned { .. }), "{error}");
        assert!(error.to_string().contains(PINNED_VERSION), "{error}");
    }

    #[cfg(feature = "bundled")]
    #[test]
    fn the_bundled_package_is_the_pinned_one() {
        assert_eq!(bundled().len() as u64, PINNED_LEN);
        assert_eq!(sha256_hex(bundled()), PINNED_SHA256);

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("cache").join("hakchi.hmod");
        assert!(install_bundled(&dest).unwrap());
        check_pinned(&dest).unwrap();
        let version = crate::Package::open(&dest).unwrap().validate().unwrap();
        assert_eq!(version.hakchi, PINNED_VERSION);
        assert_eq!(version.boot.as_deref(), Some("1.0.3"));
        assert_eq!(version.kernel.as_deref(), Some("3.4.113.29-madmonkey"));

        // Already in place: left alone. Replaced by something else: written again.
        assert!(!install_bundled(&dest).unwrap());
        std::fs::write(&dest, b"something else").unwrap();
        assert!(install_bundled(&dest).unwrap());
        check_pinned(&dest).unwrap();
        let names: Vec<_> = std::fs::read_dir(dest.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["hakchi.hmod"]);
    }
}
