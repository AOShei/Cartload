//! The commands the frontend invokes. Thin: they read the shared state, ask the watcher, or
//! hand a flow to the runner.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use std::path::PathBuf;

use cartload_library::{MenuOptions, menu::MenuItem};

use crate::{
    PACKAGE_EVENT,
    error::CommandError,
    flow::{FlowKind, FlowRunner, FlowSnapshot, StepSnapshot},
    library::{
        ART_EVENT, ArtOutcome, GameEdit, GameView, IMPORT_EVENT, ImportReport, KeyStatus,
        LibraryState, LibraryView, MenuEdit, MenuView,
    },
    logging::LogLine,
    package::{self, DownloadProgress, PackageStatus},
    state::AppState,
    status::StatusReport,
    sync::{SyncPlanView, SyncRunner, SyncSnapshot},
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

/// What the Linux system setup (udev rule, network profile, firewall zone) looks like on
/// this machine. Off the main thread: `firewall-cmd` takes a couple of seconds to answer.
#[tauri::command]
pub async fn setup_status() -> Result<crate::setup::SetupStatus, CommandError> {
    tauri::async_runtime::spawn_blocking(crate::setup::status)
        .await
        .map_err(|e| CommandError::new("The setup check failed.").with_detail(e.to_string()))
}

/// Installs the missing pieces of the Linux system setup, asking for administrator rights.
#[tauri::command]
pub async fn setup_install() -> Result<crate::setup::SetupStatus, CommandError> {
    tauri::async_runtime::spawn_blocking(crate::setup::install)
        .await
        .map_err(|e| CommandError::new("The setup task failed.").with_detail(e.to_string()))?
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

/// Where the log file is, and whether it exists.
#[tauri::command]
pub fn log_file() -> crate::diagnostics::LogFileInfo {
    crate::diagnostics::log_file_info()
}

/// Opens the log folder in the desktop's file manager.
#[tauri::command]
pub async fn log_reveal() -> Result<(), CommandError> {
    let dir = crate::logging::log_dir()
        .ok_or_else(|| CommandError::new("This system has no application data folder."))?;
    std::fs::create_dir_all(&dir).map_err(|e| {
        CommandError::new("Could not create the log folder.").with_detail(e.to_string())
    })?;
    tracing::info!(dir = %dir.display(), "log_reveal");
    crate::diagnostics::reveal(&dir)
}

/// A text report for a bug report: version, system, setup, console status, the last flow and
/// sync, and the recent log lines.
#[tauri::command]
pub async fn diagnostics_report(
    state: State<'_, Arc<AppState>>,
    runner: State<'_, Arc<FlowRunner>>,
    sync_runner: State<'_, Arc<SyncRunner>>,
    library: State<'_, Arc<LibraryState>>,
) -> Result<String, CommandError> {
    tracing::info!("diagnostics_report");
    let state = Arc::clone(&state);
    let flow = runner.snapshot();
    let sync = sync_runner.snapshot();
    let key = library.key_status().ok();
    tauri::async_runtime::spawn_blocking(move || {
        crate::diagnostics::report(&state, &flow, &sync, key.as_ref())
    })
    .await
    .map_err(|e| CommandError::new("The diagnostics task failed.").with_detail(e.to_string()))
}

/// Writes the diagnostics report to `path` (chosen with a save dialog).
#[tauri::command]
pub async fn diagnostics_save(
    state: State<'_, Arc<AppState>>,
    runner: State<'_, Arc<FlowRunner>>,
    sync_runner: State<'_, Arc<SyncRunner>>,
    library: State<'_, Arc<LibraryState>>,
    path: PathBuf,
) -> Result<(), CommandError> {
    tracing::info!(path = %path.display(), "diagnostics_save");
    let state = Arc::clone(&state);
    let flow = runner.snapshot();
    let sync = sync_runner.snapshot();
    let key = library.key_status().ok();
    tauri::async_runtime::spawn_blocking(move || {
        let report = crate::diagnostics::report(&state, &flow, &sync, key.as_ref());
        std::fs::write(&path, report).map_err(|e| {
            CommandError::new(format!("Could not write {}.", path.display()))
                .with_detail(e.to_string())
        })
    })
    .await
    .map_err(|e| CommandError::new("The diagnostics task failed.").with_detail(e.to_string()))?
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
    let mods = match &kind {
        FlowKind::InstallModules { ids } => crate::modules::load(ids)?,
        _ => Vec::new(),
    };
    tracing::info!(
        kind = kind.name(),
        package = package.is_some(),
        modules = mods.len(),
        "flow_start"
    );
    runner.start(kind, package, mods)
}

/// The emulator modules Cartload can install: cached here, installed on the console.
#[tauri::command]
pub async fn modules_status(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::modules::ModulesStatus, CommandError> {
    let guard = state.session.lock().await;
    Ok(crate::modules::status(guard.as_ref()).await)
}

/// Downloads modules into the cache, with `module-download` progress events, and returns the
/// new status.
#[tauri::command]
pub async fn modules_fetch(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    ids: Vec<String>,
) -> Result<crate::modules::ModulesStatus, CommandError> {
    tracing::info!(?ids, "modules_fetch");
    let mut last: Option<(String, u64)> = None;
    crate::modules::fetch(&ids, move |progress| {
        let finished = progress.total.is_some_and(|t| progress.done >= t);
        let coarse = (progress.id.clone(), progress.done / (256 * 1024));
        if finished || last.as_ref() != Some(&coarse) {
            last = Some(coarse);
            if let Err(error) = app.emit(crate::modules::MODULE_EVENT, &progress) {
                tracing::warn!(%error, "could not emit {}", crate::modules::MODULE_EVENT);
            }
        }
    })
    .await?;
    let guard = state.session.lock().await;
    Ok(crate::modules::status(guard.as_ref()).await)
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

/// Every game in the library with its size and cover URL.
#[tauri::command]
pub async fn library_list(
    library: State<'_, Arc<LibraryState>>,
) -> Result<LibraryView, CommandError> {
    library.view()
}

/// Imports files and folders; one `library-import` event per file, before and after it.
/// With `folder`, the new games go to that folder of an edited tree.
#[tauri::command]
pub async fn library_import(
    app: AppHandle,
    library: State<'_, Arc<LibraryState>>,
    paths: Vec<PathBuf>,
    folder: Option<Vec<usize>>,
) -> Result<ImportReport, CommandError> {
    let _busy = library.begin("import")?;
    tracing::info!(files = paths.len(), "library_import");
    library
        .import(&paths, folder.as_deref(), |event| {
            if let Err(error) = app.emit(IMPORT_EVENT, event) {
                tracing::warn!(%error, "could not emit {IMPORT_EVENT}");
            }
        })
        .await
}

/// Saves the editable fields of a game.
#[tauri::command]
pub async fn library_update(
    library: State<'_, Arc<LibraryState>>,
    edit: GameEdit,
) -> Result<GameView, CommandError> {
    library.update(&edit)
}

/// Removes games from the library (not from the console until the next sync).
#[tauri::command]
pub async fn library_remove(
    library: State<'_, Arc<LibraryState>>,
    codes: Vec<String>,
) -> Result<usize, CommandError> {
    tracing::info!(count = codes.len(), "library_remove");
    library.remove(&codes)
}

/// The folder tree the console will show.
#[tauri::command]
pub async fn menu_get(
    state: State<'_, Arc<AppState>>,
    library: State<'_, Arc<LibraryState>>,
) -> Result<MenuView, CommandError> {
    library.menu_view(&state.report().status)
}

/// Applies an automatic structure (replacing a tree edited by hand).
#[tauri::command]
pub async fn menu_set_structure(
    state: State<'_, Arc<AppState>>,
    library: State<'_, Arc<LibraryState>>,
    options: MenuOptions,
) -> Result<MenuView, CommandError> {
    library.set_structure(options)?;
    library.menu_view(&state.report().status)
}

/// Moves games and folders into a folder (the root for an empty path), at `index` or at the
/// end.
#[tauri::command]
pub async fn menu_move(
    state: State<'_, Arc<AppState>>,
    library: State<'_, Arc<LibraryState>>,
    items: Vec<MenuItem>,
    target: Vec<usize>,
    index: Option<usize>,
) -> Result<MenuView, CommandError> {
    let status = state.report().status;
    library.move_items(&status, &items, &target, index)?;
    library.menu_view(&status)
}

/// Adds, renames or removes a folder.
#[tauri::command]
pub async fn menu_edit(
    state: State<'_, Arc<AppState>>,
    library: State<'_, Arc<LibraryState>>,
    edit: MenuEdit,
) -> Result<MenuView, CommandError> {
    let status = state.report().status;
    library.apply_menu_edit(&status, &edit)?;
    library.menu_view(&status)
}

/// Fetches cover art and metadata from TheGamesDB; progress as `art-progress` events.
#[tauri::command]
pub async fn art_fetch(
    app: AppHandle,
    library: State<'_, Arc<LibraryState>>,
    codes: Vec<String>,
    force: bool,
) -> Result<ArtOutcome, CommandError> {
    let _busy = library.begin("art")?;
    tracing::info!(games = codes.len(), force, "art_fetch");
    library
        .fetch_art(&codes, force, |progress| {
            if let Err(error) = app.emit(ART_EVENT, progress) {
                tracing::warn!(%error, "could not emit {ART_EVENT}");
            }
        })
        .await
}

/// Uses an image file as the cover of a game.
#[tauri::command]
pub async fn art_set_cover(
    library: State<'_, Arc<LibraryState>>,
    code: String,
    path: PathBuf,
) -> Result<GameView, CommandError> {
    library.set_cover(&code, &path)
}

/// Where the TheGamesDB key comes from.
#[tauri::command]
pub async fn tgdb_key_get(
    library: State<'_, Arc<LibraryState>>,
) -> Result<KeyStatus, CommandError> {
    library.key_status()
}

/// Stores a TheGamesDB key in the library settings, or clears it.
#[tauri::command]
pub async fn tgdb_key_set(
    library: State<'_, Arc<LibraryState>>,
    key: Option<String>,
) -> Result<KeyStatus, CommandError> {
    library.set_key(key.as_deref())
}

/// What a sync would do, against the connected console (read only).
#[tauri::command]
pub async fn sync_plan(runner: State<'_, Arc<SyncRunner>>) -> Result<SyncPlanView, CommandError> {
    runner.plan().await
}

/// Runs a sync; progress as `sync-progress` events. Returns the final snapshot.
#[tauri::command]
pub async fn sync_run(runner: State<'_, Arc<SyncRunner>>) -> Result<SyncSnapshot, CommandError> {
    tracing::info!("sync_run");
    runner.run().await
}

/// The latest sync snapshot.
#[tauri::command]
pub async fn sync_status(runner: State<'_, Arc<SyncRunner>>) -> Result<SyncSnapshot, CommandError> {
    Ok(runner.snapshot())
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
