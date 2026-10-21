//! State shared by the commands and the watcher for the life of the app.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicUsize, Ordering},
};

use cartload_console::Session;

use crate::{
    logging::LogBuffer,
    status::{ConsoleStatus, StatusReport, now_ms},
};

/// Long-lived application state, managed by Tauri and shared with the watcher.
pub struct AppState {
    /// The SSH session to the console, when one is open. Flows take it for exclusive use.
    pub session: tokio::sync::Mutex<Option<Session>>,
    /// Serialises the watcher's ticks with the commands that look at the console.
    pub tick_lock: tokio::sync::Mutex<()>,
    /// The most recent log lines.
    pub log: Arc<LogBuffer>,
    report: Mutex<StatusReport>,
    paused: AtomicUsize,
}

impl AppState {
    /// Fresh state: no session, status "looking for the console".
    pub fn new(log: Arc<LogBuffer>) -> Self {
        Self {
            session: tokio::sync::Mutex::new(None),
            tick_lock: tokio::sync::Mutex::new(()),
            log,
            report: Mutex::new(ConsoleStatus::starting().describe(now_ms())),
            paused: AtomicUsize::new(0),
        }
    }

    /// The latest status report.
    pub fn report(&self) -> StatusReport {
        self.report
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Replaces the latest report. Returns whether it says something new.
    pub fn set_report(&self, report: StatusReport) -> bool {
        let mut current = self.report.lock().unwrap_or_else(PoisonError::into_inner);
        let changed = !current.same_as(&report);
        *current = report;
        changed
    }

    /// Stops the watcher from probing (and from touching the session) until the guard is
    /// dropped. Flows that own the USB link hold one.
    pub fn pause(self: &Arc<Self>) -> PauseGuard {
        self.paused.fetch_add(1, Ordering::SeqCst);
        PauseGuard {
            state: Arc::clone(self),
        }
    }

    /// Whether at least one [`PauseGuard`] is alive.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst) > 0
    }

    /// Closes the session, if any.
    pub async fn drop_session(&self) {
        if let Some(session) = self.session.lock().await.take() {
            let _ = session.close().await;
        }
    }
}

/// Keeps the watcher paused while alive.
pub struct PauseGuard {
    state: Arc<AppState>,
}

impl Drop for PauseGuard {
    fn drop(&mut self) {
        self.state.paused.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_counts_guards() {
        let state = Arc::new(AppState::new(Arc::new(LogBuffer::new(8))));
        assert!(!state.is_paused());
        let a = state.pause();
        let b = state.pause();
        assert!(state.is_paused());
        drop(a);
        assert!(state.is_paused());
        drop(b);
        assert!(!state.is_paused());
    }

    #[test]
    fn report_change_detection() {
        let state = AppState::new(Arc::new(LogBuffer::new(8)));
        let same = ConsoleStatus::starting().describe(now_ms() + 5);
        assert!(!state.set_report(same));
        let other = ConsoleStatus::Disconnected {
            hint: crate::status::Hint::Nothing,
        }
        .describe(now_ms());
        assert!(state.set_report(other.clone()));
        assert!(state.report().same_as(&other));
    }
}
