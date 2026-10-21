//! Exercises the Library page's backend without the UI: lists the library and the folder
//! tree, waits for the console, and prints the sync plan (read only; nothing is synced).
//!
//! `cargo run -p cartload-app --example plan -- [--seconds N]`

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use cartload_app_lib::{
    library::LibraryState,
    logging::{self, LogBuffer},
    state::AppState,
    status::StatusReport,
    sync::{SyncRunner, SyncSink, SyncSnapshot},
    watcher::{StatusSink, Watcher, WatcherOptions},
};

struct Stdout;

impl StatusSink for Stdout {
    fn status_changed(&self, report: &StatusReport) {
        println!("status: {}", report.title);
    }
}

impl SyncSink for Stdout {
    fn sync_changed(&self, snapshot: &SyncSnapshot) {
        println!("sync: {:?}", snapshot.state);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut seconds = 15u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => {
                seconds = args
                    .next()
                    .context("--seconds needs a number")?
                    .parse()
                    .context("--seconds needs a number")?;
            }
            other => anyhow::bail!("unknown argument {other}; use [--seconds N]"),
        }
    }

    let (log, feed) = logging::init(LogBuffer::DEFAULT_CAPACITY);
    tokio::spawn(logging::pump(feed, |line| {
        eprintln!("{:?} {}: {}", line.level, line.source, line.message);
    }));
    let state = Arc::new(AppState::new(log));
    let sink = Arc::new(Stdout);
    let watcher = Arc::new(Watcher::new(
        Arc::clone(&state),
        sink.clone(),
        WatcherOptions::default(),
    ));
    let library = Arc::new(LibraryState::open()?);
    let runner = Arc::new(SyncRunner::new(
        Arc::clone(&state),
        Arc::clone(&watcher),
        Arc::clone(&library),
        sink,
    ));

    let view = library.view()?;
    println!(
        "library: {} ({} games, {} bytes)",
        view.root,
        view.games.len(),
        view.total_size
    );
    for game in &view.games {
        println!(
            "  {}  {:<40} {}  {} bytes  art {}",
            game.game.code,
            game.game.name,
            game.system_name,
            game.size,
            if game.game.placeholder_art {
                "placeholder"
            } else {
                "yes"
            }
        );
    }

    let handle = tokio::spawn(Arc::clone(&watcher).run());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        if state.report().status.is_connected() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            println!(
                "no console answered within {seconds} s; the menu below is for the last console seen"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let menu = library.menu_view(&state.report().status)?;
    println!(
        "menu: {} root entries, structure {:?}, edited by hand: {}, console {:?}, {} firmware games",
        menu.entries.len(),
        menu.options.structure,
        menu.custom,
        menu.system_code,
        menu.originals.len()
    );

    if state.report().status.is_connected() {
        let plan = runner.plan().await?;
        println!(
            "plan: {} games, {} firmware games, {} folders, {} files, {} bytes on the console",
            plan.games, plan.originals, plan.folders, plan.files, plan.total_size
        );
        println!(
            "      upload {} files ({} bytes), keep {}, remove {} files, {} links, {} dirs; fits {}, changes {}",
            plan.uploads.len(),
            plan.upload_size,
            plan.unchanged,
            plan.deletions.len(),
            plan.link_deletions.len(),
            plan.empty_dirs.len(),
            plan.fits,
            plan.changes
        );
        for file in plan.uploads.iter().take(8) {
            println!("      upload {}  {} bytes", file.path, file.size);
        }
    }
    handle.abort();
    state.drop_session().await;
    Ok(())
}
