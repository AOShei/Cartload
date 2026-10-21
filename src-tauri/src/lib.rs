//! Tauri shell for Cartload: thin commands over the `cartload-*` crates.
//!
//! The shell owns one long-lived [`state::AppState`] (the console session, the latest status,
//! the log buffer) and a [`watcher::Watcher`] that looks for the console every two seconds.
//! Every status change reaches the frontend as a `console-status` event, every log line as a
//! `log` event; the commands in [`commands`] answer direct questions.

pub mod commands;
pub mod error;
pub mod logging;
pub mod state;
pub mod status;
pub mod usb;
pub mod watcher;

use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Emitter};

use crate::{
    logging::LogBuffer,
    state::AppState,
    status::StatusReport,
    watcher::{StatusSink, Watcher, WatcherOptions},
};

/// Event carrying a [`StatusReport`] on every change.
pub const STATUS_EVENT: &str = "console-status";
/// Event carrying a [`logging::LogLine`].
pub const LOG_EVENT: &str = "log";

/// Emits status changes to the webview once the app handle exists.
#[derive(Default)]
struct TauriSink {
    handle: OnceLock<AppHandle>,
}

impl StatusSink for TauriSink {
    fn status_changed(&self, report: &StatusReport) {
        if let Some(handle) = self.handle.get()
            && let Err(error) = handle.emit(STATUS_EVENT, report)
        {
            eprintln!("could not emit {STATUS_EVENT}: {error}");
        }
    }
}

/// The command handler, with `console_exec` only when the `dev-exec` feature is on.
fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    #[cfg(feature = "dev-exec")]
    {
        tauri::generate_handler![
            commands::app_version,
            commands::console_status,
            commands::console_info,
            commands::fel_probe,
            commands::log_recent,
            commands::console_exec,
        ]
    }
    #[cfg(not(feature = "dev-exec"))]
    {
        tauri::generate_handler![
            commands::app_version,
            commands::console_status,
            commands::console_info,
            commands::fel_probe,
            commands::log_recent,
        ]
    }
}

/// Builds and runs the Tauri application.
pub fn run() {
    let (log, log_feed) = logging::init(LogBuffer::DEFAULT_CAPACITY);
    let state = Arc::new(AppState::new(log));
    let sink = Arc::new(TauriSink::default());
    let watcher = Arc::new(Watcher::new(
        Arc::clone(&state),
        sink.clone(),
        WatcherOptions::default(),
    ));
    let result = tauri::Builder::default()
        .manage(state)
        .manage(Arc::clone(&watcher))
        .setup(move |app| {
            let handle = app.handle().clone();
            let _ = sink.handle.set(handle.clone());
            tauri::async_runtime::spawn(logging::pump(log_feed, move |line| {
                if let Err(error) = handle.emit(LOG_EVENT, line) {
                    eprintln!("could not emit {LOG_EVENT}: {error}");
                }
            }));
            tauri::async_runtime::spawn(watcher.run());
            tracing::info!(version = commands::version().app, "Cartload started");
            Ok(())
        })
        .invoke_handler(handler())
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("error while running Cartload: {err}");
        std::process::exit(1);
    }
}
