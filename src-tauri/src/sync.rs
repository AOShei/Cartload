//! Planning and running the sync from the app: the plan as the sync bar shows it (what goes
//! up, what comes down, whether it fits), and one sync at a time observed as a
//! [`SyncSnapshot`] that is emitted on every change. While a sync runs the watcher is paused
//! and the session is held, so nothing else talks to the console.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use cartload_console::{ConsoleInfo, Session};
use cartload_library::{
    DevicePaths, SyncEvent, SyncPlan, menu,
    sync::{self, commands, parse_link_listing, parse_listing},
};
use serde::Serialize;
use tracing::{info, warn};

use crate::{
    error::CommandError,
    library::LibraryState,
    state::AppState,
    status::{ConsoleStatus, now_ms},
    watcher::Watcher,
};

/// Event carrying a [`SyncSnapshot`] on every change of the running sync.
pub const SYNC_EVENT: &str = "sync-progress";

/// Name of the sync in the `Working` status.
pub const SYNC_FLOW: &str = "sync";

/// One file of the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileView {
    /// Path relative to the upload directory.
    pub path: String,
    /// Bytes.
    pub size: u64,
}

/// The console's storage, from the console info.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StorageView {
    /// Bytes in the games partition.
    pub total: u64,
    /// Bytes in use.
    pub used: u64,
    /// Bytes free.
    pub free: u64,
}

/// What a sync would do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncPlanView {
    /// `snes-eur`.
    pub system_code: String,
    /// Where the files go.
    pub upload_path: String,
    /// Library games in the menu.
    pub games: usize,
    /// Firmware games in the menu.
    pub originals: usize,
    /// Folders (including back links).
    pub folders: usize,
    /// Files in the whole collection.
    pub files: usize,
    /// Files to upload.
    pub uploads: Vec<FileView>,
    /// Remote files to remove.
    pub deletions: Vec<String>,
    /// Remote symlinks to remove.
    pub link_deletions: Vec<String>,
    /// Remote directories that end up empty.
    pub empty_dirs: Vec<String>,
    /// Files already on the console.
    pub unchanged: usize,
    /// Bytes the collection takes on the console.
    pub total_size: u64,
    /// Bytes to transfer.
    pub upload_size: u64,
    /// Bytes available for games, when known.
    pub available: Option<u64>,
    /// Whether the collection fits.
    pub fits: bool,
    /// Whether the console already holds everything.
    pub noop: bool,
    /// Uploads, deletions, unlinks and removed directories together.
    pub changes: usize,
    /// The console's storage.
    pub storage: Option<StorageView>,
    /// When the plan was made, milliseconds since the Unix epoch.
    pub planned_at: u64,
    /// Games that run through RetroArch, one entry per system.
    pub retroarch: Vec<RetroarchView>,
    /// Things the sync will do but that may not work on the console (missing RetroArch, ...).
    pub warnings: Vec<String>,
    /// Compressed games are in the plan and the console has no 7z support (`tiny7zx`): the
    /// app adds it (the `base_mods` flow, one restart) before it syncs.
    pub needs_7z: bool,
}

/// The games of one system that run through RetroArch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetroarchView {
    /// The system.
    pub system: cartload_library::System,
    /// Its short name.
    pub name: String,
    /// Games of the system that run through RetroArch.
    pub games: usize,
    /// Of those, games pointed at RetroArch because the console has no emulator for them.
    pub redirected: usize,
    /// The core module the games need.
    pub module: String,
    /// Whether the console has RetroArch and the core (`None` when not checked).
    pub installed: Option<bool>,
}

impl SyncPlanView {
    fn new(plan: &SyncPlan, info: &ConsoleInfo, now: u64) -> Self {
        Self {
            system_code: plan.paths.system_code.clone(),
            upload_path: plan.paths.upload_path(),
            games: plan.collection.games,
            originals: plan.collection.originals,
            folders: plan.collection.folders,
            files: plan.collection.files.len(),
            uploads: plan
                .uploads
                .iter()
                .map(|f| FileView {
                    path: f.path.clone(),
                    size: f.size,
                })
                .collect(),
            deletions: plan.deletions.clone(),
            link_deletions: plan.link_deletions.clone(),
            empty_dirs: plan.empty_dirs.clone(),
            unchanged: plan.unchanged,
            total_size: plan.total_size(),
            upload_size: plan.upload_size(),
            available: plan.available,
            fits: plan.fits(),
            noop: plan.is_noop(),
            changes: plan.uploads.len()
                + plan.deletions.len()
                + plan.link_deletions.len()
                + plan.empty_dirs.len(),
            storage: info.storage.as_ref().map(|s| StorageView {
                total: s.usage.total,
                used: s.usage.used,
                free: s.usage.free,
            }),
            planned_at: now,
            retroarch: plan
                .collection
                .retroarch
                .iter()
                .map(|(system, usage)| RetroarchView {
                    system: *system,
                    name: crate::library::short_system_name(*system).to_string(),
                    games: usage.games,
                    redirected: usage.redirected,
                    module: system.core_module().to_string(),
                    installed: None,
                })
                .collect(),
            warnings: Vec::new(),
            needs_7z: false,
        }
    }
}

/// State of the sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// No sync has run.
    Idle,
    /// Running.
    Running,
    /// Finished.
    Done,
    /// Ended with an error.
    Failed,
}

/// The running step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncStep {
    /// One-based index.
    pub index: usize,
    /// Number of steps.
    pub total: usize,
    /// The crate's step name.
    pub name: String,
    /// Label for people.
    pub label: String,
    /// What the step does.
    pub detail: String,
}

/// Progress of the file being uploaded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UploadView {
    /// Relative path.
    pub path: String,
    /// Bytes of this file sent.
    pub done: u64,
    /// Bytes of this file.
    pub total: u64,
    /// Bytes sent overall.
    pub overall_done: u64,
    /// Bytes to send overall.
    pub overall_total: u64,
}

/// What a sync did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncReportView {
    /// Files uploaded.
    pub uploaded_files: usize,
    /// Bytes uploaded.
    pub uploaded_bytes: u64,
    /// Files removed.
    pub removed_files: usize,
    /// Symlinks removed.
    pub removed_links: usize,
    /// Directories removed.
    pub removed_dirs: usize,
    /// Whether the console mounted the games.
    pub games_mounted: bool,
}

/// Lines of the log kept in the snapshot.
pub const LOG_CAP: usize = 200;

/// The sync as the Library page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncSnapshot {
    /// Sequence number, higher is newer.
    pub seq: u64,
    /// State.
    pub state: SyncState,
    /// The plan the sync runs.
    pub plan: Option<SyncPlanView>,
    /// The running (or last) step.
    pub step: Option<SyncStep>,
    /// The file being uploaded.
    pub upload: Option<UploadView>,
    /// What the console said, most recent last.
    pub log: Vec<String>,
    /// The report, once done.
    pub report: Option<SyncReportView>,
    /// The error, once failed.
    pub error: Option<CommandError>,
    /// Start time, milliseconds since the Unix epoch.
    pub started_at: Option<u64>,
    /// End time.
    pub finished_at: Option<u64>,
}

impl SyncSnapshot {
    /// Before any sync.
    pub fn idle() -> Self {
        Self {
            seq: 0,
            state: SyncState::Idle,
            plan: None,
            step: None,
            upload: None,
            log: Vec::new(),
            report: None,
            error: None,
            started_at: None,
            finished_at: None,
        }
    }

    /// Folds a crate event in.
    pub fn apply(&mut self, event: SyncEvent) {
        match event {
            SyncEvent::Step {
                index,
                total,
                name,
                detail,
            } => {
                self.step = Some(SyncStep {
                    index,
                    total,
                    name: name.to_string(),
                    label: step_label(name).to_string(),
                    detail,
                });
                self.upload = None;
            }
            SyncEvent::Upload {
                path,
                done,
                total,
                overall_done,
                overall_total,
            } => {
                self.upload = Some(UploadView {
                    path,
                    done,
                    total,
                    overall_done,
                    overall_total,
                });
            }
            SyncEvent::Log(line) => {
                self.log.push(line);
                if self.log.len() > LOG_CAP {
                    let excess = self.log.len() - LOG_CAP;
                    self.log.drain(..excess);
                }
            }
        }
    }
}

/// Short label for a sync step.
pub fn step_label(name: &str) -> &str {
    match name {
        "stop-ui" => "Stop the console's menu",
        "umount" => "Unmount the games",
        "diff" => "Compare with the console",
        "cleanup" => "Remove old files",
        "upload" => "Upload the games",
        "link" => "Link the games",
        "config" => "Write the settings",
        "finish" => "Mount the games and start the menu",
        "done" => "Done",
        other => other,
    }
}

/// Where sync snapshots go.
pub trait SyncSink: Send + Sync {
    /// The snapshot changed.
    fn sync_changed(&self, snapshot: &SyncSnapshot);
}

/// Plans and runs syncs.
pub struct SyncRunner {
    state: Arc<AppState>,
    watcher: Arc<Watcher>,
    library: Arc<LibraryState>,
    sink: Arc<dyn SyncSink>,
    snapshot: Mutex<SyncSnapshot>,
    running: AtomicBool,
}

/// The connected console, or the sentence saying why there is none.
fn connected_info(state: &AppState) -> Result<ConsoleInfo, CommandError> {
    match state.report().status {
        ConsoleStatus::Connected { info, .. } if info.can_interact => Ok(*info),
        ConsoleStatus::Connected { .. } => Err(CommandError::new(
            "The console's hakchi is too old to sync with, or it runs a recovery shell. Install hakchi again from the Console page.",
        )),
        ConsoleStatus::Working { title, .. } => Err(CommandError::new(format!(
            "{title} is in progress. Wait for it to finish."
        ))),
        _ => Err(CommandError::new(
            "No console is connected. Connect it with USB and wait for the status bar to show it.",
        )),
    }
}

impl SyncRunner {
    /// A runner over the shared state, the watcher and the library, reporting to `sink`.
    pub fn new(
        state: Arc<AppState>,
        watcher: Arc<Watcher>,
        library: Arc<LibraryState>,
        sink: Arc<dyn SyncSink>,
    ) -> Self {
        Self {
            state,
            watcher,
            library,
            sink,
            snapshot: Mutex::new(SyncSnapshot::idle()),
            running: AtomicBool::new(false),
        }
    }

    /// The latest snapshot.
    pub fn snapshot(&self) -> SyncSnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Whether a sync is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn publish(&self, change: impl FnOnce(&mut SyncSnapshot)) -> SyncSnapshot {
        let snapshot = {
            let mut guard = self.snapshot.lock().unwrap_or_else(PoisonError::into_inner);
            change(&mut guard);
            guard.seq += 1;
            guard.clone()
        };
        self.sink.sync_changed(&snapshot);
        snapshot
    }

    /// Builds the plan against the connected console (its listings are read; nothing is
    /// changed).
    pub async fn plan(&self) -> Result<SyncPlanView, CommandError> {
        if self.is_running() {
            return Err(CommandError::new("A sync is running."));
        }
        let info = connected_info(&self.state)?;
        let guard = self.state.session.lock().await;
        let session = guard.as_ref().ok_or_else(|| {
            CommandError::new("The console's session is not open yet. Try again in a moment.")
        })?;
        let (_, view) = self.plan_with(session, &info).await?;
        Ok(view)
    }

    async fn plan_with(
        &self,
        session: &Session,
        info: &ConsoleInfo,
    ) -> Result<(SyncPlan, SyncPlanView), CommandError> {
        let paths = DevicePaths::from_info(info)?;
        let library = self.library.library();
        self.library.ensure_originals(info, session).await;
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
        let mut config = library.config()?;
        if config.last_console.as_deref() != Some(paths.system_code.as_str()) {
            config.last_console = Some(paths.system_code.clone());
            library.save_config(&config)?;
        }
        let (mut tree, _, _) = self.library.menu(Some(&paths.system_code))?;
        tree.ensure_back_links();
        let menus = menu::export(&tree);
        let collection = cartload_library::collect(library, &menus, &paths)?;
        let plan =
            cartload_library::sync_plan(collection, &remote, &remote_links, paths, available);
        let mut view = SyncPlanView::new(&plan, info, now_ms());
        for usage in &mut view.retroarch {
            let has = session
                .exec(&commands::has_retroarch(usage.system))
                .await?
                .success();
            usage.installed = Some(has);
            if !has {
                let n = usage.games;
                let core = cartload_hmod::modhub::find(&usage.module)
                    .map_or(usage.module.as_str(), |m| m.title);
                view.warnings.push(format!(
                    "{n} {} game{} need{} RetroArch with the {core}, which is not installed on \
                     this console.",
                    usage.name,
                    if n == 1 { "" } else { "s" },
                    if n == 1 { "s" } else { "" },
                ));
            }
        }
        if plan.needs_extractor() && !session.exec(commands::WHICH_EXTRACTOR).await?.success() {
            view.needs_7z = true;
        }
        info!(
            uploads = view.uploads.len(),
            deletions = view.deletions.len(),
            upload_bytes = view.upload_size,
            fits = view.fits,
            "sync planned"
        );
        Ok((plan, view))
    }

    /// Runs a sync: plans against the console as it is now, then executes the plan. The
    /// returned snapshot is the final one; every change on the way is emitted. The work
    /// continues even when the caller stops waiting.
    pub async fn run(self: &Arc<Self>) -> Result<SyncSnapshot, CommandError> {
        if self.is_running() {
            return Err(CommandError::new("A sync is already running."));
        }
        let busy = self.library.begin("sync")?;
        let info = connected_info(&self.state)?;
        self.running.store(true, Ordering::SeqCst);
        let runner = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let _busy = busy;
            let snapshot = runner.drive(info).await;
            runner.running.store(false, Ordering::SeqCst);
            snapshot
        });
        handle
            .await
            .map_err(|e| CommandError::new(format!("The sync task failed: {e}")))
    }

    async fn drive(self: &Arc<Self>, info: ConsoleInfo) -> SyncSnapshot {
        let started = now_ms();
        self.publish(|s| {
            *s = SyncSnapshot::idle();
            s.state = SyncState::Running;
            s.started_at = Some(started);
        });
        let total = sync::STEPS.len();
        let pause = self.state.pause();
        self.watcher.publish(ConsoleStatus::Working {
            flow: SYNC_FLOW.to_string(),
            title: "Syncing games".to_string(),
            step: None,
            index: None,
            total,
        });
        let result = self.execute(&info, total).await;
        drop(pause);
        let now = now_ms();
        let snapshot = match result {
            Ok(report) => {
                info!(
                    uploaded_files = report.uploaded_files,
                    uploaded_bytes = report.uploaded_bytes,
                    removed_files = report.removed_files,
                    elapsed_ms = now.saturating_sub(started),
                    "sync finished"
                );
                self.publish(|s| {
                    s.state = SyncState::Done;
                    s.finished_at = Some(now);
                    s.upload = None;
                    s.report = Some(SyncReportView {
                        uploaded_files: report.uploaded_files,
                        uploaded_bytes: report.uploaded_bytes,
                        removed_files: report.removed_files,
                        removed_links: report.removed_links,
                        removed_dirs: report.removed_dirs,
                        games_mounted: report.games_mounted,
                    });
                })
            }
            Err(error) => {
                warn!(%error, "sync failed");
                let error = if error.message.ends_with("try again.") {
                    error
                } else {
                    let message = format!(
                        "{} Run the sync again once the console is back: games already copied \
                         are kept and only the rest is transferred.",
                        error.message
                    );
                    error.with_message(message)
                };
                self.publish(|s| {
                    s.state = SyncState::Failed;
                    s.finished_at = Some(now);
                    s.upload = None;
                    s.error = Some(error);
                })
            }
        };
        // The console's free space and games changed: read it again.
        self.watcher.tick(true).await;
        snapshot
    }

    async fn execute(
        self: &Arc<Self>,
        info: &ConsoleInfo,
        total: usize,
    ) -> Result<cartload_library::SyncReport, CommandError> {
        let guard = self.state.session.lock().await;
        let session = guard.as_ref().ok_or_else(|| {
            CommandError::new(
                "The console's session is not open. Connect the console and try again.",
            )
        })?;
        let (plan, view) = self.plan_with(session, info).await?;
        if !plan.fits() {
            return Err(CommandError::new(format!(
                "The games do not fit: they need {} and the console has {} for games. Remove some games and try again.",
                human_bytes(plan.total_size()),
                human_bytes(plan.available.unwrap_or(0))
            )));
        }
        self.publish(|s| s.plan = Some(view));
        let runner = Arc::clone(self);
        let report = cartload_library::sync(session, &plan, move |event| {
            if let SyncEvent::Step { index, name, .. } = &event {
                runner.watcher.publish(ConsoleStatus::Working {
                    flow: SYNC_FLOW.to_string(),
                    title: "Syncing games".to_string(),
                    step: Some(step_label(name).to_string()),
                    index: Some(index.saturating_sub(1)),
                    total,
                });
            }
            let coalesce = matches!(&event, SyncEvent::Upload { done, total, .. } if done != total)
                && runner.should_skip_upload_event();
            if coalesce {
                return;
            }
            runner.publish(|s| s.apply(event));
        })
        .await?;
        Ok(report)
    }

    /// Upload progress arrives per chunk; one snapshot every few hundred milliseconds is
    /// plenty for a bar.
    fn should_skip_upload_event(&self) -> bool {
        use std::sync::atomic::AtomicU64;
        static LAST: AtomicU64 = AtomicU64::new(0);
        let now = now_ms();
        let last = LAST.load(Ordering::Relaxed);
        if now.saturating_sub(last) < 150 {
            return true;
        }
        LAST.store(now, Ordering::Relaxed);
        false
    }
}

/// `9.3 MB`, `761 KB`.
pub fn human_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KIB * KIB * KIB {
        format!("{:.1} GB", b / (KIB * KIB * KIB))
    } else if b >= KIB * KIB {
        format!("{:.1} MB", b / (KIB * KIB))
    } else if b >= KIB {
        format!("{:.0} KB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn snapshot_folds_events() {
        let mut snapshot = SyncSnapshot::idle();
        snapshot.apply(SyncEvent::Step {
            index: 5,
            total: 9,
            name: "upload",
            detail: "3 files".into(),
        });
        assert_eq!(snapshot.step.as_ref().unwrap().label, "Upload the games");
        snapshot.apply(SyncEvent::Upload {
            path: "./a".into(),
            done: 1,
            total: 2,
            overall_done: 1,
            overall_total: 4,
        });
        assert_eq!(snapshot.upload.as_ref().unwrap().overall_total, 4);
        snapshot.apply(SyncEvent::Step {
            index: 6,
            total: 9,
            name: "link",
            detail: String::new(),
        });
        assert!(snapshot.upload.is_none(), "a new step clears the upload");
        for i in 0..(LOG_CAP + 5) {
            snapshot.apply(SyncEvent::Log(format!("line {i}")));
        }
        assert_eq!(snapshot.log.len(), LOG_CAP);
        assert_eq!(snapshot.log[0], "line 5");
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["state"], "idle");
        assert_eq!(json["step"]["name"], "link");
    }

    #[test]
    fn labels_and_bytes() {
        assert_eq!(step_label("finish"), "Mount the games and start the menu");
        assert_eq!(step_label("other"), "other");
        assert_eq!(human_bytes(500), "500 B");
        assert_eq!(human_bytes(9_340_382), "8.9 MB");
        assert_eq!(human_bytes(761_856), "744 KB");
    }

    #[tokio::test]
    async fn plan_and_run_need_a_console() {
        use crate::{logging::LogBuffer, watcher::WatcherOptions};
        struct Quiet;
        impl SyncSink for Quiet {
            fn sync_changed(&self, _: &SyncSnapshot) {}
        }
        impl crate::watcher::StatusSink for Quiet {
            fn status_changed(&self, _: &crate::status::StatusReport) {}
        }
        let (_dir, library) = crate::library::tests::temp_state();
        let state = Arc::new(AppState::new(Arc::new(LogBuffer::new(8))));
        let sink = Arc::new(Quiet);
        let watcher = Arc::new(Watcher::new(
            Arc::clone(&state),
            sink.clone(),
            WatcherOptions::default(),
        ));
        let runner = Arc::new(SyncRunner::new(state, watcher, library.clone(), sink));
        let error = runner.plan().await.unwrap_err();
        assert!(error.message.starts_with("No console is connected"));
        let error = runner.run().await.unwrap_err();
        assert!(error.message.starts_with("No console is connected"));
        assert!(!runner.is_running());
        assert!(library.busy_with().is_none(), "the slot is released");
        assert_eq!(runner.snapshot().state, SyncState::Idle);
    }
}
