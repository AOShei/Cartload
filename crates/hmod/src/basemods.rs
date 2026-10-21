//! hakchi2 CE's base mods: the small hmod packages CE installs next to the kernel
//! (`basehmods.tar`, built at CE's build time from `hakchi_gui/mods/hmods/` in its source
//! tree). Cartload needs one of them for compressed games: `tiny7zx`, the 7z extractor the
//! emulator wrappers run at launch. `font_remount` (CE's extended title font) is listed too
//! because CE always installs it; it is optional here.
//!
//! The files are GPL-3 artifacts of the CE repository and are never bundled: they are
//! downloaded on demand from the CE repository at a pinned commit ([`SOURCE_COMMIT`]), checked
//! against the sizes and MD5s recorded here, and cached under a directory of the caller's
//! choice (see [`default_cache_dir`]). [`BaseMod::load`] turns the cached files into the tar
//! stream the console's `pack_install` takes, exactly as CE's `ModTasks.TransferHmod` sends a
//! directory-form hmod (`tar -xvC <dir>` on the console, then `hakchi packs_install`).

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The CE repository the files come from.
pub const SOURCE_REPOSITORY: &str = "https://github.com/TeamShinkansen/Hakchi2-CE";
/// The commit of the CE repository the files are pinned to (head of `mainline`, checked on
/// 2026-10-21 against the local CE checkout and `basehmods.tar`).
pub const SOURCE_COMMIT: &str = "4fb3092c29cd39a85504bd75889ffb18e749a704";
/// Where the mods live in the CE source tree.
const SOURCE_DIR: &str = "hakchi_gui/mods/hmods";

/// One file of a base mod.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModFile {
    /// Path inside the hmod (`bin/tiny7zx`).
    pub path: &'static str,
    /// Size in bytes.
    pub size: u64,
    /// Lowercase hex MD5.
    pub md5: &'static str,
    /// Whether the file is executable on the console.
    pub executable: bool,
}

impl ModFile {
    /// Whether the file ends up in the console's rootfs. `install`, `uninstall` and the
    /// readme are consumed by the device's `pack_install` and never copied.
    pub fn is_payload(&self) -> bool {
        let name = self.path.rsplit('/').next().unwrap_or(self.path);
        !(self.path == "install"
            || self.path == "uninstall"
            || self.path == "transfer"
            || name.eq_ignore_ascii_case("readme")
            || name.to_ascii_lowercase().starts_with("readme.")
            || name.to_ascii_lowercase().ends_with(".txt")
            || name.to_ascii_lowercase().ends_with(".md"))
    }
}

/// One of hakchi2 CE's base mods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseMod {
    /// The hmod name (`tiny7zx`): the console registers it as `uninstall-<name>`.
    pub name: &'static str,
    /// Title from the mod's readme.
    pub title: &'static str,
    /// What it does and why Cartload offers it.
    pub summary: &'static str,
    /// Its files.
    pub files: &'static [ModFile],
}

/// `tiny7zx`: pts-tiny-7z-sfx by Péter Szabó (GPL-2), packaged by cluster, madmonkey and
/// KMFDManic. The emulator wrappers in the hakchi rootfs run `tiny7zx x` on a `.7z` ROM.
pub const TINY7ZX: BaseMod = BaseMod {
    name: "tiny7zx",
    title: "tiny7zx",
    summary: "7z extractor the console runs at launch for compressed games; required when any \
              game in the library is compressed",
    files: &[
        ModFile {
            path: "bin/tiny7zx",
            size: 26578,
            md5: "eb8fa3b5a0d8a832e81eec9b5fccb60b",
            executable: true,
        },
        ModFile {
            path: "install",
            size: 37,
            md5: "9fc874cd8fe53b3662fdd9446b7f38cc",
            executable: false,
        },
        ModFile {
            path: "readme.md",
            size: 3501,
            md5: "b19c71bef9cbc6f8e42ba5743aad0ec6",
            executable: false,
        },
    ],
};

/// `font_remount` 1.2.0 by DanTheMan827: binds an extended title font (and any font files
/// placed in the hakchi `fonts` directory) over the firmware's. CE installs it by default.
pub const FONT_REMOUNT: BaseMod = BaseMod {
    name: "font_remount",
    title: "Font Remount",
    summary: "extended title font bound over the firmware's at boot (hakchi2 CE installs it \
              by default); optional, not needed for compressed games",
    files: &[
        ModFile {
            path: "etc/preinit.d/b8025_fontfix",
            size: 1903,
            md5: "2c45facb0333242ac6cfcc23567b0278",
            executable: false,
        },
        ModFile {
            path: "etc/preinit.d/p8025_fontfix",
            size: 8,
            md5: "4667b29c682828e8b7f6070f65d10464",
            executable: false,
        },
        ModFile {
            path: "usr/share/fonts/title.fnt",
            size: 986_778,
            md5: "fa25d29460e78d4353120f3f53453919",
            executable: false,
        },
        ModFile {
            path: "readme.md",
            size: 740,
            md5: "1d69ce6d1888e538332ea8273e7b467b",
            executable: false,
        },
        ModFile {
            path: "uninstall",
            size: 89,
            md5: "e9bde43d14ac373268b82a21a25d3322",
            executable: false,
        },
    ],
};

/// Every base mod Cartload knows.
pub const CATALOG: &[&BaseMod] = &[&TINY7ZX, &FONT_REMOUNT];
/// The mods a compressed library needs.
pub const FOR_COMPRESSION: &[&BaseMod] = &[&TINY7ZX];

/// Looks a mod up by name (`tiny7zx`, with or without `.hmod`).
pub fn find(name: &str) -> Option<&'static BaseMod> {
    let name = name.strip_suffix(".hmod").unwrap_or(name);
    CATALOG.iter().copied().find(|m| m.name == name)
}

/// The default cache: `<user cache dir>/cartload/basemods`.
pub fn default_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("cartload").join("basemods"))
}

/// Which files of a mod the cache holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheStatus {
    /// Files present with the right size and MD5.
    pub present: Vec<ModFile>,
    /// Files missing, truncated or with the wrong MD5.
    pub missing: Vec<ModFile>,
}

impl CacheStatus {
    /// Whether every file is there.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// A mod read from the cache, ready to send to the console.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedMod {
    /// Which mod.
    pub spec: &'static BaseMod,
    /// The files, in catalog order: path inside the hmod and content.
    pub files: Vec<(String, Vec<u8>)>,
    /// The hmod as a plain (uncompressed) tar stream, what `tar -xvC <dir>` on the console
    /// takes.
    pub tar: Vec<u8>,
}

impl LoadedMod {
    /// The hmod name.
    pub fn name(&self) -> &'static str {
        self.spec.name
    }

    /// Bytes of all files together.
    pub fn len(&self) -> u64 {
        self.files.iter().map(|(_, data)| data.len() as u64).sum()
    }

    /// Whether the mod has no files (never, for the catalog).
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

impl BaseMod {
    /// The directory the mod's files are cached in: `<root>/<commit>/<name>.hmod/`.
    pub fn cache_dir(&self, cache_root: &Path) -> PathBuf {
        cache_root
            .join(SOURCE_COMMIT)
            .join(format!("{}.hmod", self.name))
    }

    /// Where a file of the mod is cached.
    pub fn cached_path(&self, cache_root: &Path, file: &ModFile) -> PathBuf {
        self.cache_dir(cache_root).join(file.path)
    }

    /// Where a file of the mod lives in the CE source tree at the pinned commit.
    pub fn source_path(&self, file: &ModFile) -> String {
        format!("{SOURCE_DIR}/{}.hmod/{}", self.name, file.path)
    }

    /// The download locations of a file, in the order to try them: GitHub's raw file host,
    /// then the repository's own `raw` route (same bytes, different front door).
    pub fn urls(&self, file: &ModFile) -> [String; 2] {
        let path = self.source_path(file);
        let repo = SOURCE_REPOSITORY.trim_start_matches("https://github.com/");
        [
            format!("https://raw.githubusercontent.com/{repo}/{SOURCE_COMMIT}/{path}"),
            format!("{SOURCE_REPOSITORY}/raw/{SOURCE_COMMIT}/{path}"),
        ]
    }

    /// The files that end up in the console's rootfs.
    pub fn payload(&self) -> impl Iterator<Item = &'static ModFile> {
        self.files.iter().filter(|f| f.is_payload())
    }

    /// Checks the cache for the mod's files (size and MD5).
    pub fn status(&self, cache_root: &Path) -> CacheStatus {
        let mut status = CacheStatus::default();
        for file in self.files {
            match std::fs::read(self.cached_path(cache_root, file)) {
                Ok(data) if verify(file, &data).is_ok() => status.present.push(*file),
                _ => status.missing.push(*file),
            }
        }
        status
    }

    /// Reads the mod from the cache, verifying every file, and builds its tar stream.
    pub fn load(&self, cache_root: &Path) -> Result<LoadedMod> {
        let mut files = Vec::with_capacity(self.files.len());
        for file in self.files {
            let path = self.cached_path(cache_root, file);
            let data = match std::fs::read(&path) {
                Ok(data) => data,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    return Err(Error::ModNotCached {
                        name: self.name.to_string(),
                        path,
                    });
                }
                Err(source) => {
                    return Err(Error::Io {
                        op: "read",
                        path,
                        source,
                    });
                }
            };
            verify(file, &data).map_err(|(expected, actual)| Error::ModChecksum {
                name: self.name.to_string(),
                path,
                expected,
                actual,
            })?;
            files.push((file.path.to_string(), data));
        }
        let tar = build_tar(self, &files)?;
        Ok(LoadedMod {
            spec: find(self.name).unwrap_or(&TINY7ZX),
            files,
            tar,
        })
    }
}

/// Size and MD5 check; the error carries the expected and actual `size/md5` descriptions.
fn verify(file: &ModFile, data: &[u8]) -> std::result::Result<(), (String, String)> {
    let actual_md5 = md5_hex(data);
    if data.len() as u64 == file.size && actual_md5 == file.md5 {
        Ok(())
    } else {
        Err((
            format!("{} bytes, md5 {}", file.size, file.md5),
            format!("{} bytes, md5 {actual_md5}", data.len()),
        ))
    }
}

/// Lowercase hex MD5 of `data`.
pub fn md5_hex(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// Fixed timestamp for the tar entries (CE's build stamps them with the build time; the
/// console's `pack_install` does not look at it).
const TAR_MTIME: u64 = 1_585_912_351;

/// A plain tar of the mod's files, modes 0755 for executables and 0644 otherwise, owner root.
fn build_tar(spec: &BaseMod, files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, data) in files {
        let executable = spec.files.iter().any(|f| f.path == path && f.executable);
        let mut header = tar::Header::new_gnu();
        header.set_mtime(TAR_MTIME);
        header.set_uid(0);
        header.set_gid(0);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(if executable { 0o755 } else { 0o644 });
        header.set_size(data.len() as u64);
        builder
            .append_data(&mut header, path, data.as_slice())
            .map_err(|source| tar_error(spec, source))?;
    }
    builder
        .into_inner()
        .map_err(|source| tar_error(spec, source))
}

fn tar_error(spec: &BaseMod, source: std::io::Error) -> Error {
    Error::Io {
        op: "pack",
        path: PathBuf::from(format!("{}.hmod", spec.name)),
        source,
    }
}

/// Downloading the mods' files into the cache.
#[cfg(feature = "fetch")]
pub use fetch::{ModProgress, fetch};

#[cfg(feature = "fetch")]
mod fetch {
    use std::path::Path;

    use tokio::io::AsyncWriteExt;
    use tracing::{debug, info, warn};

    use super::{BaseMod, LoadedMod, ModFile, md5_hex};
    use crate::{Error, Failures, Result};

    /// Progress of one file's download.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ModProgress {
        /// The mod.
        pub name: &'static str,
        /// The file inside the mod.
        pub file: &'static str,
        /// Bytes received so far.
        pub done: u64,
        /// Bytes expected (the catalog size).
        pub total: u64,
    }

    /// Makes sure every file of `mods` is in the cache, downloading the missing or damaged
    /// ones, then loads them. Each file is written to `<path>.partial`, checked against the
    /// catalog size and MD5 and renamed; a failed source moves on to the next one. Nothing is
    /// sent to the console here.
    pub async fn fetch(
        mods: &[&'static BaseMod],
        cache_root: &Path,
        mut progress: impl FnMut(ModProgress),
    ) -> Result<Vec<LoadedMod>> {
        let mut client = None;
        for spec in mods {
            let status = spec.status(cache_root);
            for file in &status.missing {
                let client = match &client {
                    Some(client) => client,
                    None => client.insert(crate::fetch::client()?),
                };
                fetch_file(client, spec, file, cache_root, &mut progress).await?;
            }
        }
        mods.iter().map(|spec| spec.load(cache_root)).collect()
    }

    async fn fetch_file(
        client: &reqwest::Client,
        spec: &'static BaseMod,
        file: &ModFile,
        cache_root: &Path,
        progress: &mut impl FnMut(ModProgress),
    ) -> Result<()> {
        let dest = spec.cached_path(cache_root, file);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|source| io_error("create directory", parent, source))?;
        }
        let mut failures = Failures::default();
        for url in spec.urls(file) {
            match fetch_one(client, &url, spec, file, &dest, progress).await {
                Ok(()) => {
                    info!(url, path = %dest.display(), size = file.size, "downloaded mod file");
                    return Ok(());
                }
                Err(error) => {
                    warn!(url, %error, "download failed");
                    failures.0.push((url, error));
                }
            }
        }
        Err(Error::AllSourcesFailed(failures))
    }

    async fn fetch_one(
        client: &reqwest::Client,
        url: &str,
        spec: &'static BaseMod,
        file: &ModFile,
        dest: &Path,
        progress: &mut impl FnMut(ModProgress),
    ) -> Result<()> {
        let mut partial = dest.as_os_str().to_owned();
        partial.push(".partial");
        let partial = std::path::PathBuf::from(partial);
        let result = download(client, url, spec, file, &partial, progress).await;
        if let Err(error) = result {
            discard(&partial).await;
            return Err(error);
        }
        if let Err(source) = tokio::fs::rename(&partial, dest).await {
            discard(&partial).await;
            return Err(io_error("rename", &partial, source));
        }
        Ok(())
    }

    async fn download(
        client: &reqwest::Client,
        url: &str,
        spec: &'static BaseMod,
        file: &ModFile,
        partial: &Path,
        progress: &mut impl FnMut(ModProgress),
    ) -> Result<()> {
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|source| Error::Download {
                url: url.to_string(),
                source: source.without_url(),
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }
        debug!(url, final_url = %response.url(), "download started");
        let mut out = tokio::fs::File::create(partial)
            .await
            .map_err(|source| io_error("create", partial, source))?;
        let mut data = Vec::with_capacity(usize::try_from(file.size).unwrap_or(0));
        let report = |done: u64, progress: &mut dyn FnMut(ModProgress)| {
            progress(ModProgress {
                name: spec.name,
                file: file.path,
                done,
                total: file.size,
            });
        };
        report(0, progress);
        while let Some(chunk) = response.chunk().await.map_err(|source| Error::Download {
            url: url.to_string(),
            source: source.without_url(),
        })? {
            if data.len() as u64 + chunk.len() as u64 > file.size {
                return Err(Error::ModChecksum {
                    name: spec.name.to_string(),
                    path: partial.to_path_buf(),
                    expected: format!("{} bytes, md5 {}", file.size, file.md5),
                    actual: format!("more than {} bytes from {url}", file.size),
                });
            }
            out.write_all(&chunk)
                .await
                .map_err(|source| io_error("write", partial, source))?;
            data.extend_from_slice(&chunk);
            report(data.len() as u64, progress);
        }
        out.sync_all()
            .await
            .map_err(|source| io_error("sync", partial, source))?;
        drop(out);
        let actual = md5_hex(&data);
        if data.len() as u64 != file.size || actual != file.md5 {
            return Err(Error::ModChecksum {
                name: spec.name.to_string(),
                path: partial.to_path_buf(),
                expected: format!("{} bytes, md5 {}", file.size, file.md5),
                actual: format!("{} bytes, md5 {actual} from {url}", data.len()),
            });
        }
        Ok(())
    }

    async fn discard(partial: &Path) {
        if let Err(error) = tokio::fs::remove_file(partial).await
            && error.kind() != std::io::ErrorKind::NotFound
        {
            warn!(path = %partial.display(), %error, "could not remove partial download");
        }
    }

    fn io_error(op: &'static str, path: &Path, source: std::io::Error) -> Error {
        Error::Io {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::io::Read;

    use super::*;

    /// Writes a mod's files to a cache with the catalog's sizes and MD5s faked by content: the
    /// real files are not in the repository, so the tests make content whose MD5 the catalog
    /// does not have and check the mismatch, and a tiny catalog of their own for the rest.
    fn write_cached(spec: &BaseMod, root: &Path, content: impl Fn(&ModFile) -> Vec<u8>) {
        for file in spec.files {
            let path = spec.cached_path(root, file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content(file)).unwrap();
        }
    }

    #[test]
    fn catalog_names_paths_and_urls() {
        assert_eq!(find("tiny7zx").map(|m| m.name), Some("tiny7zx"));
        assert_eq!(find("tiny7zx.hmod").map(|m| m.name), Some("tiny7zx"));
        assert_eq!(find("font_remount").map(|m| m.title), Some("Font Remount"));
        assert!(find("clovercon").is_none());
        assert_eq!(FOR_COMPRESSION, [&TINY7ZX]);
        let urls = TINY7ZX.urls(&TINY7ZX.files[0]);
        assert_eq!(
            urls[0],
            "https://raw.githubusercontent.com/TeamShinkansen/Hakchi2-CE/4fb3092c29cd39a85504bd75889ffb18e749a704/hakchi_gui/mods/hmods/tiny7zx.hmod/bin/tiny7zx"
        );
        assert_eq!(
            urls[1],
            "https://github.com/TeamShinkansen/Hakchi2-CE/raw/4fb3092c29cd39a85504bd75889ffb18e749a704/hakchi_gui/mods/hmods/tiny7zx.hmod/bin/tiny7zx"
        );
        let payload: Vec<&str> = TINY7ZX.payload().map(|f| f.path).collect();
        assert_eq!(payload, ["bin/tiny7zx"]);
        let payload: Vec<&str> = FONT_REMOUNT.payload().map(|f| f.path).collect();
        assert_eq!(
            payload,
            [
                "etc/preinit.d/b8025_fontfix",
                "etc/preinit.d/p8025_fontfix",
                "usr/share/fonts/title.fnt"
            ]
        );
        assert!(
            CATALOG
                .iter()
                .flat_map(|m| m.files)
                .all(|f| f.md5.len() == 32 && f.size > 0)
        );
        let root = Path::new("/cache");
        assert_eq!(
            TINY7ZX.cached_path(root, &TINY7ZX.files[0]),
            Path::new("/cache/4fb3092c29cd39a85504bd75889ffb18e749a704/tiny7zx.hmod/bin/tiny7zx")
        );
        assert_eq!(md5_hex(b"123456789"), "25f9e794323b453885f5181f1b624d0b");
    }

    #[test]
    fn cache_status_load_and_tar() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(!TINY7ZX.status(root).is_complete());
        assert!(matches!(
            TINY7ZX.load(root),
            Err(Error::ModNotCached { name, .. }) if name == "tiny7zx"
        ));

        // Wrong content: reported as a checksum mismatch and counted as missing.
        write_cached(&TINY7ZX, root, |f| vec![b'x'; f.size as usize]);
        let status = TINY7ZX.status(root);
        assert_eq!(status.missing.len(), 3);
        let error = TINY7ZX.load(root).unwrap_err();
        assert!(
            matches!(&error, Error::ModChecksum { name, expected, actual, .. }
                if name == "tiny7zx" && expected.contains("26578 bytes") && actual.contains("26578 bytes")),
            "{error}"
        );
        assert!(error.to_string().contains("tiny7zx"));

        // A catalog entry whose content we can produce: the install script's MD5 is of its
        // known text.
        let install = b"chmod +x \"$transferpath/bin/tiny7zx\"\n";
        assert_eq!(md5_hex(install), TINY7ZX.files[1].md5);
        assert_eq!(install.len() as u64, TINY7ZX.files[1].size);
        std::fs::write(TINY7ZX.cached_path(root, &TINY7ZX.files[1]), install).unwrap();
        let status = TINY7ZX.status(root);
        assert_eq!(status.present.len(), 1);
        assert_eq!(status.present[0].path, "install");
        assert_eq!(status.missing.len(), 2);
    }

    #[test]
    fn tar_holds_every_file_with_modes_and_root_owner() {
        let files = vec![
            ("bin/tiny7zx".to_string(), b"ELF".to_vec()),
            ("install".to_string(), b"chmod +x x\n".to_vec()),
            ("readme.md".to_string(), b"# readme".to_vec()),
        ];
        let tar = build_tar(&TINY7ZX, &files).unwrap();
        assert_eq!(tar.len() % 512, 0);
        let mut archive = tar::Archive::new(tar.as_slice());
        let mut seen = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let header = entry.header();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            let mode = header.mode().unwrap();
            assert_eq!(header.uid().unwrap(), 0);
            assert_eq!(header.gid().unwrap(), 0);
            assert_eq!(header.mtime().unwrap(), TAR_MTIME);
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            seen.push((path, mode, data));
        }
        assert_eq!(
            seen,
            [
                ("bin/tiny7zx".to_string(), 0o755, b"ELF".to_vec()),
                ("install".to_string(), 0o644, b"chmod +x x\n".to_vec()),
                ("readme.md".to_string(), 0o644, b"# readme".to_vec()),
            ]
        );
        assert_eq!(build_tar(&TINY7ZX, &files).unwrap(), tar, "deterministic");
    }

    #[test]
    fn payload_rules_match_the_device_script() {
        let file = |path: &'static str| ModFile {
            path,
            size: 1,
            md5: "",
            executable: false,
        };
        assert!(file("bin/x").is_payload());
        assert!(file("etc/preinit.d/p8025_fontfix").is_payload());
        assert!(!file("install").is_payload());
        assert!(!file("uninstall").is_payload());
        assert!(!file("readme.md").is_payload());
        assert!(!file("README").is_payload());
        assert!(!file("notes.txt").is_payload());
    }
}
