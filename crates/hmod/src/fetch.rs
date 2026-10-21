//! Downloading the kernel package.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use crate::{Error, Failures, Package, Result, Version};

/// The release asset hakchi2 CE ships (hakchi v1.0.4-126, April 2020). Tried first.
pub const GITHUB_URL: &str = "https://github.com/TeamShinkansen/Hakchi2-CE/releases/download/supplemental/hakchi-v1.0.4-126.Fri.03.Apr.2020.11_12_31.AM.GMT.hmod";
/// hakchi.net's "latest" package, hakchi2 CE's update location. Tried second.
pub const HAKCHI_NET_URL: &str = "https://hakchi.net/hakchi/hmods/hakchi-latest.hmod";
/// Known locations of the package, in the order [`fetch_latest`] tries them.
pub const DOWNLOAD_URLS: [&str; 2] = [GITHUB_URL, HAKCHI_NET_URL];

const USER_AGENT: &str = concat!("cartload/", env!("CARGO_PKG_VERSION"));
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Download progress, reported after every received chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Bytes received so far.
    pub done: u64,
    /// Total size announced by the server, when it sent one.
    pub total: Option<u64>,
}

/// A downloaded and verified package.
#[derive(Clone, Debug)]
pub struct Fetched {
    /// The source that delivered it.
    pub url: String,
    /// Where it was written.
    pub path: PathBuf,
    /// Its size in bytes.
    pub len: u64,
    /// Its `var/version`.
    pub version: Version,
}

/// Download the latest package into `dest` and return its hakchi version (e.g. `v1.0.4-126`).
///
/// Tries the [`DOWNLOAD_URLS`] in order. The download goes to a temporary file next to `dest`,
/// is opened and checked to contain the boot files and a version, and only then renamed over
/// `dest`. See [`fetch_from`] for progress reporting and the details.
pub async fn fetch_latest(dest: impl AsRef<Path>) -> Result<String> {
    fetch_from(&DOWNLOAD_URLS, dest.as_ref(), |_| {})
        .await
        .map(|fetched| fetched.version.hakchi)
}

/// Download a package from the first of `urls` that works, verify it and write it to `dest`.
///
/// Each source is tried in turn; a failed request, an error status, a short body or a download
/// that does not open as a kernel package (see [`Package::validate`]) moves on to the next one.
/// Nothing is left behind on failure. On success `dest` is replaced atomically: data is written
/// to `<dest>.partial` in the same directory, synced, verified and then renamed. Parent
/// directories are created as needed. `progress` is called after every received chunk.
pub async fn fetch_from(
    urls: &[&str],
    dest: &Path,
    mut progress: impl FnMut(Progress),
) -> Result<Fetched> {
    let client = client()?;
    let mut failures = Failures::default();
    for url in urls {
        match fetch_one(&client, url, dest, &mut progress).await {
            Ok(fetched) => {
                info!(
                    url,
                    path = %dest.display(),
                    len = fetched.len,
                    version = %fetched.version,
                    "downloaded kernel package"
                );
                return Ok(fetched);
            }
            Err(error) => {
                warn!(url, %error, "download failed");
                failures.0.push(((*url).to_string(), error));
            }
        }
    }
    Err(Error::AllSourcesFailed(failures))
}

pub(crate) fn client() -> Result<reqwest::Client> {
    // reqwest is built without a crypto provider, so install ring as the process default. It is
    // the backend cartload-console's SSH stack uses too. A second install is refused, which is
    // fine: any provider already in place works as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(Error::Client)
}

async fn fetch_one(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    progress: &mut impl FnMut(Progress),
) -> Result<Fetched> {
    let partial = partial_path(dest)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| download_error(url, source))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }
    let total = response.content_length();
    debug!(url, final_url = %response.url(), ?total, "download started");

    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| io_error("create directory", parent, source))?;
    }

    let result = write_and_verify(response, url, &partial, total, progress).await;
    let (len, version) = match result {
        Ok(ok) => ok,
        Err(error) => {
            discard(&partial).await;
            return Err(error);
        }
    };
    if let Err(source) = tokio::fs::rename(&partial, dest).await {
        discard(&partial).await;
        return Err(io_error("rename", &partial, source));
    }
    Ok(Fetched {
        url: url.to_string(),
        path: dest.to_path_buf(),
        len,
        version,
    })
}

/// Stream the body into `partial`, sync it, then open it as a package and validate it.
async fn write_and_verify(
    mut response: reqwest::Response,
    url: &str,
    partial: &Path,
    total: Option<u64>,
    progress: &mut impl FnMut(Progress),
) -> Result<(u64, Version)> {
    let mut file = tokio::fs::File::create(partial)
        .await
        .map_err(|source| io_error("create", partial, source))?;
    let mut done = 0u64;
    progress(Progress { done, total });
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|source| download_error(url, source))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|source| io_error("write", partial, source))?;
        done += chunk.len() as u64;
        progress(Progress { done, total });
    }
    if let Some(total) = total
        && done != total
    {
        return Err(Error::Truncated {
            url: url.to_string(),
            done,
            total,
        });
    }
    file.sync_all()
        .await
        .map_err(|source| io_error("sync", partial, source))?;
    drop(file);

    // Opening decompresses the whole archive; keep that off the async threads.
    let path = partial.to_path_buf();
    let verified = tokio::task::spawn_blocking(move || Package::open(&path)?.validate())
        .await
        .map_err(|join| io_error("verify", partial, std::io::Error::other(join)))?;
    let version = verified.map_err(|source| Error::Unusable {
        url: url.to_string(),
        source: Box::new(source),
    })?;
    Ok((done, version))
}

/// `<dest>.partial` in the same directory, so the final rename stays on one file system.
fn partial_path(dest: &Path) -> Result<PathBuf> {
    let name = dest.file_name().ok_or_else(|| Error::BadDestination {
        path: dest.to_path_buf(),
    })?;
    let mut partial = name.to_os_string();
    partial.push(".partial");
    Ok(dest.with_file_name(partial))
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

fn download_error(url: &str, source: reqwest::Error) -> Error {
    Error::Download {
        url: url.to_string(),
        // The message already names the URL; reqwest would repeat it (as the final URL).
        source: source.without_url(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;
    use crate::test_support::sample;

    /// A one-shot HTTP/1.1 server on the loopback interface: each accepted connection gets the
    /// next canned `(status, body)` response, then is closed. Returns the base URL.
    async fn serve(responses: Vec<(u16, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                }
                let reason = match status {
                    200 => "OK",
                    404 => "Not Found",
                    _ => "Error",
                };
                let head = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: \
                     application/octet-stream\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(&body).await.unwrap();
                stream.shutdown().await.ok();
            }
        });
        base
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn falls_back_to_the_next_source_and_writes_atomically() {
        let package = sample();
        let base = serve(vec![(404, b"gone".to_vec()), (200, package.clone())]).await;
        let first = format!("{base}/missing.hmod");
        let second = format!("{base}/hakchi-latest.hmod");
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("downloads").join("hakchi.hmod");

        let mut seen = Vec::new();
        let fetched = fetch_from(&[&first, &second], &dest, |p| seen.push(p))
            .await
            .unwrap();

        assert_eq!(fetched.url, second);
        assert_eq!(fetched.path, dest);
        assert_eq!(fetched.len, package.len() as u64);
        assert_eq!(fetched.version.hakchi, "v1.0.4-126");
        assert_eq!(std::fs::read(&dest).unwrap(), package);
        assert_eq!(leftovers(dest.parent().unwrap()), ["hakchi.hmod"]);
        assert_eq!(seen.first().unwrap().done, 0);
        assert_eq!(seen.last().unwrap().done, package.len() as u64);
        assert!(seen.iter().all(|p| p.total == Some(package.len() as u64)));
    }

    #[tokio::test]
    async fn replaces_an_existing_destination() {
        let package = sample();
        let base = serve(vec![(200, package.clone())]).await;
        let url = format!("{base}/hakchi.hmod");
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("hakchi.hmod");
        std::fs::write(&dest, b"old contents").unwrap();

        let fetched = fetch_from(&[&url], &dest, |_| {}).await.unwrap();
        assert_eq!(fetched.len, package.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), package);
        assert_eq!(leftovers(dir.path()), ["hakchi.hmod"]);
    }

    #[tokio::test]
    async fn rejects_downloads_that_are_not_kernel_packages() {
        let garbage = b"<html>not a package</html>".to_vec();
        let no_boot_files = crate::test_support::archive(&[crate::test_support::Spec::File(
            "var/version",
            b"hakchiVersion='v9'\n",
        )]);
        let base = serve(vec![(200, garbage), (200, no_boot_files)]).await;
        let first = format!("{base}/a.hmod");
        let second = format!("{base}/b.hmod");
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("hakchi.hmod");

        let error = fetch_from(&[&first, &second], &dest, |_| {})
            .await
            .unwrap_err();
        let Error::AllSourcesFailed(failures) = &error else {
            panic!("{error}");
        };
        assert_eq!(failures.0.len(), 2);
        assert_eq!(failures.0[0].0, first);
        assert!(
            matches!(&failures.0[0].1, Error::Unusable { source, .. } if matches!(**source, Error::Archive { .. })),
            "{error}"
        );
        assert!(
            matches!(&failures.0[1].1, Error::Unusable { source, .. } if matches!(**source, Error::MissingEntry { .. })),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .starts_with("all download sources failed: ")
        );
        assert!(error.to_string().contains("is not a usable kernel package"));
        assert!(
            leftovers(dir.path()).is_empty(),
            "{:?}",
            leftovers(dir.path())
        );
    }

    #[tokio::test]
    async fn reports_status_and_connection_errors() {
        let base = serve(vec![(500, b"oops".to_vec())]).await;
        let failing = format!("{base}/x.hmod");
        // A listener that is dropped immediately leaves a closed port behind.
        let closed = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            format!("http://{}/y.hmod", listener.local_addr().unwrap())
        };
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("hakchi.hmod");

        let error = fetch_from(&[&failing, &closed], &dest, |_| {})
            .await
            .unwrap_err();
        let Error::AllSourcesFailed(failures) = &error else {
            panic!("{error}");
        };
        assert!(
            matches!(failures.0[0].1, Error::HttpStatus { status: 500, .. }),
            "{error}"
        );
        assert!(matches!(failures.0[1].1, Error::Download { .. }), "{error}");
        assert!(error.to_string().contains("HTTP status 500"));
        assert!(leftovers(dir.path()).is_empty());
    }

    #[tokio::test]
    async fn no_sources_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = fetch_from(&[], &dir.path().join("x.hmod"), |_| {})
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "all download sources failed: no download sources were given"
        );
    }

    #[tokio::test]
    async fn destination_needs_a_file_name() {
        let base = serve(vec![]).await;
        let url = format!("{base}/x.hmod");
        let error = fetch_from(&[&url], Path::new(""), |_| {})
            .await
            .unwrap_err();
        let Error::AllSourcesFailed(failures) = &error else {
            panic!("{error}");
        };
        assert!(
            matches!(failures.0[0].1, Error::BadDestination { .. }),
            "{error}"
        );
    }

    #[test]
    fn partial_path_sits_next_to_the_destination() {
        assert_eq!(
            partial_path(Path::new("/tmp/x/hakchi.hmod")).unwrap(),
            Path::new("/tmp/x/hakchi.hmod.partial")
        );
        assert_eq!(
            partial_path(Path::new("hakchi.hmod")).unwrap(),
            Path::new("hakchi.hmod.partial")
        );
        assert!(matches!(
            partial_path(Path::new("")).unwrap_err(),
            Error::BadDestination { .. }
        ));
    }

    #[test]
    fn known_sources_are_https() {
        assert_eq!(DOWNLOAD_URLS.len(), 2);
        assert!(DOWNLOAD_URLS.iter().all(|u| u.starts_with("https://")));
        assert!(GITHUB_URL.contains("v1.0.4-126"));
        assert!(HAKCHI_NET_URL.ends_with("hakchi-latest.hmod"));
    }
}
