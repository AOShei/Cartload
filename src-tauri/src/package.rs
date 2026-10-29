//! The kernel package the Console page installs: which one, where it is, which version it is,
//! and downloading a fresh copy into the cache.
//!
//! Two packages are built into the app (`cartload_hmod::pinned`): Cartload's own system (the
//! default) and hakchi v1.0.4-126 (hakchi2 CE's, the way back to hakchi's own system). The
//! frontend names one by its id with each call. Resolution order: the `CARTLOAD_HMOD`
//! environment variable (a development override pointing at any package file), then the cached
//! copy (`<cache dir>/cartload/<the package's file>`). The app writes the built-in package to
//! the cache whenever the cached file is missing or is anything else, so installing needs no
//! network. The cached file is only used when its SHA-256 is the pinned one. For hakchi's
//! package, the download from the locations hakchi2 CE uses remains as a fallback, pinned too.

use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use cartload_flow::HmodPackage;
use cartload_hmod::{
    DOWNLOAD_URLS, Package, Progress, fetch_from,
    pinned::{self, PinnedPackage},
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::CommandError;

/// Environment variable naming a package file to use instead of the cache.
pub const OVERRIDE_ENV: &str = "CARTLOAD_HMOD";

/// The package with this id; the default (Cartload's system) without one.
pub fn chosen(id: Option<&str>) -> Result<&'static PinnedPackage, CommandError> {
    match id {
        None => Ok(pinned::PACKAGES[0]),
        Some(id) => PinnedPackage::by_id(id)
            .ok_or_else(|| CommandError::new(format!("There is no kernel package \"{id}\"."))),
    }
}

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
    /// `cartloadVersion`, in Cartload's own system.
    pub system: Option<String>,
    /// `bootVersion`.
    pub boot: Option<String>,
    /// `kernelVersion`.
    pub kernel: Option<String>,
    /// Size in bytes.
    pub len: u64,
    /// Modification time of the file, unix milliseconds.
    pub modified: Option<u64>,
}

/// A package the user can choose.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageChoice {
    /// [`PinnedPackage::id`].
    pub id: String,
    /// [`PinnedPackage::label`].
    pub label: String,
}

/// What the Console page shows about the package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageStatus {
    /// The id of the package asked for.
    pub choice: String,
    /// The packages there are, the default first.
    pub choices: Vec<PackageChoice>,
    /// The package that would be installed, when one is usable.
    pub package: Option<PackageInfo>,
    /// Where a download goes.
    pub cache_path: Option<String>,
    /// Download locations, in order (none for a package only built in).
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

/// The package's cached copy, when the system has a cache directory.
pub fn cache_path(package: &PinnedPackage) -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("cartload").join(package.cache_file))
}

/// Download locations of a package: hakchi2 CE's for hakchi's, none for Cartload's (built in).
fn download_urls(package: &PinnedPackage) -> &'static [&'static str] {
    if package == &pinned::HAKCHI {
        &DOWNLOAD_URLS
    } else {
        &[]
    }
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

fn default_candidates(package: &PinnedPackage) -> Vec<(PackageSource, PathBuf)> {
    candidates(
        std::env::var_os(OVERRIDE_ENV).map(PathBuf::from),
        cache_path(package),
    )
}

/// Writes the built-in package to the cache when the cached file is missing or not the pinned
/// one. A failure is logged and leaves the cache as it was. Blocking.
pub fn prepare_cache(package: &PinnedPackage, cache: Option<&Path>) {
    let Some(cache) = cache else {
        return;
    };
    match package.install_bundled(cache) {
        Ok(true) => {
            info!(path = %cache.display(), package = package.label, "built-in kernel package written to the cache")
        }
        Ok(false) => {}
        Err(error) => {
            warn!(path = %cache.display(), %error, "could not write the built-in kernel package")
        }
    }
}

/// Opens and validates a package file; a cached file must also be the pinned package (the
/// override may be any package). Blocking: it hashes the file and decompresses the archive.
pub fn inspect(
    path: &Path,
    source: PackageSource,
    package: &PinnedPackage,
) -> cartload_hmod::Result<PackageInfo> {
    if source == PackageSource::Cached {
        package.check(path)?;
    }
    let opened = Package::open(path)?;
    let version = opened.validate()?;
    let metadata = std::fs::metadata(path).ok();
    Ok(PackageInfo {
        path: path.display().to_string(),
        source,
        version: version.hakchi,
        system: version.cartload,
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
pub fn status_from(
    package: &PinnedPackage,
    candidates: &[(PackageSource, PathBuf)],
    cache: Option<&Path>,
) -> PackageStatus {
    let mut problem = None;
    let mut found = None;
    for (source, path) in candidates {
        if !path.is_file() {
            continue;
        }
        match inspect(path, *source, package) {
            Ok(info) => {
                found = Some(info);
                break;
            }
            Err(error) => {
                warn!(path = %path.display(), %error, "unusable kernel package");
                problem.get_or_insert_with(|| error.to_string());
            }
        }
    }
    PackageStatus {
        choice: package.id.to_string(),
        choices: pinned::PACKAGES
            .iter()
            .map(|p| PackageChoice {
                id: p.id.to_string(),
                label: p.label.to_string(),
            })
            .collect(),
        package: found,
        cache_path: cache.map(|p| p.display().to_string()),
        sources: download_urls(package)
            .iter()
            .map(|url| (*url).to_string())
            .collect(),
        problem,
    }
}

/// The current status of a package.
pub async fn status(package: &'static PinnedPackage) -> PackageStatus {
    let candidates = default_candidates(package);
    let cache = cache_path(package);
    tokio::task::spawn_blocking(move || {
        prepare_cache(package, cache.as_deref());
        status_from(package, &candidates, cache.as_deref())
    })
    .await
    .unwrap_or_else(|join| PackageStatus {
        choice: package.id.to_string(),
        choices: Vec::new(),
        package: None,
        cache_path: None,
        sources: Vec::new(),
        problem: Some(format!("could not inspect the package: {join}")),
    })
}

/// Loads the first usable copy of a package for a flow. `None` when there is none; problems are
/// logged.
pub async fn load_for_flow(package: &'static PinnedPackage) -> Option<HmodPackage> {
    let candidates = default_candidates(package);
    let cache = cache_path(package);
    tokio::task::spawn_blocking(move || {
        prepare_cache(package, cache.as_deref());
        for (source, path) in candidates {
            if !path.is_file() {
                continue;
            }
            if source == PackageSource::Cached
                && let Err(error) = package.check(&path)
            {
                warn!(%error, "unusable kernel package");
                continue;
            }
            match HmodPackage::load(&path) {
                Ok(loaded) => {
                    info!(path = %path.display(), ?source, version = %loaded.files.version, "kernel package loaded");
                    return Some(loaded);
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

/// Downloads a package into the cache, reporting progress, and returns the new status. The
/// download is checked against the pinned SHA-256 before it replaces any earlier copy. A
/// fallback for when the built-in copy cannot be written; only hakchi's package has download
/// locations.
pub async fn update(
    package: &'static PinnedPackage,
    progress: impl FnMut(Progress) + Send,
) -> Result<PackageStatus, CommandError> {
    let urls = download_urls(package);
    if urls.is_empty() {
        return Err(CommandError::new(format!(
            "{} is built into Cartload and has no download location.",
            package.label
        )));
    }
    let dest = cache_path(package).ok_or_else(|| {
        CommandError::new("This system has no cache directory to download the package into.")
    })?;
    let fetched = fetch_from(urls, &dest, Some(package.sha256), progress)
        .await
        .map_err(|error| {
            CommandError::from_error(&error)
                .with_message(format!("The package could not be downloaded: {error}"))
        })?;
    info!(url = fetched.url, len = fetched.len, version = %fetched.version, "kernel package updated");
    Ok(status(package).await)
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
            &pinned::HAKCHI,
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
        assert_eq!(status.choice, "hakchi");
        assert_eq!(status.choices[0].id, "cartload");
        assert_eq!(status.cache_path.as_deref(), Some(good.to_str().unwrap()));

        let status = status_from(&pinned::CARTLOAD, &[(PackageSource::Cached, missing)], None);
        assert!(status.sources.is_empty());
        assert!(status.package.is_none());
        assert!(status.problem.is_none());
    }

    #[test]
    fn the_cache_holds_only_the_pinned_package() {
        let dir = tempfile::tempdir().unwrap();
        for package in pinned::PACKAGES {
            let cache = dir.path().join("cartload").join(package.cache_file);

            // A valid package that is not the pinned one is refused from the cache...
            std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
            std::fs::write(&cache, sample_package()).unwrap();
            let status = status_from(
                package,
                &[(PackageSource::Cached, cache.clone())],
                Some(&cache),
            );
            assert!(status.package.is_none());
            assert!(
                status.problem.as_deref().unwrap().contains(package.label),
                "{:?}",
                status.problem
            );

            // ...and replaced by the built-in one.
            prepare_cache(package, Some(&cache));
            let status = status_from(
                package,
                &[(PackageSource::Cached, cache.clone())],
                Some(&cache),
            );
            let found = status.package.expect("the built-in package");
            assert_eq!(found.version, package.hakchi_version);
            assert_eq!(found.system.as_deref(), package.system_version);
            assert_eq!(found.len, package.len);
            assert!(status.problem.is_none());
        }
    }

    #[test]
    fn the_default_is_cartloads_system() {
        assert_eq!(chosen(None).unwrap(), &pinned::CARTLOAD);
        assert_eq!(chosen(Some("hakchi")).unwrap(), &pinned::HAKCHI);
        assert!(chosen(Some("nothing")).is_err());
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
