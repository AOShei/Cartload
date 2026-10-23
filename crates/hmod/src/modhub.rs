//! Emulator modules from KMFD's Mod Hub, the repository hakchi2 CE ships as its default
//! (`https://hakchi.net/KMFDManic/NESC-SNESC-Modifications/.repo/`): RetroArch and the
//! libretro cores Cartload knows how to use. The hub is a plain directory tree: `<item>/link`
//! holds the download URL of the `.hmod` (a gzip tar), `<item>/md5` its checksum, as hakchi2
//! CE's `ModHub.Repository` reads them. Downloads are cached under the user's cache
//! directory and verified against the hub's MD5 before use.
//!
//! The catalog is deliberately short: the three modules that make NES and SNES games run
//! through RetroArch. Other systems are the same mechanism with another core.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// The hub's `.repo/` directory.
pub const HUB_URL: &str = "https://hakchi.net/KMFDManic/NESC-SNESC-Modifications/.repo/";

/// One module Cartload can install.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module {
    /// Cartload's short id (`retroarch`, `fceumm`, `snes9x`).
    pub id: &'static str,
    /// The hub item, also the hmod name the console registers it under.
    pub item: &'static str,
    /// Human name.
    pub title: &'static str,
    /// What it is for, one sentence.
    pub purpose: &'static str,
    /// Binaries the module puts on the console's `PATH`; checked after the install.
    pub binaries: &'static [&'static str],
    /// Rough download size, for the UI before the download starts.
    pub approx_bytes: u64,
}

/// RetroArch itself: the frontend, its launchers (`retroarch-clover`, `/bin/nes`, ...) and
/// no cores.
pub const RETROARCH: Module = Module {
    id: "retroarch",
    item: "_km_retroarch_1910_ozone",
    title: "RetroArch 1.9.10",
    purpose: "The frontend every core runs in.",
    binaries: &["retroarch-clover", "nes"],
    approx_bytes: 10_600_000,
};

/// The NES core hakchi's launcher script and `/bin/nes` use.
pub const FCEUMM: Module = Module {
    id: "fceumm",
    item: "_km_fceumm_xtreme",
    title: "NES core (FCEUmm)",
    purpose: "Plays NES games through RetroArch.",
    binaries: &["fceumm"],
    approx_bytes: 760_000,
};

/// A SNES core for games the built-in emulator does not run well.
pub const SNES9X: Module = Module {
    id: "snes9x",
    item: "_km_snes9x_xtreme",
    title: "SNES core (Snes9x)",
    purpose: "Plays SNES games through RetroArch.",
    binaries: &["snes9x", "snes"],
    approx_bytes: 3_100_000,
};

/// Every module Cartload knows, RetroArch first.
pub const CATALOG: &[&Module] = &[&RETROARCH, &FCEUMM, &SNES9X];

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

    /// URL of the hub's `md5` file for this module.
    pub fn md5_url(&self) -> String {
        format!("{HUB_URL}{}.hmod/md5", self.item)
    }

    /// Whether the module is cached (no checksum: the hub's MD5 is only known online and was
    /// checked when the file was written).
    pub fn is_cached(&self, cache_root: &Path) -> bool {
        self.cached_path(cache_root).is_file()
    }

    /// Size of the cached file, when it exists.
    pub fn cached_size(&self, cache_root: &Path) -> Option<u64> {
        std::fs::metadata(self.cached_path(cache_root))
            .ok()
            .map(|m| m.len())
    }

    /// Reads the cached module.
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
        Ok(ModArchive {
            id: self.id.to_string(),
            name: self.item.to_string(),
            title: self.title.to_string(),
            bytes,
            binaries: self.binaries.iter().map(|b| (*b).to_string()).collect(),
        })
    }
}

/// Lowercase hex MD5 of `data`.
pub fn md5_hex(data: &[u8]) -> String {
    crate::basemods::md5_hex(data)
}

/// Parses the hub's `md5` file: the first 32 hex digits on it.
pub fn parse_md5(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    let token = token.to_ascii_lowercase();
    (token.len() == 32 && token.chars().all(|c| c.is_ascii_hexdigit())).then_some(token)
}

/// Parses the hub's `link` file: one HTTPS URL.
pub fn parse_link(text: &str) -> Option<String> {
    let link = text.trim();
    (link.starts_with("https://") && !link.contains(char::is_whitespace)).then(|| link.to_string())
}

#[cfg(feature = "fetch")]
pub use download::{ModuleProgress, fetch};

#[cfg(feature = "fetch")]
mod download {
    use std::path::Path;

    use tokio::io::AsyncWriteExt;
    use tracing::info;

    use super::{Module, md5_hex, parse_link, parse_md5};
    use crate::{Error, Result};

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

    /// Makes sure every module is cached, downloading the missing ones from the hub (the
    /// `link` file gives the location, the `md5` file the checksum the download must have).
    /// Each file is written to `<path>.partial`, verified and renamed; nothing is sent to the
    /// console here.
    pub async fn fetch(
        modules: &[&'static Module],
        cache_root: &Path,
        mut progress: impl FnMut(ModuleProgress),
    ) -> Result<()> {
        let mut client = None;
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
        }
        Ok(())
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

    async fn fetch_one(
        client: &reqwest::Client,
        module: &'static Module,
        cache_root: &Path,
        mut progress: impl FnMut(u64, Option<u64>),
    ) -> Result<()> {
        let link_text = text(client, &module.link_url()).await?;
        let url = parse_link(&link_text).ok_or_else(|| Error::ModuleLink {
            name: module.id.to_string(),
            link: link_text.trim().to_string(),
        })?;
        let expected = parse_md5(&text(client, &module.md5_url()).await?);
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
        let result = download(client, &url, &partial, &mut progress).await;
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(error);
            }
        };
        let actual = md5_hex(&bytes);
        if let Some(expected) = &expected
            && expected != &actual
        {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(Error::ModuleChecksum {
                name: module.id.to_string(),
                path: dest,
                expected: expected.clone(),
                actual,
            });
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
            md5 = actual,
            verified = expected.is_some(),
            "downloaded module"
        );
        Ok(())
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
            parse_md5("b57f77cc7693a74b1b0dddc720f00270  _km_retroarch_1910_ozone.hmod\n"),
            Some("b57f77cc7693a74b1b0dddc720f00270".to_string())
        );
        assert_eq!(parse_md5("not a sum"), None);
        assert_eq!(
            parse_link("https://hakchi.net/x/y.hmod\n"),
            Some("https://hakchi.net/x/y.hmod".to_string())
        );
        assert_eq!(parse_link("http://insecure/y.hmod"), None);
        assert_eq!(parse_link(""), None);
    }

    #[test]
    fn load_reads_the_cache_or_says_what_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = FCEUMM.load(dir.path()).expect_err("not cached");
        assert!(matches!(error, Error::ModuleNotCached { ref name, .. } if name == "fceumm"));
        assert!(!FCEUMM.is_cached(dir.path()));
        std::fs::write(FCEUMM.cached_path(dir.path()), b"gz bytes").expect("write");
        assert!(FCEUMM.is_cached(dir.path()));
        assert_eq!(FCEUMM.cached_size(dir.path()), Some(8));
        let archive = FCEUMM.load(dir.path()).expect("load");
        assert_eq!(archive.name, "_km_fceumm_xtreme");
        assert_eq!(archive.binaries, ["fceumm"]);
        assert_eq!(archive.bytes, b"gz bytes");
    }
}
