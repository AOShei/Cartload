//! Tauri shell for Cartload: thin commands over the `cartload-*` crates.
//!
//! The shell owns one long-lived [`state::AppState`] (the console session, the latest status,
//! the log buffer), a [`watcher::Watcher`] that looks for the console every two seconds, and a
//! [`flow::FlowRunner`] that runs one kernel flow at a time while the watcher is paused.
//! Every status change reaches the frontend as a `console-status` event, every log line as a
//! `log` event, every flow change as a `flow-progress` event; the commands in [`commands`]
//! answer direct questions.

pub mod commands;
pub mod error;
pub mod failure;
pub mod flow;
pub mod logging;
pub mod package;
pub mod state;
pub mod status;
pub mod usb;
pub mod watcher;

use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Emitter};

use crate::{
    flow::{FlowRunner, FlowSink, FlowSnapshot, RunnerOptions},
    logging::LogBuffer,
    state::AppState,
    status::StatusReport,
    watcher::{StatusSink, Watcher, WatcherOptions},
};

/// Event carrying a [`StatusReport`] on every change.
pub const STATUS_EVENT: &str = "console-status";
/// Event carrying a [`logging::LogLine`].
pub const LOG_EVENT: &str = "log";
/// Event carrying a [`FlowSnapshot`] on every change of the running flow.
pub const FLOW_EVENT: &str = "flow-progress";
/// Event carrying a [`package::DownloadProgress`] while the package downloads.
pub const PACKAGE_EVENT: &str = "package-download";

/// Emits status and flow changes to the webview once the app handle exists.
#[derive(Default)]
struct TauriSink {
    handle: OnceLock<AppHandle>,
}

impl TauriSink {
    fn emit<T: serde::Serialize + Clone>(&self, event: &str, payload: &T) {
        if let Some(handle) = self.handle.get()
            && let Err(error) = handle.emit(event, payload)
        {
            eprintln!("could not emit {event}: {error}");
        }
    }
}

impl StatusSink for TauriSink {
    fn status_changed(&self, report: &StatusReport) {
        self.emit(STATUS_EVENT, report);
    }
}

impl FlowSink for TauriSink {
    fn flow_changed(&self, snapshot: &FlowSnapshot) {
        self.emit(FLOW_EVENT, snapshot);
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
            commands::flow_start,
            commands::flow_cancel,
            commands::flow_status,
            commands::flow_steps,
            commands::package_info,
            commands::package_update,
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
            commands::flow_start,
            commands::flow_cancel,
            commands::flow_status,
            commands::flow_steps,
            commands::package_info,
            commands::package_update,
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
    let runner = Arc::new(FlowRunner::new(
        Arc::clone(&state),
        Arc::clone(&watcher),
        sink.clone(),
        flow::usb_backend(),
        RunnerOptions::default(),
    ));
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .manage(Arc::clone(&watcher))
        .manage(runner)
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
