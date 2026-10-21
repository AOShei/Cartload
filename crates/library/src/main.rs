//! `cartload-library`: import games, show the menu, plan and run a sync without the UI.

use std::{
    io::{IsTerminal, Write},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use cartload_console::{
    ConsoleInfo, ConsoleType, DiscoverOptions, Hakchi, SSH_PORT, Session, SessionOptions, discover,
};
use cartload_library::{
    DevicePaths, HashDb, ImportOptions, Library, LibraryConfig, Menu, MenuOptions, Structure,
    SyncEvent, SyncPlan,
    art::{self, ApiKey, ArtEvent, ArtOptions, ArtSize, Client, CoverStatus, HttpTransport},
    hashdb,
    images::CoverOptions,
    menu::{self, Entry, GameRef},
    sync::{self, commands, parse_link_listing, parse_listing},
};
use clap::{Args, Parser, Subcommand};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "cartload-library",
    version,
    about = "Manage the games library and sync it to a NES / SNES Classic Mini running hakchi"
)]
struct Cli {
    /// Show debug output (RUST_LOG overrides this).
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Library directory (default: the CARTLOAD_LIBRARY variable or the user data directory).
    #[arg(long, global = true, value_name = "DIR")]
    library: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// How to reach the console.
#[derive(Args, Clone)]
struct Connect {
    /// Console address (`host` or `host:port`). Skips discovery.
    #[arg(long, value_name = "ADDR")]
    host: Option<String>,

    /// Timeout for one probe or connect, in milliseconds.
    #[arg(long, value_name = "MS", default_value_t = 500)]
    timeout_ms: u64,

    /// Timeout for one remote command, in seconds (0 waits forever).
    #[arg(long, value_name = "SECS", default_value_t = 120)]
    command_timeout: u64,
}

/// Menu choices.
#[derive(Args, Clone)]
struct MenuArgs {
    /// Structure: flat, letter, or count:N (default from library.json, count:30).
    #[arg(long, value_name = "STRUCTURE")]
    structure: Option<String>,

    /// Keep the firmware's games in the root menu and the library in "More games..."
    /// (default from library.json, on).
    #[arg(long, value_name = "BOOL")]
    originals_at_root: Option<bool>,

    /// Console type when none is connected (system code such as snes-eur, nes-usa).
    #[arg(long, value_name = "CODE")]
    console: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Import ROM files (.sfc, .smc, .fig, .swc, .nes) or zips of them.
    Import {
        /// Files to import.
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Do not use the hash database (names come from the file names).
        #[arg(long)]
        no_db: bool,

        /// Hash database file (romfiles.xml or .gz) instead of the cached download.
        #[arg(long, value_name = "FILE")]
        db: Option<PathBuf>,

        /// Store the ROM uncompressed (by default SNES and NES ROMs go to the console as
        /// 7z archives, which need the tiny7zx mod there).
        #[arg(long)]
        no_compress: bool,
    },
    /// Turn 7z compression on (default) or off for games, building or removing the archives.
    Compress {
        /// Game codes.
        #[arg(value_name = "CODE", required_unless_present = "all")]
        codes: Vec<String>,

        /// Every game in the library.
        #[arg(long, conflicts_with = "codes")]
        all: bool,

        /// Turn compression off: the ROM itself goes to the console.
        #[arg(long)]
        off: bool,
    },
    /// List the games in the library.
    List {
        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove games from the library.
    Remove {
        /// Game codes.
        #[arg(required = true)]
        codes: Vec<String>,
    },
    /// Show the menu tree the console will get.
    Menu {
        #[command(flatten)]
        menu: MenuArgs,

        /// Save the structure as the library's default.
        #[arg(long)]
        save: bool,

        /// Print the tree as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print the sync plan: against the console when one answers, else against an empty
    /// console.
    Plan {
        #[command(flatten)]
        connect: Connect,

        #[command(flatten)]
        menu: MenuArgs,

        /// Print the plan as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Sync the library to the console.
    Sync {
        #[command(flatten)]
        connect: Connect,

        #[command(flatten)]
        menu: MenuArgs,

        /// Do not ask for confirmation.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Download the hash database into the cache.
    FetchDb {
        /// Where to write it (default: the cache location).
        #[arg(long, value_name = "FILE")]
        dest: Option<PathBuf>,
    },
    /// Fetch cover art and metadata from TheGamesDB (key from CARTLOAD_TGDB_KEY, --key or
    /// the stored one).
    Art(ArtArgs),
    /// Show, store or clear the TheGamesDB API key kept in library.json.
    Key {
        /// Store this key.
        #[arg(long, value_name = "KEY", conflicts_with = "clear")]
        set: Option<String>,

        /// Remove the stored key.
        #[arg(long)]
        clear: bool,
    },
}

/// `art` options.
#[derive(Args, Clone)]
struct ArtArgs {
    /// Game codes (CLV-U-XXXXX).
    #[arg(value_name = "CODE", required_unless_present = "all")]
    codes: Vec<String>,

    /// Every game that still has placeholder art (every game with --force).
    #[arg(long, conflicts_with = "codes")]
    all: bool,

    /// API key for this run.
    #[arg(long, value_name = "KEY")]
    key: Option<String>,

    /// Fetch again even when art is there and downloads are cached.
    #[arg(long)]
    force: bool,

    /// Search again instead of reusing earlier search results.
    #[arg(long)]
    research: bool,

    /// Search with this term instead of the title (one game).
    #[arg(long, value_name = "NAME")]
    search: Option<String>,

    /// Use this TheGamesDB game id (one game).
    #[arg(long, value_name = "ID")]
    id: Option<u32>,

    /// Use a local image file as the cover instead of TheGamesDB (one game, no network).
    #[arg(long, value_name = "FILE")]
    image: Option<PathBuf>,

    /// Box art size to download: original, small, thumb, cropped_center_thumb, medium, large.
    #[arg(long, value_name = "SIZE", default_value = "medium")]
    size: String,

    /// Do not touch publisher, release date, players or description.
    #[arg(long)]
    no_metadata: bool,

    /// Replace metadata that was edited (by default only import defaults are filled).
    #[arg(long)]
    overwrite: bool,

    /// Do not download clear logos.
    #[arg(long)]
    no_logo: bool,

    /// Pad thumbnails to 40 pixels high with the image centred.
    #[arg(long)]
    center_thumbnail: bool,

    /// Write true-colour PNGs instead of 8-bit palette ones.
    #[arg(long)]
    no_quantize: bool,

    /// Print the report as JSON.
    #[arg(long)]
    json: bool,
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

    let root = match cli
        .library
        .or_else(|| std::env::var_os("CARTLOAD_LIBRARY").map(PathBuf::from))
    {
        Some(root) => root,
        None => Library::default_root().context("no user data directory on this system")?,
    };
    let library = Library::open(&root)?;
    info!(library = %root.display(), "library");

    match cli.command {
        Command::Import {
            files,
            no_db,
            db,
            no_compress,
        } => {
            let options = ImportOptions {
                compress: !no_compress,
            };
            run_import(&library, &files, no_db, db, &options).await
        }
        Command::Compress { codes, all, off } => run_compress(&library, &codes, all, !off),
        Command::List { json } => run_list(&library, json),
        Command::Remove { codes } => {
            for code in codes {
                library.remove(&code)?;
                println!("removed {code}");
            }
            Ok(())
        }
        Command::Menu { menu, save, json } => run_menu(&library, &menu, save, json),
        Command::Plan {
            connect,
            menu,
            json,
        } => run_plan(&library, &connect, &menu, json).await,
        Command::Sync { connect, menu, yes } => run_sync(&library, &connect, &menu, yes).await,
        Command::FetchDb { dest } => run_fetch_db(dest).await,
        Command::Art(args) => run_art(&library, &args).await,
        Command::Key { set, clear } => run_key(&library, set, clear),
    }
}

async fn load_db(no_db: bool, explicit: Option<PathBuf>) -> Result<Option<HashDb>> {
    if no_db {
        return Ok(None);
    }
    let path = match explicit {
        Some(path) => path,
        None => {
            let path = hashdb::default_cache_path().context("no cache directory on this system")?;
            if !path.exists() {
                info!(url = hashdb::ROMFILES_URL, "downloading the hash database");
                match hashdb::fetch(&path).await {
                    Ok(len) => info!(path = %path.display(), len, "hash database cached"),
                    Err(error) => {
                        warn!(%error, "could not download the hash database; names come from the file names");
                        return Ok(None);
                    }
                }
            }
            path
        }
    };
    let db = HashDb::load(&path)?;
    info!(path = %path.display(), records = db.len(), "hash database loaded");
    Ok(Some(db))
}

async fn run_import(
    library: &Library,
    files: &[PathBuf],
    no_db: bool,
    db: Option<PathBuf>,
    options: &ImportOptions,
) -> Result<()> {
    let db = load_db(no_db, db).await?;
    let mut failures = 0;
    for file in files {
        match library.import_with(file, db.as_ref(), options) {
            Ok(imported) => {
                for item in imported {
                    let game = &item.game;
                    println!(
                        "{}  {}  [{}] {}{}{}",
                        game.code,
                        game.name,
                        game.system,
                        if item.matched {
                            "matched"
                        } else {
                            "not in the hash database"
                        },
                        if item.replaced { ", replaced" } else { "" },
                        match sizes(library, game) {
                            (rom, Some(archive)) => format!(
                                ", {rom} bytes packed to {archive} ({}%)",
                                percent(archive, rom)
                            ),
                            (rom, None) => format!(", {rom} bytes uncompressed"),
                        }
                    );
                    for warning in &item.warnings {
                        println!("    warning: {warning}");
                    }
                }
            }
            Err(error) => {
                failures += 1;
                eprintln!("{}: {error}", file.display());
            }
        }
    }
    if failures > 0 {
        bail!("{failures} file(s) could not be imported");
    }
    Ok(())
}

/// ROM size and, for a compressed game, archive size.
fn sizes(library: &Library, game: &cartload_library::Game) -> (u64, Option<u64>) {
    let size = |path: PathBuf| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let rom = size(library.rom_path(game));
    let archive = game.compress.then(|| size(library.archive_path(game)));
    (rom, archive)
}

fn percent(part: u64, whole: u64) -> u64 {
    (part * 100).checked_div(whole).unwrap_or(100)
}

fn run_compress(library: &Library, codes: &[String], all: bool, compress: bool) -> Result<()> {
    let codes: Vec<String> = if all {
        library.games()?.into_iter().map(|g| g.code).collect()
    } else {
        codes.to_vec()
    };
    let mut total_rom = 0u64;
    let mut total_console = 0u64;
    for code in &codes {
        let mut game = library.game(code)?;
        let report = library.set_compress(&mut game, compress)?;
        total_rom += report.rom_size;
        total_console += report.archive_size.unwrap_or(report.rom_size);
        println!(
            "{code}  {:<40}  {}{}",
            game.name,
            match report.archive_size {
                Some(archive) => format!(
                    "compressed: {} bytes packed to {archive} ({}%)",
                    report.rom_size,
                    percent(archive, report.rom_size)
                ),
                None => format!("uncompressed: {} bytes", report.rom_size),
            },
            if report.changed { "" } else { "  (unchanged)" }
        );
    }
    if codes.len() > 1 {
        println!(
            "{} games: {total_rom} bytes of ROMs, {total_console} bytes on the console ({}%)",
            codes.len(),
            percent(total_console, total_rom)
        );
    }
    Ok(())
}

fn run_list(library: &Library, json: bool) -> Result<()> {
    let games = library.games()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&games)?);
        return Ok(());
    }
    if games.is_empty() {
        println!("no games in {}", library.root().display());
        return Ok(());
    }
    for game in &games {
        println!(
            "{}  {:<40}  {:<6}  {}  crc32 {:08X}  {}{}",
            game.code,
            game.name,
            match game.system {
                cartload_library::System::Snes => "SNES",
                cartload_library::System::Nes => "NES",
                cartload_library::System::MegaDrive => "MD",
            },
            game.console_rom_file(),
            game.crc32,
            match (game.tgdb_id, game.tgdb_ids.is_empty()) {
                (Some(id), _) => format!("tgdb {id}"),
                (None, false) => format!("tgdb {:?}", game.tgdb_ids),
                (None, true) => String::new(),
            },
            if game.placeholder_art { "  no art" } else { "" }
        );
    }
    Ok(())
}

fn menu_options(library: &Library, args: &MenuArgs) -> Result<(LibraryConfig, MenuOptions)> {
    let config = library.config()?;
    let mut options = config.menu.clone();
    if let Some(structure) = &args.structure {
        options.structure = structure.parse::<Structure>()?;
    }
    if let Some(originals) = args.originals_at_root {
        options.originals_at_root = originals;
    }
    Ok((config, options))
}

fn build_menu(library: &Library, options: &MenuOptions, system_code: Option<&str>) -> Result<Menu> {
    let mut refs: Vec<GameRef> = library
        .games()?
        .iter()
        .map(|g| GameRef {
            code: g.code.clone(),
            sort_title: g.sort_title(),
            original: false,
        })
        .collect();
    if let Some(system_code) = system_code {
        for (code, entry, _) in library.originals(system_code)? {
            refs.push(GameRef {
                code,
                sort_title: entry.sort_title.clone(),
                original: true,
            });
        }
    }
    Ok(menu::build(&refs, options))
}

fn print_menu(entries: &[Entry], depth: usize) {
    let indent = "  ".repeat(depth);
    for entry in entries {
        match entry {
            Entry::Game { code } => println!("{indent}{code}"),
            Entry::Original { code } => println!("{indent}{code}  (firmware)"),
            Entry::Folder(folder) => {
                println!(
                    "{indent}[{}]  ({} entries)",
                    folder.name,
                    folder.entries.len()
                );
                print_menu(&folder.entries, depth + 1);
            }
            Entry::Back(link) => println!("{indent}<- {}", link.name),
        }
    }
}

fn run_menu(library: &Library, args: &MenuArgs, save: bool, json: bool) -> Result<()> {
    let (mut config, options) = menu_options(library, args)?;
    let system_code = args.console.clone().or(config.last_console.clone());
    let menu = build_menu(library, &options, system_code.as_deref())?;
    if save {
        config.menu = options.clone();
        library.save_config(&config)?;
        info!("saved menu options to library.json");
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&menu)?);
        return Ok(());
    }
    println!(
        "structure {}, originals at root: {}{}",
        options.structure,
        options.originals_at_root,
        system_code
            .as_deref()
            .map(|c| format!(", firmware games of {c}"))
            .unwrap_or_default()
    );
    print_menu(&menu.entries, 0);
    let exported = menu::export(&menu);
    println!("{} menu directories on the console", exported.len());
    Ok(())
}

async fn resolve(connect: &Connect) -> Result<Option<SocketAddr>> {
    let Some(host) = &connect.host else {
        let options = DiscoverOptions {
            probe_timeout: Duration::from_millis(connect.timeout_ms),
            ..DiscoverOptions::default()
        };
        return match discover(&options).await {
            Ok(found) => Ok(Some(found.address)),
            Err(error) => {
                info!(%error, "no console found");
                Ok(None)
            }
        };
    };
    if let Ok(address) = host.parse::<SocketAddr>() {
        return Ok(Some(address));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(Some(SocketAddr::new(ip, SSH_PORT)));
    }
    let candidate = if host.contains(':') {
        host.clone()
    } else {
        format!("{host}:{SSH_PORT}")
    };
    Ok(Some(
        tokio::net::lookup_host(&candidate)
            .await
            .with_context(|| format!("resolving {host}"))?
            .next()
            .with_context(|| format!("{host} did not resolve to an address"))?,
    ))
}

/// Connects and reads the console; `None` when discovery finds nothing.
async fn open_console(connect: &Connect) -> Result<Option<(Session, ConsoleInfo)>> {
    let Some(address) = resolve(connect).await? else {
        return Ok(None);
    };
    let options = SessionOptions {
        connect_timeout: Duration::from_millis(connect.timeout_ms.max(1000)),
        command_timeout: (connect.command_timeout > 0)
            .then(|| Duration::from_secs(connect.command_timeout)),
        ..SessionOptions::default()
    };
    let session = Session::connect_with(address, options).await?;
    let info = Hakchi::new(&session).info().await?;
    if !info.can_interact {
        bail!("the console's hakchi software is too old or it is in a minimal memboot");
    }
    println!(
        "console: {} [{}], storage {}",
        info.console,
        info.system_code.as_deref().unwrap_or("?"),
        info.game_sync_path.as_deref().unwrap_or("?")
    );
    Ok(Some((session, info)))
}

/// Everything a plan needs from a console, or the offline defaults.
struct Target {
    session: Option<Session>,
    paths: DevicePaths,
    system_code: String,
    available: Option<u64>,
    remote: Vec<sync::RemoteFile>,
    remote_links: Vec<String>,
}

async fn prepare_target(
    library: &Library,
    connect: &Connect,
    args: &MenuArgs,
    need_console: bool,
) -> Result<Target> {
    let mut config = library.config()?;
    match open_console(connect).await? {
        Some((session, info)) => {
            let paths = DevicePaths::from_info(&info)?;
            let system_code = paths.system_code.clone();
            if library.originals(&system_code)?.is_empty() && !info.original_games.is_empty() {
                info!("reading the firmware's launch entries");
                let originals = sync::fetch_originals(&session, &paths.squashfs_games).await?;
                library.store_originals(&system_code, &originals)?;
                info!(
                    count = originals.len(),
                    "cached the firmware's launch entries"
                );
            }
            let upload_path = paths.upload_path();
            let links = session.exec(&commands::list_links(&upload_path)).await?;
            let remote_links = parse_link_listing(&links.stdout_lossy());
            let listing = session
                .exec_checked(&commands::list_files(&upload_path))
                .await?;
            let remote = parse_listing(&listing);
            let available = info
                .storage
                .as_ref()
                .map(|s| s.available_for_games(info.console));
            config.last_console = Some(system_code.clone());
            library.save_config(&config)?;
            Ok(Target {
                session: Some(session),
                paths,
                system_code,
                available,
                remote,
                remote_links,
            })
        }
        None if need_console => bail!("no console answered; connect it and try again"),
        None => {
            let code = args
                .console
                .clone()
                .or(config.last_console.clone())
                .context("no console answered and no --console given")?;
            let console = ConsoleType::from_system_code(&code);
            let paths = DevicePaths::for_console(console)
                .with_context(|| format!("{code:?} is not a known console type"))?;
            println!("no console answered; planning against an empty {code} console");
            Ok(Target {
                session: None,
                paths,
                system_code: code,
                available: None,
                remote: Vec::new(),
                remote_links: Vec::new(),
            })
        }
    }
}

fn make_plan(library: &Library, target: &Target, options: &MenuOptions) -> Result<SyncPlan> {
    let menu = build_menu(library, options, Some(&target.system_code))?;
    let menus = menu::export(&menu);
    let collection = cartload_library::collect(library, &menus, &target.paths)?;
    Ok(cartload_library::sync_plan(
        collection,
        &target.remote,
        &target.remote_links,
        target.paths.clone(),
        target.available,
    ))
}

fn print_plan(plan: &SyncPlan) {
    print!("{}", plan.summary());
    for file in &plan.uploads {
        println!("  upload  {}  ({} bytes)", file.path, file.size);
    }
    for path in &plan.deletions {
        println!("  remove  {path}");
    }
    for path in &plan.link_deletions {
        println!("  unlink  {path}");
    }
    for dir in &plan.empty_dirs {
        println!("  rmdir   {dir}");
    }
}

async fn run_plan(library: &Library, connect: &Connect, args: &MenuArgs, json: bool) -> Result<()> {
    let (_, options) = menu_options(library, args)?;
    let target = prepare_target(library, connect, args, false).await?;
    let plan = make_plan(library, &target, &options)?;
    if let Some(session) = target.session {
        let _ = session.close().await;
    }
    if json {
        #[derive(serde::Serialize)]
        struct PlanJson<'a> {
            paths: &'a DevicePaths,
            uploads: Vec<(&'a str, u64)>,
            deletions: &'a [String],
            link_deletions: &'a [String],
            empty_dirs: &'a [String],
            unchanged: usize,
            total_size: u64,
            upload_size: u64,
            available: Option<u64>,
        }
        let json = PlanJson {
            paths: &plan.paths,
            uploads: plan
                .uploads
                .iter()
                .map(|f| (f.path.as_str(), f.size))
                .collect(),
            deletions: &plan.deletions,
            link_deletions: &plan.link_deletions,
            empty_dirs: &plan.empty_dirs,
            unchanged: plan.unchanged,
            total_size: plan.total_size(),
            upload_size: plan.upload_size(),
            available: plan.available,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        print_plan(&plan);
    }
    Ok(())
}

async fn run_sync(library: &Library, connect: &Connect, args: &MenuArgs, yes: bool) -> Result<()> {
    let (_, options) = menu_options(library, args)?;
    let target = prepare_target(library, connect, args, true).await?;
    let plan = make_plan(library, &target, &options)?;
    print_plan(&plan);
    if !plan.fits() {
        bail!("the games do not fit in the console's free space");
    }
    if plan.is_noop() {
        println!(
            "the console is up to date; running the sync anyway re-links and remounts the games"
        );
    }
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("not a terminal: pass --yes to sync without confirmation");
        }
        print!("sync now? [y/N] ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("cancelled");
            return Ok(());
        }
    }
    let session = target.session.context("no console")?;
    let started = std::time::Instant::now();
    let mut last_path = String::new();
    let report = cartload_library::sync(&session, &plan, |event| match event {
        SyncEvent::Step {
            index,
            total,
            name,
            detail,
        } => println!("[{index}/{total}] {name}: {detail}"),
        SyncEvent::Upload {
            path,
            done,
            total,
            overall_done,
            overall_total,
        } => {
            if path != last_path || done == total {
                last_path = path.clone();
                let percent = (overall_done * 100)
                    .checked_div(overall_total)
                    .unwrap_or(100);
                println!("    {path}: {done}/{total} bytes ({percent}% overall)");
            }
        }
        SyncEvent::Log(line) => println!("    {line}"),
    })
    .await?;
    let _ = session.close().await;
    println!(
        "uploaded {} files ({} bytes), removed {} files, {} symlinks, {} directories in {:.1?}; games mounted: {}",
        report.uploaded_files,
        report.uploaded_bytes,
        report.removed_files,
        report.removed_links,
        report.removed_dirs,
        started.elapsed(),
        report.games_mounted
    );
    Ok(())
}

async fn run_fetch_db(dest: Option<PathBuf>) -> Result<()> {
    let dest = match dest {
        Some(dest) => dest,
        None => hashdb::default_cache_path().context("no cache directory on this system")?,
    };
    let len = hashdb::fetch(&dest).await?;
    let db = HashDb::load(&dest)?;
    println!("{}: {len} bytes, {} records", dest.display(), db.len());
    Ok(())
}

fn run_key(library: &Library, set: Option<String>, clear: bool) -> Result<()> {
    let mut config = library.config()?;
    if let Some(key) = set {
        let key = ApiKey::new(&key).context("the key is empty")?;
        config.tgdb_key = Some(key.as_str().to_string());
        library.save_config(&config)?;
        println!(
            "key stored in {}",
            library.root().join("library.json").display()
        );
    } else if clear {
        config.tgdb_key = None;
        library.save_config(&config)?;
        println!("stored key removed");
    }
    let source = if std::env::var_os(art::KEY_ENV).is_some_and(|v| !v.is_empty()) {
        format!("the {} environment variable", art::KEY_ENV)
    } else if config.tgdb_key.is_some() {
        "library.json".to_string()
    } else {
        format!(
            "nowhere: set {} or run `cartload-library key --set <KEY>` (keys come from https://thegamesdb.net/)",
            art::KEY_ENV
        )
    };
    println!("TheGamesDB key comes from {source}");
    if let Ok(store) = library.art_store() {
        let allowance = store.allowance()?;
        if allowance.remaining.is_some() {
            println!("{allowance}");
        }
    }
    Ok(())
}

async fn run_art(library: &Library, args: &ArtArgs) -> Result<()> {
    let single = args.codes.len() == 1 && !args.all;
    if (args.search.is_some() || args.id.is_some() || args.image.is_some()) && !single {
        bail!("--search, --id and --image apply to one game code");
    }
    let cover = CoverOptions {
        center_thumbnail: args.center_thumbnail,
        quantize: !args.no_quantize,
    };
    if let Some(image) = &args.image {
        let code = &args.codes[0];
        let set = art::set_cover_from_file(library, code, image, &cover)?;
        println!(
            "{code}: cover {}x{} and thumbnail {}x{} from {} ({}x{})",
            set.cover_size.0,
            set.cover_size.1,
            set.thumbnail_size.0,
            set.thumbnail_size.1,
            image.display(),
            set.source_size.0,
            set.source_size.1
        );
        return Ok(());
    }

    let config = library.config()?;
    let key = ApiKey::resolve(args.key.as_deref(), config.tgdb_key.as_deref())
        .ok_or(cartload_library::Error::NoApiKey)?;
    let client = Client::new(HttpTransport::new()?, key);

    let mut options = ArtOptions {
        size: args.size.parse::<ArtSize>()?,
        cover,
        force: args.force,
        research: args.research,
        query: args.search.clone(),
        metadata: !args.no_metadata,
        overwrite: args.overwrite,
        logo: !args.no_logo,
    };
    if let Some(id) = args.id {
        art::set_match(library, &args.codes[0], id)?;
        options.force = true;
    }
    let codes: Vec<String> = if args.all {
        library
            .games()?
            .into_iter()
            .filter(|g| args.force || g.placeholder_art || g.tgdb_id.is_none())
            .map(|g| g.code)
            .collect()
    } else {
        args.codes.clone()
    };
    if codes.is_empty() {
        println!("every game has art already; use --force to fetch again");
        return Ok(());
    }
    info!(games = codes.len(), size = %options.size, "fetching art");

    let json = args.json;
    let result = cartload_library::fetch_art(library, &client, &codes, &options, |event| {
        if json {
            return;
        }
        match event {
            ArtEvent::Searching { code, query } => {
                println!("{code}: searching TheGamesDB for {query:?}")
            }
            ArtEvent::Matched {
                code,
                id,
                title,
                source,
            } => println!(
                "{code}: TheGamesDB {id}{} ({})",
                if title.is_empty() {
                    String::new()
                } else {
                    format!(" {title:?}")
                },
                match source {
                    art::MatchSource::HashDb => "hash database",
                    art::MatchSource::Search => "search",
                    art::MatchSource::Manual => "chosen",
                }
            ),
            ArtEvent::NoMatch { code, queries } => {
                println!("{code}: nothing found for {queries:?}")
            }
            ArtEvent::Fetching { code, what } => match code {
                Some(code) => println!("{code}: fetching {what}"),
                None => println!("fetching {what}"),
            },
            ArtEvent::Done { code, outcome } => {
                let mut parts = Vec::new();
                if let (Some(id), Some(title)) = (outcome.id, &outcome.title) {
                    parts.push(format!("TheGamesDB {id} {title:?}"));
                }
                match outcome.cover {
                    CoverStatus::Written => parts.push(format!(
                        "cover {}x{}, thumbnail {}x{}{}",
                        outcome.cover_size.map_or(0, |s| s.0),
                        outcome.cover_size.map_or(0, |s| s.1),
                        outcome.thumbnail_size.map_or(0, |s| s.0),
                        outcome.thumbnail_size.map_or(0, |s| s.1),
                        if outcome.from_cache { " (cached)" } else { "" }
                    )),
                    CoverStatus::Unchanged => parts.push("art unchanged".to_string()),
                    CoverStatus::NoBoxArt => {
                        parts.push("no box art on TheGamesDB, placeholder kept".to_string())
                    }
                    CoverStatus::NoMatch => parts.push("no match, placeholder kept".to_string()),
                }
                if outcome.logo == art::LogoState::Present {
                    parts.push("clear logo".to_string());
                }
                if !outcome.metadata.is_empty() {
                    parts.push(format!("metadata: {}", outcome.metadata.join(", ")));
                }
                println!("{code}: {}", parts.join("; "));
            }
            ArtEvent::Failed { code, error } => eprintln!("{code}: {error}"),
        }
    })
    .await;
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            if let Ok(store) = library.art_store()
                && let Ok(allowance) = store.allowance()
                && allowance.remaining.is_some()
            {
                eprintln!("{allowance}");
            }
            return Err(error.into());
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.summary());
        if report.allowance.remaining.is_some() {
            println!("{}", report.allowance);
        }
    }
    if report.failed() > 0 {
        bail!("{} game(s) failed", report.failed());
    }
    Ok(())
}
