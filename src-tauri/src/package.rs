//! The kernel package (`hakchi.hmod`) the Console page installs: where it is, which version it
//! is, and downloading a fresh copy into the cache.
//!
//! Resolution order: the `CARTLOAD_HMOD` environment variable (a development override pointing
//! at any package file), then the cached copy (`<cache dir>/cartload/hakchi.hmod`). The app
//! carries the one package it installs, hakchi v1.0.4-126 (`cartload_hmod::pinned`), and writes
//! it to the cache whenever the cached file is missing or is anything else, so installing hakchi
//! needs no network. The cached file is only used when its SHA-256 is the pinned one; the
//! download from the locations hakchi2 CE uses remains as a fallback and is pinned too.

use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use cartload_flow::HmodPackage;
use cartload_hmod::{DOWNLOAD_URLS, Package, Progress, fetch_from, pinned};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::CommandError;

/// Environment variable naming a package file to use instead of the cache.
pub const OVERRIDE_ENV: &str = "CARTLOAD_HMOD";
/// File name of the cached download.
pub const CACHE_FILE: &str = "hakchi.hmod";

/// Where a package came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageSource {
    /// `CARTLOAD_HMOD`.
    Override,
    /// The cached copy: the built-in package written out, or a download of the same bytes.
    Cached,
}

/// A usable package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Where it is.
    pub path: String,
    /// How it was found.
    pub source: PackageSource,
    /// `hakchiVersion`, for example `v1.0.4-126`.
    pub version: String,
    /// `bootVersion`.
    pub boot: Option<String>,
    /// `kernelVersion`.
    pub kernel: Option<String>,
    /// Size in bytes.
    pub len: u64,
    /// Modification time of the file, unix milliseconds.
    pub modified: Option<u64>,
}

/// What the Console page shows about the package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageStatus {
    /// The package that would be installed, when one is usable.
    pub package: Option<PackageInfo>,
    /// Where a download goes.
    pub cache_path: Option<String>,
    /// Download locations, in order.
    pub sources: Vec<String>,
    /// A candidate file exists but cannot be used.
    pub problem: Option<String>,
}

/// Download progress, as emitted with the `package-download` event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Bytes received.
    pub done: u64,
    /// Total size when the server announced one.
    pub total: Option<u64>,
}

impl From<Progress> for DownloadProgress {
    fn from(progress: Progress) -> Self {
        Self {
            done: progress.done,
            total: progress.total,
        }
    }
}

/// The cached download's path, when the system has a cache directory.
pub fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("cartload").join(CACHE_FILE))
}

/// The package files to try, in order.
pub fn candidates(
    override_path: Option<PathBuf>,
    cache: Option<PathBuf>,
) -> Vec<(PackageSource, PathBuf)> {
    let mut candidates = Vec::new();
    if let Some(path) = override_path.filter(|p| !p.as_os_str().is_empty()) {
        candidates.push((PackageSource::Override, path));
    }
    if let Some(path) = cache {
        candidates.push((PackageSource::Cached, path));
    }
    candidates
}

fn default_candidates() -> Vec<(PackageSource, PathBuf)> {
    candidates(
        std::env::var_os(OVERRIDE_ENV).map(PathBuf::from),
        cache_path(),
    )
}

/// Writes the built-in package to the cache when the cached file is missing or not the pinned
/// one. A failure is logged and leaves the cache as it was. Blocking.
pub fn prepare_cache(cache: Option<&Path>) {
    let Some(cache) = cache else {
        return;
    };
    match pinned::install_bundled(cache) {
        Ok(true) => {
            info!(path = %cache.display(), version = pinned::PINNED_VERSION, "built-in kernel package written to the cache")
        }
        Ok(false) => {}
        Err(error) => {
            warn!(path = %cache.display(), %error, "could not write the built-in kernel package")
        }
    }
}

/// Opens and validates a package file; a cached file must also be the pinned package (the
/// override may be any package). Blocking: it hashes the file and decompresses the archive.
pub fn inspect(path: &Path, source: PackageSource) -> cartload_hmod::Result<PackageInfo> {
    if source == PackageSource::Cached {
        pinned::check_pinned(path)?;
    }
    let package = Package::open(path)?;
    let version = package.validate()?;
    let metadata = std::fs::metadata(path).ok();
    Ok(PackageInfo {
        path: path.display().to_string(),
        source,
        version: version.hakchi,
        boot: version.boot,
        kernel: version.kernel,
        len: metadata.as_ref().map_or(0, std::fs::Metadata::len),
        modified: metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
    })
}

/// The first usable candidate, with the problem of any unusable file on the way. Blocking.
pub fn status_from(candidates: &[(PackageSource, PathBuf)], cache: Option<&Path>) -> PackageStatus {
    let mut problem = None;
    let mut package = None;
    for (source, path) in candidates {
        if !path.is_file() {
            continue;
        }
        match inspect(path, *source) {
            Ok(info) => {
                package = Some(info);
                break;
            }
            Err(error) => {
                warn!(path = %path.display(), %error, "unusable kernel package");
                problem.get_or_insert_with(|| error.to_string());
            }
        }
    }
    PackageStatus {
        package,
        cache_path: cache.map(|p| p.display().to_string()),
        sources: DOWNLOAD_URLS.iter().map(|url| (*url).to_string()).collect(),
        problem,
    }
}

/// The current package status.
pub async fn status() -> PackageStatus {
    let candidates = default_candidates();
    let cache = cache_path();
    tokio::task::spawn_blocking(move || {
        prepare_cache(cache.as_deref());
        status_from(&candidates, cache.as_deref())
    })
    .await
    .unwrap_or_else(|join| PackageStatus {
        package: None,
        cache_path: None,
        sources: Vec::new(),
        problem: Some(format!("could not inspect the package: {join}")),
    })
}

/// Loads the first usable package for a flow. `None` when there is none; problems are logged.
pub async fn load_for_flow() -> Option<HmodPackage> {
    let candidates = default_candidates();
    let cache = cache_path();
    tokio::task::spawn_blocking(move || {
        prepare_cache(cache.as_deref());
        for (source, path) in candidates {
            if !path.is_file() {
                continue;
            }
            if source == PackageSource::Cached
                && let Err(error) = pinned::check_pinned(&path)
            {
                warn!(%error, "unusable kernel package");
                continue;
            }
            match HmodPackage::load(&path) {
                Ok(package) => {
                    info!(path = %path.display(), ?source, version = %package.files.version, "kernel package loaded");
                    return Some(package);
                }
                Err(error) => warn!(path = %path.display(), %error, "unusable kernel package"),
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// Downloads the pinned package into the cache, reporting progress, and returns the new
/// status. The download is checked against the pinned SHA-256 before it replaces any earlier
/// copy. A fallback for when the built-in copy cannot be written.
pub async fn update(progress: impl FnMut(Progress) + Send) -> Result<PackageStatus, CommandError> {
    let dest = cache_path().ok_or_else(|| {
        CommandError::new("This system has no cache directory to download the package into.")
    })?;
    let fetched = fetch_from(&DOWNLOAD_URLS, &dest, Some(pinned::PINNED_SHA256), progress)
        .await
        .map_err(|error| {
            CommandError::from_error(&error)
                .with_message(format!("The package could not be downloaded: {error}"))
        })?;
    info!(url = fetched.url, len = fetched.len, version = %fetched.version, "kernel package updated");
    Ok(status().await)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::io::Write;

    use flate2::{Compression, write::GzEncoder};

    use super::*;

    /// A gzip tar with the three files a kernel package needs.
    fn sample_package() -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (name, data) in [
            ("boot/boot.img", &b"ANDROID!fake"[..]),
            ("boot/uboot.bin", &b"uboot"[..]),
            (
                "var/version",
                &b"bootVersion='1.0.3'\nhakchiVersion='v1.0.4-126'\nkernelVersion='3.4.113.29-madmonkey'\n"[..],
            ),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_path(name).unwrap();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, data).unwrap();
        }
        let mut gz = builder.into_inner().unwrap();
        gz.flush().unwrap();
        gz.finish().unwrap()
    }

    #[test]
    fn override_comes_before_the_cache_and_empty_values_are_ignored() {
        let list = candidates(
            Some("/tmp/x.hmod".into()),
            Some("/cache/hakchi.hmod".into()),
        );
        assert_eq!(list[0].0, PackageSource::Override);
        assert_eq!(list[1].0, PackageSource::Cached);
        let list = candidates(Some(PathBuf::new()), None);
        assert!(list.is_empty());
    }

    #[test]
    fn status_reports_the_first_usable_file_and_remembers_problems() {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("broken.hmod");
        std::fs::write(&broken, b"not a package").unwrap();
        let good = dir.path().join("hakchi.hmod");
        std::fs::write(&good, sample_package()).unwrap();
        let missing = dir.path().join("missing.hmod");

        let status = status_from(
            &[
                (PackageSource::Override, missing.clone()),
                (PackageSource::Override, broken.clone()),
                (PackageSource::Override, good.clone()),
            ],
            Some(&good),
        );
        let package = status.package.expect("the good file");
        assert_eq!(package.source, PackageSource::Override);
        assert_eq!(package.version, "v1.0.4-126");
        assert_eq!(package.boot.as_deref(), Some("1.0.3"));
        assert_eq!(package.kernel.as_deref(), Some("3.4.113.29-madmonkey"));
        assert_eq!(package.len, std::fs::metadata(&good).unwrap().len());
        assert!(package.modified.is_some());
        assert!(
            status.problem.as_deref().unwrap().contains("gzip"),
            "{:?}",
            status.problem
        );
        assert_eq!(status.sources.len(), 2);
        assert_eq!(status.cache_path.as_deref(), Some(good.to_str().unwrap()));

        let status = status_from(&[(PackageSource::Cached, missing)], None);
        assert!(status.package.is_none());
        assert!(status.problem.is_none());
    }

    #[test]
    fn the_cache_holds_only_the_pinned_package() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cartload").join(CACHE_FILE);

        // A valid package that is not the pinned one is refused from the cache...
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(&cache, sample_package()).unwrap();
        let status = status_from(&[(PackageSource::Cached, cache.clone())], Some(&cache));
        assert!(status.package.is_none());
        assert!(
            status.problem.as_deref().unwrap().contains("v1.0.4-126"),
            "{:?}",
            status.problem
        );

        // ...and replaced by the built-in one.
        prepare_cache(Some(&cache));
        let status = status_from(&[(PackageSource::Cached, cache.clone())], Some(&cache));
        let package = status.package.expect("the built-in package");
        assert_eq!(package.version, pinned::PINNED_VERSION);
        assert_eq!(package.len, pinned::PINNED_LEN);
        assert!(status.problem.is_none());
    }

    #[test]
    fn package_status_serialises_with_snake_case_tags() {
        let json = serde_json::to_value(PackageSource::Override).unwrap();
        assert_eq!(json, "override");
        let progress: DownloadProgress = Progress {
            done: 5,
            total: Some(10),
        }
        .into();
        assert_eq!(
            serde_json::to_value(progress).unwrap(),
            serde_json::json!({"done": 5, "total": 10})
        );
    }
}
