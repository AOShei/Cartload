//! `cartload-console`: find a console running the hakchi kernel and talk to it, without the UI.

use std::{
    io::{Read, Write},
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use cartload_console::{
    ConsoleInfo, DiscoverOptions, Ftp, Hakchi, SSH_PORT, Session, SessionOptions, discover,
    discover_all, link_local_interfaces, parse::sh_quote,
};
use clap::{Args, Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "cartload-console",
    version,
    about = "Talk to a NES / SNES / Mega Drive Classic Mini running the hakchi kernel"
)]
struct Cli {
    /// Show debug output (RUST_LOG overrides this).
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

/// How to reach the console.
#[derive(Args, Clone)]
struct Connect {
    /// Console address (`host` or `host:port`). Skips discovery.
    #[arg(long, value_name = "ADDR")]
    host: Option<String>,

    /// Also browse mDNS (_ssh._tcp), for consoles on Wi-Fi.
    #[arg(long)]
    mdns: bool,

    /// Timeout for one probe or connect, in milliseconds.
    #[arg(long, value_name = "MS", default_value_t = 500)]
    timeout_ms: u64,

    /// Timeout for one remote command, in seconds (0 waits forever).
    #[arg(long, value_name = "SECS", default_value_t = 60)]
    command_timeout: u64,
}

#[derive(Subcommand)]
enum Command {
    /// Look for the console over the USB link (and optionally mDNS) and print where it answers.
    Discover {
        /// Also browse mDNS (_ssh._tcp), for consoles on Wi-Fi.
        #[arg(long)]
        mdns: bool,

        /// Timeout for one probe, in milliseconds.
        #[arg(long, value_name = "MS", default_value_t = 500)]
        timeout_ms: u64,

        /// Run every strategy to completion and list all consoles found.
        #[arg(long)]
        all: bool,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Connect over SSH and print what the console is: type, versions, storage, original games.
    Info {
        #[command(flatten)]
        connect: Connect,

        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run a shell command on the console and exit with its exit code.
    Exec {
        #[command(flatten)]
        connect: Connect,

        /// Feed this process's stdin to the command.
        #[arg(long)]
        stdin: bool,

        /// The command line, passed to the console's shell as one string.
        #[arg(required = true, trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// List a remote directory (`ls -la` over SSH, or an FTP LIST with --ftp).
    Ls {
        #[command(flatten)]
        connect: Connect,

        /// Use the FTP server instead of the shell.
        #[arg(long)]
        ftp: bool,

        /// Remote path.
        path: String,
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
        Command::Discover {
            mdns,
            timeout_ms,
            all,
            json,
        } => run_discover(mdns, timeout_ms, all, json).await,
        Command::Info { connect, json } => run_info(&connect, json).await,
        Command::Exec {
            connect,
            stdin,
            command,
        } => run_exec(&connect, stdin, &command.join(" ")).await,
        Command::Ls { connect, ftp, path } => run_ls(&connect, ftp, &path).await,
    }
}

fn discover_options(mdns: bool, timeout_ms: u64) -> DiscoverOptions {
    DiscoverOptions {
        mdns,
        probe_timeout: Duration::from_millis(timeout_ms),
        ..DiscoverOptions::default()
    }
}

async fn run_discover(mdns: bool, timeout_ms: u64, all: bool, json: bool) -> Result<()> {
    let interfaces = link_local_interfaces()?;
    for interface in &interfaces {
        info!(
            "link-local interface {} ({}) operstate {}{}",
            interface.name,
            interface.address,
            interface.operstate,
            if interface.usable { "" } else { ", skipped" }
        );
    }
    let options = discover_options(mdns, timeout_ms);
    let found = if all {
        discover_all(&options).await?
    } else {
        vec![discover(&options).await?]
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&found)?);
    } else if found.is_empty() {
        println!("no console found");
    } else {
        for console in &found {
            println!("{console}");
        }
    }
    if found.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

async fn resolve(connect: &Connect) -> Result<SocketAddr> {
    let Some(host) = &connect.host else {
        let found = discover(&discover_options(connect.mdns, connect.timeout_ms)).await?;
        return Ok(found.address);
    };
    if let Ok(address) = host.parse::<SocketAddr>() {
        return Ok(address);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, SSH_PORT));
    }
    let candidate = if host.contains(':') {
        host.clone()
    } else {
        format!("{host}:{SSH_PORT}")
    };
    tokio::net::lookup_host(&candidate)
        .await
        .with_context(|| format!("resolving {host}"))?
        .next()
        .with_context(|| format!("{host} did not resolve to an address"))
}

async fn open_session(connect: &Connect) -> Result<Session> {
    let address = resolve(connect).await?;
    let options = SessionOptions {
        connect_timeout: Duration::from_millis(connect.timeout_ms.max(1000)),
        command_timeout: (connect.command_timeout > 0)
            .then(|| Duration::from_secs(connect.command_timeout)),
        ..SessionOptions::default()
    };
    let session = Session::connect_with(address, options).await?;
    if let Some(key) = session.host_key() {
        info!("host key {} {}", key.algorithm, key.fingerprint);
    }
    Ok(session)
}

async fn run_info(connect: &Connect, json: bool) -> Result<()> {
    let session = open_session(connect).await?;
    let info = Hakchi::new(&session).info().await?;
    let _ = session.close().await;
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        print_info(&info);
    }
    Ok(())
}

fn print_info(info: &ConsoleInfo) {
    println!("hwid:            {}", info.hwid);
    match &info.versions {
        Some(versions) => println!("versions:        {versions}"),
        None => println!("versions:        /var/version missing (kernel too old)"),
    }
    if info.minimal_memboot {
        println!("state:           minimal memboot (recovery); nothing else detected");
        return;
    }
    if !info.can_interact {
        println!("state:           software too old; reinstall the kernel before using it");
        return;
    }
    println!(
        "console:         {} [{}]",
        info.console,
        info.system_code.as_deref().unwrap_or("?")
    );
    if let Some(firmware) = &info.firmware {
        println!("firmware:        {firmware}");
    }
    println!(
        "md partitioning: {}",
        if info.md_partitioning { "yes" } else { "no" }
    );
    println!(
        "games storage:   {}",
        info.game_sync_path.as_deref().unwrap_or("?")
    );
    println!(
        "paths:           gamepath {}, rootfs {}, squashfs {}",
        info.original_games_path.as_deref().unwrap_or("?"),
        info.rootfs_path.as_deref().unwrap_or("?"),
        info.squashfs_path.as_deref().unwrap_or("?")
    );
    if let Some(storage) = &info.storage {
        println!("{}", storage.summary(info.console));
        if storage.external_saves {
            println!("    Save states are on external storage");
        }
    }
    if !info.config.is_empty() {
        let config: Vec<String> = info
            .config
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect();
        println!("config:          {}", config.join(", "));
    }
    println!(
        "original games:  {} ({})",
        info.original_games.len(),
        info.original_games.join(", ")
    );
}

async fn run_exec(connect: &Connect, read_stdin: bool, command: &str) -> Result<()> {
    let stdin = if read_stdin {
        let mut data = Vec::new();
        std::io::stdin()
            .read_to_end(&mut data)
            .context("reading stdin")?;
        Some(data)
    } else {
        None
    };
    let session = open_session(connect).await?;
    let output = session.exec_with_stdin(command, stdin.as_deref()).await?;
    let _ = session.close().await;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    match output.exit_code {
        Some(0) => Ok(()),
        Some(code) => {
            info!("remote exit code {code}");
            std::process::exit(i32::try_from(code).unwrap_or(1))
        }
        None => bail!("the console did not report an exit status for `{command}`"),
    }
}

async fn run_ls(connect: &Connect, ftp: bool, path: &str) -> Result<()> {
    if ftp {
        let address = resolve(connect).await?;
        let mut ftp = Ftp::connect(
            SocketAddr::new(address.ip(), cartload_console::FTP_PORT),
            Duration::from_millis(connect.timeout_ms.max(1000)),
        )
        .await?;
        if let Some(welcome) = ftp.welcome() {
            info!("FTP welcome: {}", welcome.trim());
        }
        let lines = ftp.list(Some(path)).await?;
        let _ = ftp.quit().await;
        for line in lines {
            println!("{line}");
        }
        return Ok(());
    }
    run_exec(connect, false, &format!("ls -la -- {}", sh_quote(path))).await
}
