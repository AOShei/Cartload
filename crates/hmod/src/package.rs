//! Reading a `hakchi.hmod` package from disk.

use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use tracing::debug;

use crate::{Error, Result, Version};

/// Archive entry holding the kernel and ramdisk (Android boot image).
pub const BOOT_IMAGE: &str = "boot/boot.img";
/// Archive entry holding u-boot.
pub const UBOOT: &str = "boot/uboot.bin";
/// Archive entry holding the version variables, see [`Version`].
pub const VERSION_FILE: &str = "var/version";

/// Upper bound on the buffer reserved up front for one entry; larger entries just grow.
const PREALLOC_LIMIT: usize = 64 * 1024 * 1024;

/// What kind of object an archive entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link; the target is in [`Entry::link_target`].
    Symlink,
    /// A hard link to an earlier entry; the target is in [`Entry::link_target`].
    HardLink,
    /// Anything else (device nodes, fifos, ...).
    Other,
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::File => "regular file",
            Self::Directory => "directory",
            Self::Symlink => "symbolic link",
            Self::HardLink => "hard link",
            Self::Other => "special entry",
        })
    }
}

/// One entry of the archive, as listed by [`Package::entries`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Normalised name: no leading `./` or `/`, no trailing `/`.
    pub name: String,
    /// Size of the contents in bytes (0 for anything but regular files).
    pub size: u64,
    /// What the entry is.
    pub kind: EntryKind,
    /// Link target for symbolic and hard links.
    pub link_target: Option<String>,
}

/// The files a FEL memboot needs, read in one pass with [`Package::boot_files`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootFiles {
    /// `boot/boot.img`.
    pub boot_image: Vec<u8>,
    /// `boot/uboot.bin`.
    pub uboot: Vec<u8>,
    /// `var/version`.
    pub version: Version,
}

/// A `hakchi.hmod` package on disk.
///
/// [`Package::open`] decompresses the whole archive once to list it; every read then streams the
/// archive again up to the wanted entry, so the package is never held in memory as a whole.
/// Entry names are normalised (see [`Entry::name`]) and lookups accept both `boot/boot.img` and
/// `./boot/boot.img`. When a name occurs more than once, the last occurrence wins, as it does
/// when tar extracts the archive.
#[derive(Clone, Debug)]
pub struct Package {
    path: PathBuf,
    entries: Vec<Entry>,
}

impl Package {
    /// Open and list a package. Fails if the file cannot be read, is not a gzip tar, is
    /// truncated or corrupt, or has no entries.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut archive = open_archive(&path)?;
        let mut entries = Vec::new();
        for entry in archive.entries().map_err(|e| archive_error(&path, e))? {
            let entry = entry.map_err(|e| archive_error(&path, e))?;
            let described = describe(&path, &entry)?;
            if described.name.is_empty() {
                // The archive root (`./`), which carries nothing.
                continue;
            }
            entries.push(described);
        }
        if entries.is_empty() {
            return Err(Error::Empty { path });
        }
        debug!(path = %path.display(), entries = entries.len(), "opened package");
        Ok(Self { path, entries })
    }

    /// Where the package lives.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// All entries in archive order, with normalised names.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Look an entry up by name (either name form). The last occurrence wins.
    pub fn entry(&self, name: &str) -> Option<&Entry> {
        let name = normalise(name);
        self.entries.iter().rev().find(|e| e.name == name)
    }

    /// Whether an entry with this name exists, of any kind.
    pub fn contains(&self, name: &str) -> bool {
        self.entry(name).is_some()
    }

    /// Read one regular file out of the archive.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let wanted = normalise(name);
        let mut files = self.read_many(&[wanted.as_str()])?;
        files.remove(&wanted).ok_or_else(|| self.missing(&wanted))
    }

    /// Read and parse `var/version`.
    pub fn version(&self) -> Result<Version> {
        let data = self.read(VERSION_FILE)?;
        self.parse_version(&data)
    }

    /// Read `boot/boot.img`.
    pub fn boot_image(&self) -> Result<Vec<u8>> {
        self.read(BOOT_IMAGE)
    }

    /// Read `boot/uboot.bin`.
    pub fn uboot(&self) -> Result<Vec<u8>> {
        self.read(UBOOT)
    }

    /// Read the boot image, u-boot and the version in one pass over the archive.
    pub fn boot_files(&self) -> Result<BootFiles> {
        let mut files = self.read_many(&[BOOT_IMAGE, UBOOT, VERSION_FILE])?;
        let mut take = |name: &str| files.remove(name).ok_or_else(|| self.missing(name));
        let boot_image = take(BOOT_IMAGE)?;
        let uboot = take(UBOOT)?;
        let version = take(VERSION_FILE)?;
        Ok(BootFiles {
            boot_image,
            uboot,
            version: self.parse_version(&version)?,
        })
    }

    /// Check that this is a kernel package: `boot/boot.img` and `boot/uboot.bin` are non-empty
    /// regular files and `var/version` parses. Returns the version.
    pub fn validate(&self) -> Result<Version> {
        for name in [BOOT_IMAGE, UBOOT] {
            let entry = self.file_entry(name)?;
            if entry.size == 0 {
                return Err(Error::EmptyEntry {
                    path: self.path.clone(),
                    entry: name.to_string(),
                });
            }
        }
        self.version()
    }

    /// Resolve a name to its regular-file entry, or explain why there is none.
    fn file_entry(&self, name: &str) -> Result<&Entry> {
        let entry = self.entry(name).ok_or_else(|| self.missing(name))?;
        if entry.kind != EntryKind::File {
            return Err(Error::NotAFile {
                path: self.path.clone(),
                entry: entry.name.clone(),
                kind: entry.kind,
            });
        }
        Ok(entry)
    }

    /// Read several regular files in one pass, keyed by normalised name. Every requested name
    /// must exist as a regular file, so the map holds a value for each of them on success.
    fn read_many(&self, names: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        // Index of the last occurrence of each wanted name; reading stops after the furthest.
        let mut targets: HashMap<usize, String> = HashMap::new();
        for name in names {
            let wanted = normalise(name);
            let (index, _) = self
                .entries
                .iter()
                .enumerate()
                .rev()
                .find(|(_, e)| e.name == wanted)
                .ok_or_else(|| self.missing(&wanted))?;
            self.file_entry(&wanted)?;
            targets.insert(index, wanted);
        }
        let Some(last) = targets.keys().copied().max() else {
            return Ok(HashMap::new());
        };

        let mut files = HashMap::with_capacity(targets.len());
        let mut archive = open_archive(&self.path)?;
        // Listing skipped the root entry, so count only what the listing counted.
        let mut index = 0usize;
        for entry in archive.entries().map_err(|e| self.archive_error(e))? {
            let mut entry = entry.map_err(|e| self.archive_error(e))?;
            let described = describe(&self.path, &entry)?;
            if described.name.is_empty() {
                continue;
            }
            if let Some(wanted) = targets.get(&index) {
                if described.name != *wanted {
                    return Err(self.archive_error(io::Error::other(
                        "the entry list changed since the package was opened",
                    )));
                }
                let capacity = usize::try_from(entry.size())
                    .unwrap_or(PREALLOC_LIMIT)
                    .min(PREALLOC_LIMIT);
                let mut data = Vec::with_capacity(capacity);
                entry
                    .read_to_end(&mut data)
                    .map_err(|e| self.archive_error(e))?;
                debug!(entry = %wanted, len = data.len(), "read from package");
                files.insert(wanted.clone(), data);
            }
            if index >= last {
                break;
            }
            index += 1;
        }
        Ok(files)
    }

    fn parse_version(&self, data: &[u8]) -> Result<Version> {
        let text = std::str::from_utf8(data).map_err(|_| Error::InvalidVersion {
            path: self.path.clone(),
            reason: "not UTF-8 text".to_string(),
        })?;
        Version::parse(text).ok_or_else(|| Error::InvalidVersion {
            path: self.path.clone(),
            reason: format!("no {} variable", crate::version::HAKCHI_VERSION),
        })
    }

    fn missing(&self, name: &str) -> Error {
        Error::MissingEntry {
            path: self.path.clone(),
            entry: normalise(name),
        }
    }

    fn archive_error(&self, source: io::Error) -> Error {
        archive_error(&self.path, source)
    }
}

type Archive = tar::Archive<GzDecoder<BufReader<File>>>;

fn open_archive(path: &Path) -> Result<Archive> {
    let file = File::open(path).map_err(|source| Error::Io {
        op: "open",
        path: path.to_path_buf(),
        source,
    })?;
    Ok(tar::Archive::new(GzDecoder::new(BufReader::new(file))))
}

fn archive_error(path: &Path, source: io::Error) -> Error {
    Error::Archive {
        path: path.to_path_buf(),
        source,
    }
}

/// Describe a tar entry without reading its contents.
fn describe<R: Read>(path: &Path, entry: &tar::Entry<'_, R>) -> Result<Entry> {
    let raw = entry.path().map_err(|e| archive_error(path, e))?;
    let name = normalise(&raw.to_string_lossy());
    let kind = match entry.header().entry_type() {
        tar::EntryType::Directory => EntryKind::Directory,
        tar::EntryType::Symlink => EntryKind::Symlink,
        tar::EntryType::Link => EntryKind::HardLink,
        other if other.is_file() => EntryKind::File,
        _ => EntryKind::Other,
    };
    let link_target = entry
        .link_name()
        .map_err(|e| archive_error(path, e))?
        .map(|target| target.to_string_lossy().into_owned());
    Ok(Entry {
        name,
        size: if kind == EntryKind::File {
            entry.size()
        } else {
            0
        },
        kind,
        link_target,
    })
}

/// Strip leading `./` and `/` and trailing `/` from an entry name.
fn normalise(name: &str) -> String {
    let mut name = name;
    loop {
        if let Some(rest) = name.strip_prefix("./") {
            name = rest;
        } else if let Some(rest) = name.strip_prefix('/') {
            name = rest;
        } else {
            break;
        }
    }
    let name = name.trim_end_matches('/');
    if name == "." { "" } else { name }.to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::{
        BOOT_IMG, Spec, UBOOT as UBOOT_DATA, VERSION_TEXT, archive, sample, temp_file,
    };

    fn names(package: &Package) -> Vec<&str> {
        package.entries().iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn normalise_strips_prefixes_and_trailing_slash() {
        assert_eq!(normalise("./boot/boot.img"), "boot/boot.img");
        assert_eq!(normalise("boot/boot.img"), "boot/boot.img");
        assert_eq!(normalise("/boot/"), "boot");
        assert_eq!(normalise("././bin/"), "bin");
        assert_eq!(normalise("./"), "");
        assert_eq!(normalise("."), "");
        assert_eq!(normalise(""), "");
    }

    #[test]
    fn lists_entries_with_normalised_names() {
        let (_dir, path) = temp_file("sample.hmod", &sample());
        let package = Package::open(&path).unwrap();
        assert_eq!(package.path(), path);
        assert_eq!(
            names(&package),
            [
                "bin",
                "bin/sunxi-flash",
                "bin/sntool",
                "boot",
                "boot/boot.img",
                "boot/uboot.bin",
                "install",
                "uninstall",
                "var",
                "var/version",
            ]
        );
        let link = package.entry("bin/sunxi-flash").unwrap();
        assert_eq!(link.kind, EntryKind::Symlink);
        assert_eq!(link.link_target.as_deref(), Some("sntool"));
        assert_eq!(link.size, 0);
        let dir = package.entry("./boot/").unwrap();
        assert_eq!(dir.kind, EntryKind::Directory);
        let boot = package.entry("boot/boot.img").unwrap();
        assert_eq!(boot.kind, EntryKind::File);
        assert_eq!(boot.size, BOOT_IMG.len() as u64);
        assert!(package.contains("./var/version"));
        assert!(!package.contains("var/missing"));
    }

    #[test]
    fn reads_entries_by_either_name_form() {
        let (_dir, path) = temp_file("sample.hmod", &sample());
        let package = Package::open(&path).unwrap();
        assert_eq!(
            package.read("var/version").unwrap(),
            VERSION_TEXT.as_bytes()
        );
        assert_eq!(
            package.read("./var/version").unwrap(),
            VERSION_TEXT.as_bytes()
        );
        assert_eq!(package.read("./boot/boot.img").unwrap(), BOOT_IMG);
        assert_eq!(package.read("boot/boot.img").unwrap(), BOOT_IMG);
        assert_eq!(package.boot_image().unwrap(), BOOT_IMG);
        assert_eq!(package.uboot().unwrap(), UBOOT_DATA);
        assert_eq!(
            package.read("install").unwrap(),
            b"#!/bin/sh\necho install\n"
        );
    }

    #[test]
    fn boot_files_and_version_in_one_pass() {
        let (_dir, path) = temp_file("sample.hmod", &sample());
        let package = Package::open(&path).unwrap();
        let files = package.boot_files().unwrap();
        assert_eq!(files.boot_image, BOOT_IMG);
        assert_eq!(files.uboot, UBOOT_DATA);
        assert_eq!(files.version.hakchi, "v1.0.4-126");
        assert_eq!(files.version, package.version().unwrap());
        assert_eq!(package.validate().unwrap(), files.version);
    }

    #[test]
    fn missing_entry_is_an_error() {
        let (_dir, path) = temp_file("sample.hmod", &sample());
        let package = Package::open(&path).unwrap();
        let error = package.read("./boot/kernel.img").unwrap_err();
        assert!(
            matches!(&error, Error::MissingEntry { entry, .. } if entry == "boot/kernel.img"),
            "{error}"
        );
        assert!(error.to_string().ends_with("has no entry boot/kernel.img"));
    }

    #[test]
    fn non_files_cannot_be_read() {
        let (_dir, path) = temp_file("sample.hmod", &sample());
        let package = Package::open(&path).unwrap();
        let error = package.read("bin/sunxi-flash").unwrap_err();
        assert!(
            matches!(&error, Error::NotAFile { kind: EntryKind::Symlink, entry, .. } if entry == "bin/sunxi-flash"),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .contains("is a symbolic link, not a regular file")
        );
        let error = package.read("boot").unwrap_err();
        assert!(
            matches!(
                error,
                Error::NotAFile {
                    kind: EntryKind::Directory,
                    ..
                }
            ),
            "{error}"
        );
    }

    #[test]
    fn last_duplicate_wins() {
        let bytes = archive(&[
            Spec::File("var/version", b"hakchiVersion='first'\n"),
            Spec::File("boot/boot.img", b"x"),
            Spec::File("./var/version", b"hakchiVersion='second'\n"),
        ]);
        let (_dir, path) = temp_file("dup.hmod", &bytes);
        let package = Package::open(&path).unwrap();
        assert_eq!(package.entries().len(), 3);
        assert_eq!(package.version().unwrap().hakchi, "second");
        assert_eq!(
            package.read("var/version").unwrap(),
            b"hakchiVersion='second'\n"
        );
    }

    #[test]
    fn read_stops_at_the_last_wanted_entry() {
        // A corrupt tail after the wanted entries must not matter for reads, only for open.
        // Build a valid archive, then check that reading the first entry works even though a
        // later entry is huge, by reading only what is needed.
        let big = vec![0u8; 300_000];
        let bytes = archive(&[
            Spec::File("var/version", VERSION_TEXT.as_bytes()),
            Spec::File("big.bin", &big),
        ]);
        let (_dir, path) = temp_file("big.hmod", &bytes);
        let package = Package::open(&path).unwrap();
        assert_eq!(package.version().unwrap().hakchi, "v1.0.4-126");
        assert_eq!(package.read("big.bin").unwrap().len(), big.len());
    }

    #[test]
    fn rejects_files_that_are_not_gzip_tars() {
        let (_dir, path) = temp_file("text.hmod", b"this is not an archive at all\n");
        let error = Package::open(&path).unwrap_err();
        assert!(matches!(error, Error::Archive { .. }), "{error}");
        assert!(
            error
                .to_string()
                .contains("is not a readable gzip tar archive")
        );
    }

    #[test]
    fn rejects_truncated_archives() {
        let full = sample();
        let (_dir, path) = temp_file("cut.hmod", &full[..full.len() / 2]);
        let error = Package::open(&path).unwrap_err();
        assert!(matches!(error, Error::Archive { .. }), "{error}");
    }

    #[test]
    fn rejects_empty_input() {
        let (_dir, path) = temp_file("empty.hmod", b"");
        let error = Package::open(&path).unwrap_err();
        assert!(
            matches!(error, Error::Archive { .. } | Error::Empty { .. }),
            "{error}"
        );
        let (_dir, path) = temp_file("noentries.hmod", &archive(&[]));
        let error = Package::open(&path).unwrap_err();
        assert!(matches!(error, Error::Empty { .. }), "{error}");
    }

    #[test]
    fn missing_file_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = Package::open(dir.path().join("absent.hmod")).unwrap_err();
        assert!(matches!(error, Error::Io { op: "open", .. }), "{error}");
        assert!(error.to_string().starts_with("cannot open "));
    }

    #[test]
    fn validate_reports_what_is_missing_or_empty() {
        let without_uboot = archive(&[
            Spec::File("boot/boot.img", b"x"),
            Spec::File("var/version", VERSION_TEXT.as_bytes()),
        ]);
        let (_dir, path) = temp_file("a.hmod", &without_uboot);
        let error = Package::open(&path).unwrap().validate().unwrap_err();
        assert!(
            matches!(&error, Error::MissingEntry { entry, .. } if entry == UBOOT),
            "{error}"
        );

        let empty_boot = archive(&[
            Spec::File("boot/boot.img", b""),
            Spec::File("boot/uboot.bin", b"u"),
            Spec::File("var/version", VERSION_TEXT.as_bytes()),
        ]);
        let (_dir, path) = temp_file("b.hmod", &empty_boot);
        let error = Package::open(&path).unwrap().validate().unwrap_err();
        assert!(
            matches!(&error, Error::EmptyEntry { entry, .. } if entry == BOOT_IMAGE),
            "{error}"
        );

        let no_version = archive(&[
            Spec::File("boot/boot.img", b"x"),
            Spec::File("boot/uboot.bin", b"u"),
        ]);
        let (_dir, path) = temp_file("c.hmod", &no_version);
        let error = Package::open(&path).unwrap().validate().unwrap_err();
        assert!(
            matches!(&error, Error::MissingEntry { entry, .. } if entry == VERSION_FILE),
            "{error}"
        );

        let bad_version = archive(&[
            Spec::File("boot/boot.img", b"x"),
            Spec::File("boot/uboot.bin", b"u"),
            Spec::File("var/version", b"bootVersion='1.0.3'\n"),
        ]);
        let (_dir, path) = temp_file("d.hmod", &bad_version);
        let error = Package::open(&path).unwrap().validate().unwrap_err();
        assert!(matches!(error, Error::InvalidVersion { .. }), "{error}");
        assert!(error.to_string().contains("no hakchiVersion variable"));

        let binary_version = archive(&[
            Spec::File("boot/boot.img", b"x"),
            Spec::File("boot/uboot.bin", b"u"),
            Spec::File("var/version", &[0xff, 0xfe, 0x00]),
        ]);
        let (_dir, path) = temp_file("e.hmod", &binary_version);
        let error = Package::open(&path).unwrap().validate().unwrap_err();
        assert!(matches!(error, Error::InvalidVersion { .. }), "{error}");
        assert!(error.to_string().contains("not UTF-8"));
    }

    /// The real hakchi2 CE package, when `CARTLOAD_SAMPLE_HMOD` points at it.
    #[test]
    fn opens_the_sample_package_from_env() {
        let Some(path) = std::env::var_os("CARTLOAD_SAMPLE_HMOD") else {
            eprintln!("CARTLOAD_SAMPLE_HMOD not set, skipping");
            return;
        };
        let package = Package::open(&path).unwrap();
        assert!(package.entries().len() > 10);
        let version = package.validate().unwrap();
        assert!(version.hakchi.starts_with('v'), "{version}");
        assert!(
            version.boot.is_some() && version.kernel.is_some(),
            "{version}"
        );
        let files = package.boot_files().unwrap();
        assert_eq!(&files.boot_image[..8], b"ANDROID!");
        assert!(!files.uboot.is_empty());
        assert_eq!(files.version, version);
        for name in [
            "install",
            "uninstall",
            "etc/preinit.d/b0010_functions",
            "bin/sntool",
        ] {
            assert_eq!(
                package.entry(name).map(|e| e.kind),
                Some(EntryKind::File),
                "{name}"
            );
        }
        assert_eq!(
            package
                .entry("bin/sunxi-flash")
                .and_then(|e| e.link_target.as_deref()),
            Some("sntool")
        );
        if version.hakchi == "v1.0.4-126" {
            assert_eq!(package.entries().len(), 239);
            assert_eq!(files.boot_image.len(), 4_585_472);
            assert_eq!(files.uboot.len(), 761_856);
            assert_eq!(
                version.to_string(),
                "v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)"
            );
        }
    }
}
