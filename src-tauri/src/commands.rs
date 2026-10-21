//! The commands the frontend invokes. Thin: they read the shared state or ask the watcher.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::{
    error::CommandError,
    logging::LogLine,
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
        assert_eq!(v.crates.len(), 4);
        assert!(v.crates.iter().any(|c| c.starts_with("cartload-fel")));
        assert_eq!(
            v.features.contains(&"dev-exec".to_string()),
            cfg!(feature = "dev-exec")
        );
    }
}
