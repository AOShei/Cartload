//! `cartload-hmod`: inspect, extract and download hakchi.hmod kernel packages, without the UI.

use std::{io::Write, path::PathBuf};

use anyhow::{Context, Result};
use cartload_hmod::{DOWNLOAD_URLS, EntryKind, Package, Progress, basemods, fetch_from, pinned};
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "cartload-hmod",
    version,
    about = "hakchi.hmod kernel package tool for the NES / SNES / Mega Drive Classic Mini"
)]
struct Cli {
    /// Show debug output (RUST_LOG overrides this).
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the version variables, boot files and entry counts of a package.
    Info {
        /// hakchi.hmod to inspect.
        path: PathBuf,
    },
    /// List the entries of a package, one per line.
    List {
        /// hakchi.hmod to list.
        path: PathBuf,

        /// Also show kind, size and link target.
        #[arg(short, long)]
        long: bool,
    },
    /// Extract one entry to a file or to stdout.
    Extract {
        /// hakchi.hmod to read.
        path: PathBuf,

        /// Entry name, e.g. boot/boot.img (a leading ./ is accepted).
        entry: String,

        /// Write here instead of stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Download the pinned package (hakchi v1.0.4-126), verify it and write it atomically.
    Fetch {
        /// Where to write the package.
        #[arg(long, value_name = "PATH", default_value = "hakchi-latest.hmod")]
        dest: PathBuf,

        /// Source to download from instead of the built-in list; repeatable, tried in order.
        #[arg(long, value_name = "URL")]
        url: Vec<String>,

        /// Accept any valid kernel package, not only the pinned one.
        #[arg(long)]
        any: bool,
    },
    /// Write the built-in kernel package (hakchi v1.0.4-126) to a file, or describe it.
    Bundled {
        /// Write the package here.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Write the built-in fes1.bin (FEL DRAM init binary) to a file, or describe it.
    Fes1 {
        /// Write the binary here.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// List hakchi2 CE's base mods (tiny7zx for compressed games, font_remount) and their
    /// cache state; with --fetch, download the missing files into the cache.
    BaseMods {
        /// Download the files that are missing from the cache.
        #[arg(long)]
        fetch: bool,

        /// Cache directory (default: the user cache directory, cartload/basemods).
        #[arg(long, value_name = "DIR")]
        cache: Option<PathBuf>,

        /// Mods to fetch (default: the ones a compressed library needs).
        #[arg(value_name = "NAME")]
        names: Vec<String>,
    },
    /// List the emulator modules Cartload can install from KMFD's Mod Hub (RetroArch and
    /// cores) and their cache state; with --fetch, download the missing ones.
    HubMods {
        /// Download the modules that are missing from the cache.
        #[arg(long)]
        fetch: bool,

        /// Cache directory (default: the user cache directory, cartload/mods).
        #[arg(long, value_name = "DIR")]
        cache: Option<PathBuf>,

        /// Modules to fetch by id (default: all of them).
        #[arg(value_name = "ID")]
        ids: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(if cli.verbose { "debug" } else { "info" }));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    match cli.command {
        Command::Info { path } => info_cmd(&path),
        Command::List { path, long } => list(&path, long),
        Command::Extract {
            path,
            entry,
            output,
        } => extract(&path, &entry, output.as_deref()),
        Command::Fetch { dest, url, any } => fetch(&dest, &url, any).await,
        Command::Bundled { output } => bundled(output.as_deref()),
        Command::Fes1 { output } => fes1(output.as_deref()),
        Command::BaseMods {
            fetch,
            cache,
            names,
        } => base_mods(fetch, cache, &names).await,
        Command::HubMods { fetch, cache, ids } => hub_mods(fetch, cache, &ids).await,
    }
}

async fn hub_mods(fetch: bool, cache: Option<PathBuf>, ids: &[String]) -> Result<()> {
    use cartload_hmod::modhub;
    let cache = match cache {
        Some(cache) => cache,
        None => modhub::default_cache_dir().context("no cache directory on this system")?,
    };
    let modules: Vec<&'static modhub::Module> = if ids.is_empty() {
        modhub::CATALOG.to_vec()
    } else {
        ids.iter()
            .map(|id| {
                modhub::find(id).with_context(|| {
                    format!(
                        "{id} is not a known module; known: {}",
                        modhub::CATALOG
                            .iter()
                            .map(|m| m.id)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
            })
            .collect::<Result<_>>()?
    };
    println!("hub:     {}", modhub::HUB_URL);
    println!("cache:   {}", cache.display());
    if fetch {
        let mut last = None;
        modhub::fetch(&modules, &cache, |p| {
            let key = (
                p.index,
                p.done / (1 << 20),
                p.total.is_some_and(|t| p.done >= t),
            );
            if last != Some(key) {
                last = Some(key);
                info!(
                    "{}: {} / {} bytes",
                    modules[p.index].id,
                    p.done,
                    p.total.map_or("?".to_string(), |t| t.to_string())
                );
            }
        })
        .await?;
    }
    for module in modhub::CATALOG {
        let wanted = modules.iter().any(|m| m.id == module.id);
        let state = match module.cached_size(&cache) {
            Some(size) => format!("cached, {size} bytes"),
            None => "not cached".to_string(),
        };
        println!(
            "{:<10} {:<18} {}{}",
            module.id,
            module.title,
            state,
            if wanted { "" } else { " (not requested)" }
        );
    }
    Ok(())
}

fn open(path: &std::path::Path) -> Result<Package> {
    Package::open(path).with_context(|| format!("opening {}", path.display()))
}

fn info_cmd(path: &std::path::Path) -> Result<()> {
    let package = open(path)?;
    println!("package:   {}", path.display());
    match package.version() {
        Ok(version) => {
            println!("version:   {version}");
            for (name, value) in &version.variables {
                println!("           {name}={value}");
            }
        }
        Err(error) => println!("version:   unavailable ({error})"),
    }
    for (label, name) in [
        ("boot.img: ", cartload_hmod::BOOT_IMAGE),
        ("uboot.bin:", cartload_hmod::UBOOT),
    ] {
        match package.entry(name) {
            Some(entry) if entry.kind == EntryKind::File => {
                println!("{label} {} bytes", entry.size);
            }
            Some(entry) => println!("{label} present but a {}", entry.kind),
            None => println!("{label} MISSING"),
        }
    }
    let count = |kind: EntryKind| package.entries().iter().filter(|e| e.kind == kind).count();
    println!(
        "entries:   {} ({} files, {} directories, {} links)",
        package.entries().len(),
        count(EntryKind::File),
        count(EntryKind::Directory),
        count(EntryKind::Symlink) + count(EntryKind::HardLink)
    );
    match package.validate() {
        Ok(_) => println!("valid kernel package"),
        Err(error) => {
            println!("NOT a valid kernel package: {error}");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Treat a closed stdout (`list | head`) as a normal end, not an error.
fn ignore_broken_pipe(result: std::io::Result<()>) -> Result<()> {
    match result {
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => Ok(other?),
    }
}

fn list(path: &std::path::Path, long: bool) -> Result<()> {
    let package = open(path)?;
    ignore_broken_pipe(write_listing(&package, long))
}

fn write_listing(package: &Package, long: bool) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    for entry in package.entries() {
        if long {
            let kind = match entry.kind {
                EntryKind::File => "file",
                EntryKind::Directory => "dir",
                EntryKind::Symlink => "link",
                EntryKind::HardLink => "hard",
                EntryKind::Other => "other",
            };
            write!(out, "{kind:<5} {:>9} {}", entry.size, entry.name)?;
            if let Some(target) = &entry.link_target {
                write!(out, " -> {target}")?;
            }
            writeln!(out)?;
        } else {
            writeln!(out, "{}", entry.name)?;
        }
    }
    out.flush()
}

fn extract(path: &std::path::Path, entry: &str, output: Option<&std::path::Path>) -> Result<()> {
    let package = open(path)?;
    let data = package.read(entry)?;
    match output {
        Some(file) => {
            std::fs::write(file, &data).with_context(|| format!("writing {}", file.display()))?;
            info!("wrote {} bytes to {}", data.len(), file.display());
        }
        None => {
            let mut out = std::io::stdout().lock();
            ignore_broken_pipe(out.write_all(&data).and_then(|()| out.flush()))?;
        }
    }
    Ok(())
}

async fn fetch(dest: &std::path::Path, urls: &[String], any: bool) -> Result<()> {
    let urls: Vec<&str> = if urls.is_empty() {
        DOWNLOAD_URLS.to_vec()
    } else {
        urls.iter().map(String::as_str).collect()
    };
    let mut last_percent = None;
    let sha256 = (!any).then_some(pinned::PINNED_SHA256);
    let fetched = fetch_from(&urls, dest, sha256, |Progress { done, total }| {
        let percent = total.and_then(|t| (done * 100).checked_div(t));
        let step = percent.map(|p| p / 10);
        if step != last_percent {
            last_percent = step;
            match (percent, total) {
                (Some(percent), Some(total)) => {
                    info!("downloaded {done} / {total} bytes ({percent}%)")
                }
                _ if done > 0 => info!("downloaded {done} bytes"),
                _ => {}
            }
        }
    })
    .await?;
    println!(
        "downloaded {} ({} bytes) from {}",
        fetched.path.display(),
        fetched.len,
        fetched.url
    );
    println!("version: {}", fetched.version);
    Ok(())
}

async fn base_mods(fetch: bool, cache: Option<PathBuf>, names: &[String]) -> Result<()> {
    let cache = match cache {
        Some(cache) => cache,
        None => basemods::default_cache_dir().context("no cache directory on this system")?,
    };
    let mods: Vec<&'static basemods::BaseMod> = if names.is_empty() {
        basemods::FOR_COMPRESSION.to_vec()
    } else {
        names
            .iter()
            .map(|name| {
                basemods::find(name).with_context(|| {
                    format!(
                        "{name} is not a known base mod; known: {}",
                        basemods::CATALOG
                            .iter()
                            .map(|m| m.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
            })
            .collect::<Result<_>>()?
    };
    println!(
        "source:  {} at {}",
        basemods::SOURCE_REPOSITORY,
        basemods::SOURCE_COMMIT
    );
    println!("cache:   {}", cache.display());
    if fetch {
        let mut last = None;
        let loaded = basemods::fetch(&mods, &cache, |p| {
            let key = (p.name, p.file, p.done == p.total);
            if last != Some(key) {
                last = Some(key);
                info!("{}: {} {} / {} bytes", p.name, p.file, p.done, p.total);
            }
        })
        .await?;
        for m in &loaded {
            println!(
                "fetched: {} ({} files, {} bytes, tar {} bytes)",
                m.name(),
                m.files.len(),
                m.len(),
                m.tar.len()
            );
        }
    }
    for spec in basemods::CATALOG {
        let status = spec.status(&cache);
        let wanted = mods.iter().any(|m| m.name == spec.name);
        println!(
            "{:<13} {}{}",
            spec.name,
            if status.is_complete() {
                "cached".to_string()
            } else {
                format!(
                    "{} of {} files missing",
                    status.missing.len(),
                    spec.files.len()
                )
            },
            if wanted { "  (selected)" } else { "" }
        );
        println!("              {}", spec.summary);
        for file in spec.files {
            println!(
                "              {:<32} {:>8} bytes  md5 {}",
                file.path, file.size, file.md5
            );
        }
    }
    Ok(())
}

fn bundled(output: Option<&std::path::Path>) -> Result<()> {
    let bytes = pinned::bundled();
    match output {
        Some(file) => {
            pinned::install_bundled(file)?;
            println!(
                "wrote hakchi {} ({} bytes) to {}",
                pinned::PINNED_VERSION,
                bytes.len(),
                file.display()
            );
        }
        None => println!(
            "hakchi {}: {} bytes, SHA-256 {}, built in from data/hakchi.hmod (GPL)",
            pinned::PINNED_VERSION,
            bytes.len(),
            pinned::PINNED_SHA256
        ),
    }
    Ok(())
}

fn fes1(output: Option<&std::path::Path>) -> Result<()> {
    let bytes = cartload_hmod::fes1();
    match output {
        Some(file) => {
            std::fs::write(file, bytes).with_context(|| format!("writing {}", file.display()))?;
            println!(
                "wrote fes1.bin ({} bytes) to {}",
                bytes.len(),
                file.display()
            );
        }
        None => println!(
            "fes1.bin: {} bytes, built in from data/fes1.bin (hakchi2 CE, GPL-3)",
            bytes.len()
        ),
    }
    Ok(())
}
