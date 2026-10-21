//! Headless console watcher: the app's detection loop without the UI.
//!
//! Prints every status change (one line, or JSON with `--json`) on stdout and the live log on
//! stderr, for `--seconds N` (default 20) or until Ctrl-C.
//!
//! ```text
//! cargo run -p cartload-app --example watch -- [--json] [--seconds N]
//! ```
//!
//! Read-only: discovery probes, one SSH identification sequence when a hakchi kernel answers
//! (the same commands hakchi2 CE runs on connect), and one FEL version query when a device in
//! FEL mode is attached.

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use cartload_app_lib::{
    logging::{self, LogBuffer},
    state::AppState,
    status::StatusReport,
    watcher::{StatusSink, Watcher, WatcherOptions},
};

struct Stdout {
    json: bool,
}

impl StatusSink for Stdout {
    fn status_changed(&self, report: &StatusReport) {
        print_report(report, self.json);
    }
}

fn print_report(report: &StatusReport, json: bool) {
    if json {
        match serde_json::to_string(report) {
            Ok(line) => println!("{line}"),
            Err(error) => eprintln!("could not serialise the report: {error}"),
        }
        return;
    }
    println!(
        "[{}] {:?}: {}\n    {}",
        report.changed_at, report.tone, report.title, report.next
    );
    for detail in &report.details {
        println!("    {:<16} {}", detail.label, detail.value);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut json = false;
    let mut seconds = 20u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--seconds" => {
                seconds = args
                    .next()
                    .context("--seconds needs a number")?
                    .parse()
                    .context("--seconds needs a number")?;
            }
            other => anyhow::bail!("unknown argument {other}; use [--json] [--seconds N]"),
        }
    }

    let (log, feed) = logging::init(LogBuffer::DEFAULT_CAPACITY);
    tokio::spawn(logging::pump(feed, |_| {}));
    let state = Arc::new(AppState::new(log));
    let watcher = Arc::new(Watcher::new(
        Arc::clone(&state),
        Arc::new(Stdout { json }),
        WatcherOptions::default(),
    ));
    print_report(&state.report(), json);
    let handle = tokio::spawn(Arc::clone(&watcher).run());
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    handle.abort();
    state.drop_session().await;
    eprintln!("final status: {}", state.report().title);
    Ok(())
}
