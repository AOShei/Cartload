//! Emulator modules from KMFD's Mod Hub, the repository hakchi2 CE ships as its default
//! (`https://hakchi.net/KMFDManic/NESC-SNESC-Modifications/.repo/`): RetroArch and the
//! libretro cores Cartload knows how to use. The hub is a plain directory tree: `<item>/link`
//! holds the download URL of the `.hmod` (a gzip tar), `<item>/md5` its checksum, as hakchi2
//! CE's `ModHub.Repository` reads them.
//!
//! Each module is pinned to one file by size and SHA-256 (KMFD's set of 2022-10-31, the
//! hub's current files on 2026-10-28). The same files are mirrored as assets of the Cartload
//! release [`MIRROR_TAG`]; a download tries the mirror first and the hub second, and keeps
//! only a file with the pinned hash. Downloads are cached under the user's cache directory, and
//! a cached file is used only while it still has the pinned hash.
//!
//! The catalog is deliberately short: RetroArch and one core per system the library knows
//! (NES, SNES, Mega Drive / Genesis, the Game Boy family, N64). Each core module also puts
//! the launcher the game's entry calls on the console's `PATH` (`/bin/md`, `/bin/gba`, ...).

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The hub's `.repo/` directory.
pub const HUB_URL: &str = "https://hakchi.net/KMFDManic/NESC-SNESC-Modifications/.repo/";
/// The Cartload release whose assets mirror the pinned module files, one `<item>.hmod` each.
pub const MIRROR_TAG: &str = "emulators-km-2022-10-31";
/// Where the mirrored files are downloaded from.
pub const MIRROR_URL: &str =
    "https://github.com/AOShei/Cartload/releases/download/emulators-km-2022-10-31/";

/// One module Cartload can install.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module {
    /// Cartload's short id (`retroarch`, `fceumm`, `snes9x`).
    pub id: &'static str,
    /// The hub item, also the hmod name the console registers it under.
    pub item: &'static str,
    /// Human name.
    pub title: &'static str,
    /// The name in the Settings grid: the system it plays, or RetroArch.
    pub short: &'static str,
    /// What it is for, one sentence.
    pub purpose: &'static str,
    /// Binaries the module puts on the console's `PATH`; checked after the install.
    pub binaries: &'static [&'static str],
    /// Size of the pinned file in bytes.
    pub len: u64,
    /// Lowercase hex SHA-256 of the pinned file.
    pub sha256: &'static str,
    /// A caution the app shows before installing, when the module has one.
    pub warning: Option<&'static str>,
}

/// RetroArch itself: the frontend, its launchers (`retroarch-clover`, `/bin/nes`, ...) and
/// no cores.
pub const RETROARCH: Module = Module {
    id: "retroarch",
    item: "_km_retroarch_1910_ozone",
    title: "RetroArch 1.9.10",
    short: "RetroArch 1.9.10",
    purpose: "The frontend every core runs in.",
    binaries: &["retroarch-clover", "nes"],
    len: 10_557_798,
    sha256: "c6a2dd4b8e1078703b5c420946146ee55ddf46105adc6f0b7d3b541567390575",
    warning: None,
};

/// The NES core hakchi's launcher script and `/bin/nes` use.
pub const FCEUMM: Module = Module {
    id: "fceumm",
    item: "_km_fceumm_xtreme",
    title: "NES core (FCEUmm)",
    short: "NES",
    purpose: "The FCEUmm core.",
    binaries: &["fceumm"],
    len: 761_432,
    sha256: "b9b8111880241c18f67f7c690610766e77d1ace5ad5ce2f304662d0feb5b75a2",
    warning: None,
};

/// A SNES core for games the built-in emulator does not run well.
pub const SNES9X: Module = Module {
    id: "snes9x",
    item: "_km_snes9x_xtreme",
    title: "SNES core (Snes9x)",
    short: "SNES",
    purpose: "The Snes9x core.",
    binaries: &["snes9x", "snes"],
    len: 3_111_882,
    sha256: "dbb9676ac9ebebe3aa3ef563cd27a87f6357a836ab24ac82f5e98380e4420ad9",
    warning: None,
};

/// Genesis Plus GX: Mega Drive / Genesis games through `/bin/md` (the module also carries
/// the Master System, Game Gear and Mega-CD launchers).
pub const GENESIS: Module = Module {
    id: "genesis",
    item: "_km_genesis_xtreme_gx",
    title: "Mega Drive core (Genesis Plus GX)",
    short: "Mega Drive / Genesis",
    purpose: "The Genesis Plus GX core.",
    binaries: &["md"],
    len: 1_651_776,
    sha256: "ed11d594accecdd2da4a280af2872a86a2cd602e0852cf58d2150c74fec85f59",
    warning: None,
};

/// Gambatte: Game Boy and Game Boy Color games through `/bin/gb` and `/bin/gbc`.
pub const GAMBATTE: Module = Module {
    id: "gambatte",
    item: "_km_gambatte",
    title: "Game Boy core (Gambatte)",
    short: "Game Boy and Color",
    purpose: "The Gambatte core.",
    binaries: &["gb", "gbc"],
    len: 302_078,
    sha256: "c1b6167db93c8ec7278018c8ebc11daf803428897c2e9feb7afd7f81e38d6f3a",
    warning: None,
};

/// mGBA: Game Boy Advance games through `/bin/gba`.
pub const MGBA: Module = Module {
    id: "mgba",
    item: "_km_mgba",
    title: "Game Boy Advance core (mGBA)",
    short: "Game Boy Advance",
    purpose: "The mGBA core.",
    binaries: &["gba"],
    len: 261_366,
    sha256: "a0a88af35872aec3a1e47156745b41a90f2d56d888eaf273cacccf41b285c5be",
    warning: None,
};

/// Mupen64Plus (KMFD's Xtreme build): N64 games through `/bin/n64`. The console's chip is
/// too slow for most of the library.
pub const MUPEN64: Module = Module {
    id: "mupen64",
    item: "_km_mupen64_xtreme",
    title: "N64 core (Mupen64Plus)",
    short: "N64",
    purpose: "The Mupen64Plus core.",
    binaries: &["n64"],
    len: 1_357_651,
    sha256: "9d17435d1ead76887009679ef51ad5e193aaeb5035f57dfa3cc8123b351e66d5",
    warning: Some(
        "N64 emulation is too heavy for this console's chip. A few lighter games are playable; \
         most run slowly, stutter or show graphics and sound glitches, and some do not start. \
         Install it at your own risk.",
    ),
};

/// Every module Cartload knows, RetroArch first.
pub const CATALOG: &[&Module] = &[
    &RETROARCH, &FCEUMM, &SNES9X, &GENESIS, &GAMBATTE, &MGBA, &MUPEN64,
];

/// The module with this id or hub item name.
pub fn find(id: &str) -> Option<&'static Module> {
    CATALOG
        .iter()
        .copied()
        .find(|m| m.id == id || m.item == id || format!("{}.hmod", m.item) == id)
}

/// `~/.cache/cartload/mods` (platform equivalent elsewhere).
pub fn default_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("cartload").join("mods"))
}

/// A module as uploaded to the console: the hub's gzip tar, unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModArchive {
    /// Cartload's id.
    pub id: String,
    /// The hmod name (`_km_fceumm_xtreme`): the directory `pack_install` takes and the name
    /// of the console's `uninstall-<name>` record.
    pub name: String,
    /// Human name, for messages.
    pub title: String,
    /// The `.hmod` file, a gzip tar.
    pub bytes: Vec<u8>,
    /// Binaries that must be on the `PATH` afterwards.
    pub binaries: Vec<String>,
}

impl Module {
    /// Where the module's file is cached.
    pub fn cached_path(&self, cache_root: &Path) -> PathBuf {
        cache_root.join(format!("{}.hmod", self.item))
    }

    /// URL of the hub's `link` file (the download location) for this module.
    pub fn link_url(&self) -> String {
        format!("{HUB_URL}{}.hmod/link", self.item)
    }

    /// URL of the mirrored copy of the pinned file.
    pub fn mirror_url(&self) -> String {
        format!("{MIRROR_URL}{}.hmod", self.item)
    }

    /// Whether `bytes` are the pinned file.
    pub fn is_pinned(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.len && crate::pinned::sha256_hex(bytes) == self.sha256
    }

    /// Whether the cache holds the pinned file (size first, then the SHA-256).
    pub fn is_cached(&self, cache_root: &Path) -> bool {
        let path = self.cached_path(cache_root);
        std::fs::metadata(&path).is_ok_and(|m| m.len() == self.len)
            && crate::pinned::file_sha256(&path).is_ok_and(|sha| sha == self.sha256)
    }

    /// Size of the cached file, when it exists.
    pub fn cached_size(&self, cache_root: &Path) -> Option<u64> {
        std::fs::metadata(self.cached_path(cache_root))
            .ok()
            .map(|m| m.len())
    }

    /// Reads the cached module; it must be the pinned file.
    pub fn load(&self, cache_root: &Path) -> Result<ModArchive> {
        let path = self.cached_path(cache_root);
        let bytes = std::fs::read(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                Error::ModuleNotCached {
                    name: self.id.to_string(),
                    path: path.clone(),
                }
            } else {
                Error::Io {
                    op: "read",
                    path: path.clone(),
                    source,
                }
            }
        })?;
        if !self.is_pinned(&bytes) {
            return Err(Error::ModuleChecksum {
                name: self.id.to_string(),
                path,
                expected: self.sha256.to_string(),
                actual: crate::pinned::sha256_hex(&bytes),
            });
        }
        Ok(ModArchive {
            id: self.id.to_string(),
            name: self.item.to_string(),
            title: self.title.to_string(),
            bytes,
            binaries: self.binaries.iter().map(|b| (*b).to_string()).collect(),
        })
    }
}

/// One file of a module as it lands on the console: its path from `/` and its MD5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSum {
    /// The path on the console (`/etc/libretro/core/mgba_libretro.so`).
    pub path: String,
    /// Lowercase hex MD5.
    pub md5: String,
}

impl ModArchive {
    /// The `count` largest programs the module puts on the console (`bin/`, `usr/bin/`,
    /// `*.so`), with their MD5s: what differs between builds, unlike fonts and artwork, and
    /// never a settings file a user may have edited on the console. A module without
    /// programs gives its largest files. `pack_install` copies every file but the package's
    /// own `install`, `uninstall` and readme to the same path under the root.
    pub fn fingerprint(&self, count: usize) -> Result<Vec<FileSum>> {
        let archive_error = |source| Error::Archive {
            path: PathBuf::from(format!("{}.hmod", self.name)),
            source,
        };
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(self.bytes.as_slice()));
        let mut files: Vec<(u64, FileSum)> = Vec::new();
        for entry in archive.entries().map_err(archive_error)? {
            let mut entry = entry.map_err(archive_error)?;
            if entry.header().entry_type() != tar::EntryType::Regular {
                continue;
            }
            let path = entry
                .path()
                .map_err(archive_error)?
                .to_string_lossy()
                .trim_start_matches("./")
                .to_string();
            if !path.contains('/') {
                // `install`, `uninstall`, `readme.md`: the package's, not the console's.
                continue;
            }
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut data).map_err(archive_error)?;
            files.push((
                data.len() as u64,
                FileSum {
                    path: format!("/{path}"),
                    md5: md5_hex(&data),
                },
            ));
        }
        let program = |path: &str| {
            path.starts_with("/bin/") || path.starts_with("/usr/bin/") || path.ends_with(".so")
        };
        if files.iter().any(|(_, f)| program(&f.path)) {
            files.retain(|(_, f)| program(&f.path));
        }
        files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
        Ok(files.into_iter().take(count).map(|(_, f)| f).collect())
    }
}

/// Lowercase hex MD5 of `data`.
pub fn md5_hex(data: &[u8]) -> String {
    crate::basemods::md5_hex(data)
}

/// Parses the hub's `link` file: one HTTPS URL.
pub fn parse_link(text: &str) -> Option<String> {
    let link = text.trim();
    (link.starts_with("https://") && !link.contains(char::is_whitespace)).then(|| link.to_string())
}

#[cfg(feature = "fetch")]
pub use download::{ModuleProgress, fetch, refresh};

#[cfg(feature = "fetch")]
mod download {
    use std::path::Path;

    use tokio::io::AsyncWriteExt;
    use tracing::{info, warn};

    use super::{Module, parse_link};
    use crate::{Error, Failures, Result};

    /// Download progress of one module.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ModuleProgress {
        /// Which module (index into the requested list).
        pub index: usize,
        /// Bytes received so far.
        pub done: u64,
        /// Bytes announced, when the server said.
        pub total: Option<u64>,
    }

    /// Makes sure the cache holds the pinned file of every module, downloading the missing
    /// (or different) ones from the mirror, or from the hub when the mirror fails. Each file
    /// is written to `<path>.partial`, checked against the pin and renamed; nothing is sent
    /// to the console here.
    pub async fn fetch(
        modules: &[&'static Module],
        cache_root: &Path,
        progress: impl FnMut(ModuleProgress),
    ) -> Result<()> {
        refresh(modules, cache_root, progress).await.map(|_| ())
    }

    /// Like [`fetch`], returning the ids of the modules that were downloaded.
    pub async fn refresh(
        modules: &[&'static Module],
        cache_root: &Path,
        mut progress: impl FnMut(ModuleProgress),
    ) -> Result<Vec<&'static str>> {
        let mut client = None;
        let mut downloaded = Vec::new();
        for (index, module) in modules.iter().enumerate() {
            if module.is_cached(cache_root) {
                continue;
            }
            let client = match &client {
                Some(client) => client,
                None => client.insert(crate::fetch::client()?),
            };
            fetch_one(client, module, cache_root, |done, total| {
                progress(ModuleProgress { index, done, total });
            })
            .await?;
            downloaded.push(module.id);
        }
        Ok(downloaded)
    }

    async fn text(client: &reqwest::Client, url: &str) -> Result<String> {
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|source| Error::Download {
                url: url.to_string(),
                source,
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }
        response.text().await.map_err(|source| Error::Download {
            url: url.to_string(),
            source,
        })
    }

    /// The mirror first, then the hub's `link`; the first download with the pinned hash wins.
    async fn fetch_one(
        client: &reqwest::Client,
        module: &'static Module,
        cache_root: &Path,
        mut progress: impl FnMut(u64, Option<u64>),
    ) -> Result<()> {
        let dest = module.cached_path(cache_root);
        tokio::fs::create_dir_all(cache_root)
            .await
            .map_err(|source| Error::Io {
                op: "create directory",
                path: cache_root.to_path_buf(),
                source,
            })?;
        let mut partial = dest.as_os_str().to_owned();
        partial.push(".partial");
        let partial = std::path::PathBuf::from(partial);
        let mut failures = Failures::default();
        for source in [Source::Mirror, Source::Hub] {
            let url = match source {
                Source::Mirror => module.mirror_url(),
                Source::Hub => match text(client, &module.link_url()).await {
                    Ok(link_text) => match parse_link(&link_text) {
                        Some(url) => url,
                        None => {
                            failures.0.push((
                                module.link_url(),
                                Error::ModuleLink {
                                    name: module.id.to_string(),
                                    link: link_text.trim().to_string(),
                                },
                            ));
                            continue;
                        }
                    },
                    Err(error) => {
                        failures.0.push((module.link_url(), error));
                        continue;
                    }
                },
            };
            let bytes = match download(client, &url, &partial, &mut progress).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    let _ = tokio::fs::remove_file(&partial).await;
                    warn!(module = module.id, url, %error, "module download failed");
                    failures.0.push((url, error));
                    continue;
                }
            };
            if !module.is_pinned(&bytes) {
                let _ = tokio::fs::remove_file(&partial).await;
                let error = Error::ModuleChecksum {
                    name: module.id.to_string(),
                    path: dest.clone(),
                    expected: module.sha256.to_string(),
                    actual: crate::pinned::sha256_hex(&bytes),
                };
                warn!(module = module.id, url, %error, "module download is not the pinned file");
                failures.0.push((url, error));
                continue;
            }
            tokio::fs::rename(&partial, &dest)
                .await
                .map_err(|source| Error::Io {
                    op: "rename",
                    path: partial.clone(),
                    source,
                })?;
            info!(
                module = module.id,
                url,
                path = %dest.display(),
                len = bytes.len(),
                sha256 = module.sha256,
                "downloaded module"
            );
            return Ok(());
        }
        Err(Error::AllSourcesFailed(failures))
    }

    #[derive(Clone, Copy)]
    enum Source {
        Mirror,
        Hub,
    }

    /// Streams `url` to `partial` and returns the bytes (modules are small enough to hold).
    async fn download(
        client: &reqwest::Client,
        url: &str,
        partial: &Path,
        progress: &mut impl FnMut(u64, Option<u64>),
    ) -> Result<Vec<u8>> {
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|source| Error::Download {
                url: url.to_string(),
                source,
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus {
                url: url.to_string(),
                status: status.as_u16(),
            });
        }
        let total = response.content_length();
        let mut file = tokio::fs::File::create(partial)
            .await
            .map_err(|source| Error::Io {
                op: "create",
                path: partial.to_path_buf(),
                source,
            })?;
        let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
        progress(0, total);
        while let Some(chunk) = response.chunk().await.map_err(|source| Error::Download {
            url: url.to_string(),
            source,
        })? {
            file.write_all(&chunk).await.map_err(|source| Error::Io {
                op: "write",
                path: partial.to_path_buf(),
                source,
            })?;
            bytes.extend_from_slice(&chunk);
            progress(bytes.len() as u64, total);
        }
        if let Some(total) = total
            && bytes.len() as u64 != total
        {
            return Err(Error::Truncated {
                url: url.to_string(),
                done: bytes.len() as u64,
                total,
            });
        }
        file.sync_all().await.map_err(|source| Error::Io {
            op: "sync",
            path: partial.to_path_buf(),
            source,
        })?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_and_items_are_unique_and_findable() {
        let mut ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        assert_eq!(find("fceumm").map(|m| m.item), Some("_km_fceumm_xtreme"));
        assert_eq!(find("_km_fceumm_xtreme.hmod").map(|m| m.id), Some("fceumm"));
        assert!(find("mame").is_none());
        assert_eq!(
            RETROARCH.link_url(),
            "https://hakchi.net/KMFDManic/NESC-SNESC-Modifications/.repo/_km_retroarch_1910_ozone.hmod/link"
        );
    }

    #[test]
    fn hub_files_parse() {
        assert_eq!(
            parse_link("https://hakchi.net/x/y.hmod\n"),
            Some("https://hakchi.net/x/y.hmod".to_string())
        );
        assert_eq!(parse_link("http://insecure/y.hmod"), None);
        assert_eq!(parse_link(""), None);
    }

    #[test]
    fn fingerprint_takes_the_largest_programs() {
        let mut tar = tar::Builder::new(Vec::new());
        for (path, data) in [
            ("install", vec![b'x'; 900]),
            ("readme.md", vec![b'r'; 800]),
            ("bin/gba", b"#!/bin/sh\n".to_vec()),
            ("etc/libretro/core/mgba_libretro.so", vec![1; 500]),
            ("etc/libretro/info/mgba_libretro.info", vec![2; 50]),
            ("etc/libretro/assets/font.ttf", vec![3; 5000]),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, path, data.as_slice())
                .expect("append");
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &tar.into_inner().expect("tar")).expect("gzip");
        let archive = ModArchive {
            id: "mgba".to_string(),
            name: "_km_mgba".to_string(),
            title: "mGBA".to_string(),
            bytes: gz.finish().expect("gzip"),
            binaries: vec!["gba".to_string()],
        };
        let sums = archive.fingerprint(2).expect("fingerprint");
        assert_eq!(
            sums.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            ["/etc/libretro/core/mgba_libretro.so", "/bin/gba"]
        );
        assert_eq!(sums[0].md5, md5_hex(&[1; 500]));
        // Programs only: not the font, the info file or the package's own files.
        assert_eq!(archive.fingerprint(10).expect("all").len(), 2);
    }

    #[test]
    fn load_reads_the_cache_or_says_what_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = FCEUMM.load(dir.path()).expect_err("not cached");
        assert!(matches!(error, Error::ModuleNotCached { ref name, .. } if name == "fceumm"));
        assert!(!FCEUMM.is_cached(dir.path()));
        std::fs::write(FCEUMM.cached_path(dir.path()), b"gz bytes").expect("write");
        assert!(!FCEUMM.is_cached(dir.path()), "not the pinned file");
        assert_eq!(FCEUMM.cached_size(dir.path()), Some(8));
        let error = FCEUMM.load(dir.path()).expect_err("not pinned");
        assert!(matches!(error, Error::ModuleChecksum { .. }), "{error}");

        // A module pinned to these bytes loads.
        let module = Module {
            len: 8,
            sha256: Box::leak(crate::pinned::sha256_hex(b"gz bytes").into_boxed_str()),
            ..FCEUMM
        };
        assert!(module.is_cached(dir.path()));
        let archive = module.load(dir.path()).expect("load");
        assert_eq!(archive.name, "_km_fceumm_xtreme");
        assert_eq!(archive.binaries, ["fceumm"]);
        assert_eq!(archive.bytes, b"gz bytes");
    }

    #[test]
    fn every_module_is_pinned_and_mirrored() {
        for module in CATALOG {
            assert_eq!(module.sha256.len(), 64, "{}", module.id);
            assert!(module.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(module.len > 0);
            assert_eq!(
                module.mirror_url(),
                format!(
                    "https://github.com/AOShei/Cartload/releases/download/{MIRROR_TAG}/{}.hmod",
                    module.item
                )
            );
        }
    }
}
