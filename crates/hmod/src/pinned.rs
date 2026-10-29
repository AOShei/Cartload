//! The kernel packages Cartload installs, each pinned by size and SHA-256 so that no server
//! that disappears, or starts serving something else under the same name, can change what goes
//! onto a console:
//!
//! - [`CARTLOAD`], the default: Cartload's own system, built from source by `scripts/kernel`,
//!   `scripts/userland` and `scripts/uboot` (`docs/userland.md`); its `var/version` names it
//!   with `cartloadVersion`. The committed file is checked against a rebuild by CI.
//! - [`HAKCHI`]: hakchi v1.0.4-126 (April 2020), the build hakchi2 CE ships and the last hakchi
//!   release. Every known copy (CE's GitHub release asset, hakchi.net's `hakchi-latest.hmod`,
//!   CE's build output) has the same bytes, checked on 2026-10-28. The way back to hakchi's
//!   own system, and the only one the download fallback (`fetch`) knows.
//!
//! With the `bundled` feature both are embedded from `data/` at the repository root (origin and
//! license in `data/README.md`), so installing needs no network at all;
//! [`PinnedPackage::install_bundled`] writes one out where the path-based [`Package`] reader and
//! the flows can open it.
//!
//! [`Package`]: crate::Package

#[cfg(feature = "bundled")]
use std::path::PathBuf;
use std::{io::Read, path::Path};

use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// One package Cartload installs.
#[derive(Debug, PartialEq, Eq)]
pub struct PinnedPackage {
    /// Stable identifier, for settings and the frontend: `cartload` or `hakchi`.
    pub id: &'static str,
    /// What the user sees, for example `Cartload system 0.1.0`.
    pub label: &'static str,
    /// `hakchiVersion` in its `var/version`.
    pub hakchi_version: &'static str,
    /// `cartloadVersion` in its `var/version`, for Cartload's own system.
    pub system_version: Option<&'static str>,
    /// Size in bytes.
    pub len: u64,
    /// Lowercase hex SHA-256.
    pub sha256: &'static str,
    /// File name in the cache directory.
    pub cache_file: &'static str,
    #[cfg(feature = "bundled")]
    bytes: &'static [u8],
}

/// Cartload's own system (the default).
pub const CARTLOAD: PinnedPackage = PinnedPackage {
    id: "cartload",
    label: "Cartload system 0.1.0",
    hakchi_version: "v1.0.4-126",
    system_version: Some("0.1.0"),
    len: 8_641_616,
    sha256: "81d6dbadeaf8249911531546813c89782f70a573bc43d752c506858327d2af9e",
    cache_file: "cartload-system-0.1.0.hmod",
    #[cfg(feature = "bundled")]
    bytes: include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/cartload-system-0.1.0.hmod"
    )),
};

/// hakchi v1.0.4-126, the package hakchi2 CE installs.
pub const HAKCHI: PinnedPackage = PinnedPackage {
    id: "hakchi",
    label: "hakchi v1.0.4-126",
    hakchi_version: "v1.0.4-126",
    system_version: None,
    len: 9_340_382,
    sha256: "003994b46504959895f40b0b9e91b16951ef4a638feb6312314c8fc4fdbff592",
    cache_file: "hakchi.hmod",
    #[cfg(feature = "bundled")]
    bytes: include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/hakchi.hmod"
    )),
};

/// Every package, the default first.
pub const PACKAGES: [&PinnedPackage; 2] = [&CARTLOAD, &HAKCHI];

/// `hakchiVersion` of hakchi's package ([`HAKCHI`]), the one the download fallback fetches.
pub const PINNED_VERSION: &str = HAKCHI.hakchi_version;
/// Size of hakchi's package in bytes.
pub const PINNED_LEN: u64 = HAKCHI.len;
/// Lowercase hex SHA-256 of hakchi's package.
pub const PINNED_SHA256: &str = HAKCHI.sha256;

impl PinnedPackage {
    /// The package with this [`id`](Self::id).
    pub fn by_id(id: &str) -> Option<&'static Self> {
        PACKAGES.into_iter().find(|p| p.id == id)
    }

    /// Checks that the file at `path` is this package. A file of the wrong size is refused
    /// without hashing it.
    pub fn check(&self, path: &Path) -> Result<()> {
        let len = std::fs::metadata(path)
            .map_err(|source| io_error("inspect", path, source))?
            .len();
        let sha256 = if len == self.len {
            file_sha256(path)?
        } else {
            String::new()
        };
        if sha256 == self.sha256 {
            Ok(())
        } else {
            Err(Error::NotPinned {
                path: path.to_path_buf(),
                expected: self.label,
                expected_len: self.len,
                expected_sha256: self.sha256,
                len,
                sha256,
            })
        }
    }

    /// The package, embedded in the binary.
    #[cfg(feature = "bundled")]
    pub const fn bundled(&self) -> &'static [u8] {
        self.bytes
    }

    /// Makes sure `dest` holds this package, writing the embedded copy when the file is missing
    /// or different. Returns whether it wrote. The copy goes to `<dest>.partial` first and is
    /// renamed over `dest`, so a reader never sees half a package. Parent directories are
    /// created as needed.
    #[cfg(feature = "bundled")]
    pub fn install_bundled(&self, dest: &Path) -> Result<bool> {
        if dest.is_file() && self.check(dest).is_ok() {
            return Ok(false);
        }
        if let Some(parent) = dest.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|source| io_error("create directory", parent, source))?;
        }
        let partial = partial_path(dest)?;
        let written = std::fs::write(&partial, self.bytes)
            .map_err(|source| io_error("write", &partial, source))
            .and_then(|()| {
                std::fs::rename(&partial, dest)
                    .map_err(|source| io_error("rename", &partial, source))
            });
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        written.map(|()| true)
    }
}

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

/// Checks that the file at `path` is hakchi's package ([`HAKCHI`]).
pub fn check_pinned(path: &Path) -> Result<()> {
    HAKCHI.check(path)
}

/// hakchi's package, embedded in the binary.
#[cfg(feature = "bundled")]
pub const fn bundled() -> &'static [u8] {
    HAKCHI.bundled()
}

/// Makes sure `dest` holds hakchi's package ([`PinnedPackage::install_bundled`]).
#[cfg(feature = "bundled")]
pub fn install_bundled(dest: &Path) -> Result<bool> {
    HAKCHI.install_bundled(dest)
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
        for package in PACKAGES {
            let error = package.check(&path).unwrap_err();
            assert!(matches!(error, Error::NotPinned { .. }), "{error}");
            assert!(error.to_string().contains(package.label), "{error}");
        }
        assert!(check_pinned(&path).is_err());
    }

    #[test]
    fn packages_by_id() {
        assert_eq!(PinnedPackage::by_id("cartload"), Some(&CARTLOAD));
        assert_eq!(PinnedPackage::by_id("hakchi"), Some(&HAKCHI));
        assert_eq!(PinnedPackage::by_id("other"), None);
        assert_eq!(PACKAGES[0], &CARTLOAD);
    }

    #[cfg(feature = "bundled")]
    #[test]
    fn the_bundled_packages_are_the_pinned_ones() {
        let dir = tempfile::tempdir().unwrap();
        for package in PACKAGES {
            assert_eq!(
                package.bundled().len() as u64,
                package.len,
                "{}",
                package.id
            );
            assert_eq!(
                sha256_hex(package.bundled()),
                package.sha256,
                "{}",
                package.id
            );

            let dest = dir.path().join("cache").join(package.cache_file);
            assert!(package.install_bundled(&dest).unwrap());
            package.check(&dest).unwrap();
            let version = crate::Package::open(&dest).unwrap().validate().unwrap();
            assert_eq!(version.hakchi, package.hakchi_version);
            assert_eq!(version.cartload.as_deref(), package.system_version);
            assert_eq!(version.boot.as_deref(), Some("1.0.3"));

            // Already in place: left alone. Replaced by something else: written again.
            assert!(!package.install_bundled(&dest).unwrap());
            std::fs::write(&dest, b"something else").unwrap();
            assert!(package.install_bundled(&dest).unwrap());
            package.check(&dest).unwrap();
        }
        let mut names: Vec<_> = std::fs::read_dir(dir.path().join("cache"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["cartload-system-0.1.0.hmod", "hakchi.hmod"]);
        assert_eq!(bundled(), HAKCHI.bundled());
        assert_eq!(
            crate::Package::open(dir.path().join("cache").join(CARTLOAD.cache_file))
                .unwrap()
                .validate()
                .unwrap()
                .kernel
                .as_deref(),
            Some("3.4.113.30-cartload")
        );
    }
}
