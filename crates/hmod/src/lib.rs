//! hakchi.hmod kernel package parsing and fetching.
//!
//! `hakchi.hmod` is a gzip-compressed tar archive holding the hakchi kernel for the Classic Mini
//! consoles: `boot/boot.img` (kernel plus ramdisk in Android boot format), `boot/uboot.bin`, the
//! device-side tools under `bin/`, the `install` and `uninstall` scripts the console runs,
//! `etc/preinit.d/` and `var/version`, a shell fragment with `bootVersion`, `hakchiVersion` and
//! `kernelVersion`. Entry names in the archive may or may not start with `./`; this crate
//! normalises them so callers always use `boot/boot.img` style names.
//!
//! - [`Package`] opens a package on disk, lists it and reads entries.
//! - [`fetch_latest`] downloads the current package from the known locations (feature `fetch`,
//!   on by default).
//! - [`basemods`] knows hakchi2 CE's base mods (`tiny7zx` for compressed games,
//!   `font_remount`): a catalog with sizes and MD5s, a cache, a downloader (feature `fetch`)
//!   and the tar stream the console's `pack_install` takes.
//! - [`fes1`] returns the DRAM init binary that a FEL memboot runs before anything else. It is
//!   not part of the hmod, so it is embedded from `data/fes1.bin` at the repository root.
//!
//! The protocol knowledge comes from hakchi2 CE (`hakchi.cs`, `MembootTasks.cs`); the code is
//! new. Nothing here needs hardware.

pub mod basemods;
mod error;
#[cfg(feature = "fetch")]
mod fetch;
mod package;
mod version;

#[cfg(feature = "fetch")]
pub use error::Failures;
pub use error::{Error, Result};
#[cfg(feature = "fetch")]
pub use fetch::{
    DOWNLOAD_URLS, Fetched, GITHUB_URL, HAKCHI_NET_URL, Progress, fetch_from, fetch_latest,
};
pub use package::{BOOT_IMAGE, BootFiles, Entry, EntryKind, Package, UBOOT, VERSION_FILE};
pub use version::Version;

/// `fes1.bin`, the Allwinner first-stage binary that initialises DRAM on the R16.
///
/// A FEL memboot uploads it to SRAM and runs it before u-boot and the kernel can be loaded. It
/// is not part of `hakchi.hmod`; this copy is the one shipped in the hakchi2 CE source tree, see
/// `data/README.md` at the repository root for its origin, checksums and license.
pub const fn fes1() -> &'static [u8] {
    FES1
}

const FES1: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/fes1.bin"));

/// Crate name and version, for CLI banners and diagnostics.
pub const fn version() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
}

/// Builders for small synthetic packages used by the unit tests of several modules.
#[cfg(test)]
pub(crate) mod test_support {
    #![allow(clippy::unwrap_used)]

    use std::{io, path::PathBuf};

    use flate2::{Compression, write::GzEncoder};
    use tar::{EntryType, Header};

    /// One entry to put in a synthetic archive. The name is written to the header byte for
    /// byte, so a `./` prefix survives (the `tar` builder would strip it).
    pub(crate) enum Spec<'a> {
        File(&'a str, &'a [u8]),
        Dir(&'a str),
        Symlink(&'a str, &'a str),
    }

    /// Build a gzip tar from the given entries, in order.
    pub(crate) fn archive(specs: &[Spec<'_>]) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for spec in specs {
            let mut header = Header::new_gnu();
            header.set_mtime(1_585_912_351);
            header.set_uid(0);
            header.set_gid(0);
            match spec {
                Spec::File(name, data) => {
                    header.set_entry_type(EntryType::Regular);
                    header.set_mode(0o644);
                    header.set_size(data.len() as u64);
                    set_raw_name(&mut header, name);
                    header.set_cksum();
                    builder.append(&header, *data).unwrap();
                }
                Spec::Dir(name) => {
                    header.set_entry_type(EntryType::Directory);
                    header.set_mode(0o755);
                    header.set_size(0);
                    set_raw_name(&mut header, name);
                    header.set_cksum();
                    builder.append(&header, io::empty()).unwrap();
                }
                Spec::Symlink(name, target) => {
                    header.set_entry_type(EntryType::Symlink);
                    header.set_mode(0o777);
                    header.set_size(0);
                    set_raw_name(&mut header, name);
                    header.set_link_name(target).unwrap();
                    header.set_cksum();
                    builder.append(&header, io::empty()).unwrap();
                }
            }
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn set_raw_name(header: &mut Header, name: &str) {
        let field = &mut header.as_old_mut().name;
        assert!(name.len() < field.len(), "test entry name too long: {name}");
        field.fill(0);
        field[..name.len()].copy_from_slice(name.as_bytes());
    }

    /// The `var/version` text of hakchi v1.0.4-126.
    pub(crate) const VERSION_TEXT: &str =
        "bootVersion='1.0.3'\nhakchiVersion='v1.0.4-126'\nkernelVersion='3.4.113.29-madmonkey'\n";

    /// Synthetic boot image and u-boot contents, clearly distinguishable.
    pub(crate) const BOOT_IMG: &[u8] = b"ANDROID!fake kernel and ramdisk";
    pub(crate) const UBOOT: &[u8] = b"fake u-boot image";

    /// A package shaped like the real `hakchi.hmod`, mixing `./name` and `name` entries, with
    /// directories, a symlink and a root entry.
    pub(crate) fn sample() -> Vec<u8> {
        archive(&[
            Spec::Dir("./"),
            Spec::Dir("bin/"),
            Spec::Symlink("bin/sunxi-flash", "sntool"),
            Spec::File("bin/sntool", b"ELF sntool"),
            Spec::Dir("./boot/"),
            Spec::File("./boot/boot.img", BOOT_IMG),
            Spec::File("boot/uboot.bin", UBOOT),
            Spec::File("install", b"#!/bin/sh\necho install\n"),
            Spec::File("uninstall", b"#!/bin/sh\n"),
            Spec::Dir("var/"),
            Spec::File("./var/version", VERSION_TEXT.as_bytes()),
        ])
    }

    /// Write bytes to a file in a fresh temporary directory; the directory is removed on drop.
    pub(crate) fn temp_file(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_mentions_crate_name() {
        assert!(version().starts_with(env!("CARGO_PKG_NAME")));
    }

    /// Guards the embedded binary against accidental corruption: the hakchi2 CE `fes1.bin` is
    /// 14 048 bytes, starts with an ARM branch followed by the Allwinner `eGON.BT0` magic, and
    /// has CRC-32 0x93aada9f (md5 2d1c7b3b9fe5f88bc93aef662abe5cfe).
    #[test]
    fn fes1_is_the_hakchi2_ce_binary() {
        let bytes = fes1();
        assert_eq!(bytes.len(), 14048);
        assert_eq!(&bytes[4..12], b"eGON.BT0");
        assert_eq!(crc32(bytes), 0x93aa_da9f);
    }

    /// Plain CRC-32 (IEEE, reflected), enough to fingerprint a blob in a test.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    #[test]
    fn crc32_matches_a_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
