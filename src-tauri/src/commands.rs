//! The commands the frontend invokes. Thin: they read the shared state, ask the watcher, or
//! hand a flow to the runner.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    PACKAGE_EVENT,
    error::CommandError,
    flow::{FlowKind, FlowRunner, FlowSnapshot, StepSnapshot},
    logging::LogLine,
    package::{self, DownloadProgress, PackageStatus},
    state::AppState,
    status::StatusReport,
    watcher::{FelProbe, Watcher},
};

/// Version information reported to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct AppVersion {
    /// Application version from `Cargo.toml`.
    pub app: String,
    /// Versions of the library crates linked into this build.
    pub crates: Vec<String>,
    /// Optional features compiled in (`dev-exec`).
    pub features: Vec<String>,
    /// Whether this is a debug build.
    pub debug: bool,
}

/// The version information of this build.
pub fn version() -> AppVersion {
    AppVersion {
        app: env!("CARGO_PKG_VERSION").to_owned(),
        crates: vec![
            cartload_fel::version().to_owned(),
            cartload_console::version().to_owned(),
            cartload_hmod::version().to_owned(),
            cartload_library::version().to_owned(),
            cartload_flow::version().to_owned(),
        ],
        features: features(),
        debug: cfg!(debug_assertions),
    }
}

fn features() -> Vec<String> {
    let mut features = Vec::new();
    if cfg!(feature = "dev-exec") {
        features.push("dev-exec".to_string());
    }
    features
}

/// Returns the application version and the versions of the linked crates.
#[tauri::command]
pub fn app_version() -> AppVersion {
    version()
}

/// The latest status report, as last emitted with the `console-status` event.
#[tauri::command]
pub async fn console_status(state: State<'_, Arc<AppState>>) -> Result<StatusReport, CommandError> {
    Ok(state.report())
}

/// Looks at the console right now and, when connected, runs the identification sequence
/// again. Returns the resulting report (also emitted when it changed).
#[tauri::command]
pub async fn console_info(watcher: State<'_, Arc<Watcher>>) -> Result<StatusReport, CommandError> {
    Ok(watcher.tick(true).await)
}

/// Opens the attached FEL device and asks for its version block.
#[tauri::command]
pub async fn fel_probe(watcher: State<'_, Arc<Watcher>>) -> Result<FelProbe, CommandError> {
    watcher.fel_probe().await
}

/// The buffered log lines, oldest first.
#[tauri::command]
pub async fn log_recent(state: State<'_, Arc<AppState>>) -> Result<Vec<LogLine>, CommandError> {
    Ok(state.log.recent())
}

/// Starts a flow. The kernel package is attached when one is usable; flows that need it are
/// refused without it. Progress arrives as `flow-progress` events; the returned snapshot is
/// the first of them.
#[tauri::command]
pub async fn flow_start(
    runner: State<'_, Arc<FlowRunner>>,
    kind: FlowKind,
) -> Result<FlowSnapshot, CommandError> {
    if runner.is_running() {
        return Err(CommandError::new(
            "A flow is already running. Wait for it to finish, or stop it first.",
        ));
    }
    let package = package::load_for_flow().await;
    tracing::info!(
        kind = kind.name(),
        package = package.is_some(),
        "flow_start"
    );
    runner.start(kind, package)
}

/// Asks the running flow to stop. Steps that write to the console's flash finish first.
#[tauri::command]
pub async fn flow_cancel(runner: State<'_, Arc<FlowRunner>>) -> Result<FlowSnapshot, CommandError> {
    tracing::info!("flow_cancel");
    runner.cancel()
}

/// The latest flow snapshot (idle when no flow has run).
#[tauri::command]
pub async fn flow_status(runner: State<'_, Arc<FlowRunner>>) -> Result<FlowSnapshot, CommandError> {
    Ok(runner.snapshot())
}

/// The steps a flow would run, all pending, so the page can list them before starting.
#[tauri::command]
pub fn flow_steps(kind: FlowKind) -> Vec<StepSnapshot> {
    FlowSnapshot::starting(&kind, &kind.steps(), 0).steps
}

/// The kernel package that would be installed, and where a download would go.
#[tauri::command]
pub async fn package_info() -> Result<PackageStatus, CommandError> {
    Ok(package::status().await)
}

/// Downloads the latest kernel package into the cache, with `package-download` progress
/// events, and returns the new status.
#[tauri::command]
pub async fn package_update(
    app: AppHandle,
    runner: State<'_, Arc<FlowRunner>>,
) -> Result<PackageStatus, CommandError> {
    if runner.is_running() {
        return Err(CommandError::new(
            "Wait for the current flow to finish before updating the package.",
        ));
    }
    tracing::info!("package_update");
    let mut last_reported = 0u64;
    package::update(move |progress| {
        let finished = progress.total.is_some_and(|total| progress.done >= total);
        if progress.done == 0
            || finished
            || progress.done.saturating_sub(last_reported) >= 256 * 1024
        {
            last_reported = progress.done;
            if let Err(error) = app.emit(PACKAGE_EVENT, DownloadProgress::from(progress)) {
                tracing::warn!(%error, "could not emit {PACKAGE_EVENT}");
            }
        }
    })
    .await
}

/// What a shell line on the console produced.
#[cfg(feature = "dev-exec")]
#[derive(Debug, Clone, Serialize)]
pub struct ExecResult {
    /// Exit status, if the console reported one.
    pub exit_code: Option<u32>,
    /// Standard output (lossy UTF-8).
    pub stdout: String,
    /// Standard error (lossy UTF-8).
    pub stderr: String,
}

/// Runs a shell line on the connected console. Development builds only.
#[cfg(feature = "dev-exec")]
#[tauri::command]
pub async fn console_exec(
    state: State<'_, Arc<AppState>>,
    command: String,
) -> Result<ExecResult, CommandError> {
    let guard = state.session.lock().await;
    let session = guard.as_ref().ok_or_else(|| {
        CommandError::new("No console is connected. Connect one and wait for the status card.")
    })?;
    tracing::info!(command, "console_exec");
    let output = session.exec(&command).await?;
    Ok(ExecResult {
        exit_code: output.exit_code,
        stdout: output.stdout_lossy().into_owned(),
        stderr: output.stderr_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_lists_all_crates() {
        let v = version();
        assert_eq!(v.app, env!("CARGO_PKG_VERSION"));
        assert_eq!(v.crates.len(), 5);
        assert!(v.crates.iter().any(|c| c.starts_with("cartload-fel")));
        assert!(v.crates.iter().any(|c| c.starts_with("cartload-flow")));
        assert_eq!(
            v.features.contains(&"dev-exec".to_string()),
            cfg!(feature = "dev-exec")
        );
    }

    #[test]
    fn flow_steps_lists_pending_steps_with_labels() {
        let steps = flow_steps(FlowKind::Install);
        assert_eq!(steps.len(), 13);
        assert_eq!(steps[0].label, "Boot from memory");
        assert!(
            steps
                .iter()
                .all(|s| s.state == crate::flow::StepState::Pending)
        );
    }
}
