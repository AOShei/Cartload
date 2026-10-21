//! `cartload-hmod`: inspect, extract and download hakchi.hmod kernel packages, without the UI.

use std::{io::Write, path::PathBuf};

use anyhow::{Context, Result};
use cartload_hmod::{DOWNLOAD_URLS, EntryKind, Package, Progress, fetch_from};
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
    /// Download the latest package, verify it and write it atomically.
    Fetch {
        /// Where to write the package.
        #[arg(long, value_name = "PATH", default_value = "hakchi-latest.hmod")]
        dest: PathBuf,

        /// Source to download from instead of the built-in list; repeatable, tried in order.
        #[arg(long, value_name = "URL")]
        url: Vec<String>,
    },
    /// Write the built-in fes1.bin (FEL DRAM init binary) to a file, or describe it.
    Fes1 {
        /// Write the binary here.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
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
        Command::Fetch { dest, url } => fetch(&dest, &url).await,
        Command::Fes1 { output } => fes1(output.as_deref()),
    }
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

async fn fetch(dest: &std::path::Path, urls: &[String]) -> Result<()> {
    let urls: Vec<&str> = if urls.is_empty() {
        DOWNLOAD_URLS.to_vec()
    } else {
        urls.iter().map(String::as_str).collect()
    };
    let mut last_percent = None;
    let fetched = fetch_from(&urls, dest, |Progress { done, total }| {
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
