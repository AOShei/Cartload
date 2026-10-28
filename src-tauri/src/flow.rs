//! Running the kernel flows from the app: one at a time, observed as a [`FlowSnapshot`] that is
//! emitted on every change, and stoppable except while the console writes to its flash.
//!
//! The flows themselves live in `cartload-flow`. This module adapts their event stream to what
//! the progress window shows (one entry per step with its state, timing and last message), pauses
//! the console watcher while a flow owns the USB link, and turns a failure into a
//! [`FlowFailure`] naming the likely cause and the next action.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use cartload_flow::{
    Backend, ConsoleState, Context, Event, EventReceiver, Flow, FlowOptions, HmodPackage, Level,
    Outcome, Reporter, StepInfo, UsbBackend, channel, flows,
};
use cartload_hmod::{basemods, modhub::ModArchive};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use tracing::{info, warn};

use crate::{
    error::CommandError,
    failure::FlowFailure,
    state::AppState,
    status::{ConsoleStatus, now_ms},
    watcher::Watcher,
};

/// Which flow to run, with the inputs that are not the package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowKind {
    /// Install or repair the hakchi kernel, then the built-in base mods (`tiny7zx`, which
    /// compressed games need).
    Install,
    /// Boot the hakchi kernel from RAM and stop there.
    Memboot,
    /// Return the console to its original system.
    Uninstall {
        /// A stock kernel dump to flash when the console has no backup of its own.
        #[serde(default)]
        stock_kernel: Option<PathBuf>,
        /// Flash `stock_kernel` even when its MD5 is not a known stock image.
        #[serde(default)]
        force_stock_kernel: bool,
    },
    /// Write the stock kernel backup to a file.
    DumpKernel {
        /// The output file.
        path: PathBuf,
    },
    /// Install emulator modules (RetroArch and cores) on the running hakchi kernel.
    InstallModules {
        /// Catalog ids (`retroarch`, `fceumm`, `snes9x`).
        ids: Vec<String>,
    },
    /// Replace emulator modules on the running hakchi kernel with the hub's current builds.
    UpdateModules {
        /// Catalog ids.
        ids: Vec<String>,
    },
    /// Uninstall emulator modules from the running hakchi kernel.
    RemoveModules {
        /// Catalog ids; `retroarch` takes every core with it.
        ids: Vec<String>,
    },
    /// Add the built-in base mods (`tiny7zx`) to the running hakchi kernel; the sync runs it
    /// first when compressed games meet a console without 7z support.
    BaseMods,
}

impl FlowKind {
    /// The flow's name in `cartload-flow`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Install => flows::INSTALL,
            Self::Memboot => flows::MEMBOOT,
            Self::Uninstall { .. } => flows::UNINSTALL,
            Self::DumpKernel { .. } => flows::DUMP_KERNEL,
            Self::InstallModules { .. } => flows::MODS,
            Self::UpdateModules { .. } => flows::UPDATE_MODS,
            Self::RemoveModules { .. } => flows::REMOVE_MODS,
            Self::BaseMods => flows::BASE_MODS,
        }
    }

    /// What the flow does, as a progressive-tense title for the status bar.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Install => "Installing hakchi",
            Self::Memboot => "Booting hakchi from memory",
            Self::Uninstall { .. } => "Returning the console to its original system",
            Self::DumpKernel { .. } => "Saving the stock kernel",
            Self::InstallModules { .. } => "Installing emulator modules",
            Self::UpdateModules { .. } => "Updating emulator modules",
            Self::RemoveModules { .. } => "Removing emulator modules",
            Self::BaseMods => "Adding 7z support",
        }
    }

    /// Whether the flow cannot start without `hakchi.hmod`.
    pub fn needs_package(&self) -> bool {
        matches!(self, Self::Install | Self::Memboot)
    }

    /// Whether a retry of `other` may continue with this flow's context.
    pub fn same_work(&self, other: &Self) -> bool {
        self.name() == other.name()
    }

    /// Builds the flow.
    pub fn build(&self) -> Flow<Context> {
        match self {
            Self::Install => flows::install_kernel_with_base_mods(),
            Self::Memboot => flows::memboot(),
            Self::Uninstall { .. } => flows::uninstall_kernel(),
            Self::DumpKernel { path } => flows::dump_stock_kernel(path.clone()),
            Self::InstallModules { .. } => flows::install_mods(),
            Self::UpdateModules { .. } => flows::update_mods(),
            Self::RemoveModules { .. } => flows::remove_mods(),
            Self::BaseMods => flows::install_base_mods(),
        }
    }

    /// The flow's steps, in order.
    pub fn steps(&self) -> Vec<StepInfo> {
        self.build().steps()
    }

    /// Copies the flow's inputs into the options.
    pub fn apply_options(&self, options: &mut FlowOptions) {
        if let Self::Uninstall {
            stock_kernel,
            force_stock_kernel,
        } = self
        {
            options.stock_kernel = stock_kernel.clone();
            options.force_stock_kernel = *force_stock_kernel;
        }
    }
}

/// Short label for a step name, for the step list and the status bar.
pub fn step_label(name: &str) -> &str {
    match name {
        "memboot" => "Boot from memory",
        "wait-console" => "Wait for the console",
        "detect" => "Identify the console",
        "check-md" => "Compatibility check",
        "grow" => "Grow the data partition",
        "reboot-grown" => "Reboot into the grown partition",
        "prepare" => "Prepare the install",
        "upload" => "Upload the package",
        "install" => "Run the installer",
        "verify" => "Verify the install",
        "base-mods" => "Add 7z support",
        "verify-mods" => "Verify 7z support",
        "read-stock-kernel" => "Read the stock kernel backup",
        "flash-stock-kernel" => "Flash the stock kernel",
        "remove" => "Remove hakchi",
        "reboot" => "Reboot the console",
        "wait-offline" => "Wait for the console to leave",
        "dump-stock-kernel" => "Save the stock kernel",
        "mods" => "Install the emulator modules",
        "verify-emulators" => "Verify the emulator modules",
        "update-emulators" => "Update the emulator modules",
        "remove-emulators" => "Remove the emulator modules",
        "verify-removed" => "Verify the modules are gone",
        other => other,
    }
}

/// Steps during which stopping would leave the console's flash half written. A stop requested
/// while one of them runs takes effect when it ends.
pub const UNSAFE_TO_STOP: [&str; 4] = ["grow", "install", "flash-stock-kernel", "remove"];

/// Whether a flow may be stopped while `step` runs.
pub fn safe_to_stop(step: &str) -> bool {
    !UNSAFE_TO_STOP.contains(&step)
}

/// State of one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    /// Not reached yet.
    Pending,
    /// Running now.
    Running,
    /// Did its work.
    Done,
    /// Failed; the flow stopped here.
    Failed,
    /// Not needed on this console. A step whose goal was already reached is [`Done`] instead,
    /// with the reason as its message.
    ///
    /// [`Done`]: StepState::Done
    Skipped,
}

/// Progress of a transfer inside the running step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transfer {
    /// What is being transferred.
    pub label: String,
    /// Bytes done.
    pub done: u64,
    /// Bytes in total.
    pub total: u64,
}

/// One step as the progress window shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSnapshot {
    /// Zero-based position in the flow.
    pub index: usize,
    /// Stable identifier (`memboot`).
    pub name: String,
    /// Short label (`Boot from memory`).
    pub label: String,
    /// One sentence describing the step.
    pub description: String,
    /// Where it is.
    pub state: StepState,
    /// Unix time in milliseconds when it started, while running or after.
    pub started_at: Option<u64>,
    /// How long it took, once finished.
    pub elapsed_ms: Option<u64>,
    /// The last log line while running, the reason when skipped, the error when failed.
    pub message: Option<String>,
    /// Transfer progress while running.
    pub transfer: Option<Transfer>,
}

impl StepSnapshot {
    fn pending(info: &StepInfo) -> Self {
        Self {
            index: info.index,
            name: info.name.clone(),
            label: step_label(&info.name).to_string(),
            description: info.description.clone(),
            state: StepState::Pending,
            started_at: None,
            elapsed_ms: None,
            message: None,
            transfer: None,
        }
    }
}

/// State of the flow as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowState {
    /// No flow has run.
    Idle,
    /// A flow is running.
    Running,
    /// Every step finished.
    Done,
    /// A step failed; see [`FlowSnapshot::failure`].
    Failed,
    /// Stopped on request.
    Cancelled,
}

/// Everything the progress window shows about the current or last flow. Emitted as the
/// `flow-progress` event on every change and returned by `flow_status`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowSnapshot {
    /// Increases with every change, so stale events can be ignored.
    pub seq: u64,
    /// The flow, once one has started.
    pub kind: Option<FlowKind>,
    /// Where the flow is.
    pub state: FlowState,
    /// The steps, in order.
    pub steps: Vec<StepSnapshot>,
    /// Index of the running (or the failed) step.
    pub current: Option<usize>,
    /// The last log line.
    pub message: Option<String>,
    /// Why the flow failed.
    pub failure: Option<FlowFailure>,
    /// A stop was requested during a step that must finish first.
    pub cancel_pending: bool,
    /// Unix time in milliseconds when the last run started.
    pub started_at: Option<u64>,
    /// Unix time in milliseconds when the last run ended.
    pub finished_at: Option<u64>,
}

impl Default for FlowSnapshot {
    fn default() -> Self {
        Self::idle()
    }
}

impl FlowSnapshot {
    /// No flow has run.
    pub fn idle() -> Self {
        Self {
            seq: 0,
            kind: None,
            state: FlowState::Idle,
            steps: Vec::new(),
            current: None,
            message: None,
            failure: None,
            cancel_pending: false,
            started_at: None,
            finished_at: None,
        }
    }

    /// A flow that is about to run: every step pending.
    pub fn starting(kind: &FlowKind, steps: &[StepInfo], now: u64) -> Self {
        Self {
            seq: 0,
            kind: Some(kind.clone()),
            state: FlowState::Running,
            steps: steps.iter().map(StepSnapshot::pending).collect(),
            current: None,
            message: None,
            failure: None,
            cancel_pending: false,
            started_at: Some(now),
            finished_at: None,
        }
    }

    /// The step that is running, if any.
    pub fn current_step(&self) -> Option<&StepSnapshot> {
        self.current
            .and_then(|index| self.steps.get(index))
            .filter(|step| step.state == StepState::Running)
    }

    fn step_mut(&mut self, index: usize) -> Option<&mut StepSnapshot> {
        self.steps.get_mut(index)
    }

    /// Folds one engine event in.
    pub fn apply(&mut self, event: &Event, now: u64) {
        match event {
            Event::FlowStarted {
                steps, resume_from, ..
            } => {
                self.state = FlowState::Running;
                self.started_at = Some(now);
                self.finished_at = None;
                self.failure = None;
                self.cancel_pending = false;
                self.current = None;
                self.message = None;
                if self.steps.len() != steps.len() {
                    self.steps = steps.iter().map(StepSnapshot::pending).collect();
                }
                for (step, info) in self.steps.iter_mut().zip(steps) {
                    if step.index >= *resume_from {
                        *step = StepSnapshot::pending(info);
                    }
                }
            }
            Event::StepStarted { step } => {
                self.current = Some(step.index);
                if let Some(entry) = self.step_mut(step.index) {
                    entry.state = StepState::Running;
                    entry.started_at = Some(now);
                    entry.elapsed_ms = None;
                    entry.message = None;
                    entry.transfer = None;
                }
            }
            Event::StepFinished {
                step,
                outcome,
                elapsed,
            } => {
                if let Some(entry) = self.step_mut(step.index) {
                    entry.elapsed_ms = Some(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
                    entry.transfer = None;
                    match outcome {
                        Outcome::Done => entry.state = StepState::Done,
                        Outcome::AlreadyDone(reason) => {
                            entry.state = StepState::Done;
                            entry.message = Some(reason.clone());
                        }
                        Outcome::NotNeeded(reason) => {
                            entry.state = StepState::Skipped;
                            entry.message = Some(reason.clone());
                        }
                    }
                }
            }
            Event::StepFailed {
                step,
                error,
                elapsed,
            } => {
                self.current = Some(step.index);
                if let Some(entry) = self.step_mut(step.index) {
                    entry.state = StepState::Failed;
                    entry.elapsed_ms = Some(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
                    entry.message = Some(error.clone());
                    entry.transfer = None;
                }
                self.message = Some(error.clone());
            }
            Event::Log { level, message } => {
                if *level == Level::Debug {
                    return;
                }
                self.message = Some(message.clone());
                if let Some(index) = self.current
                    && let Some(entry) = self.step_mut(index)
                    && entry.state == StepState::Running
                {
                    entry.message = Some(message.clone());
                }
            }
            Event::Progress { label, done, total } => {
                if let Some(index) = self.current
                    && let Some(entry) = self.step_mut(index)
                    && entry.state == StepState::Running
                {
                    entry.transfer = Some(Transfer {
                        label: label.clone(),
                        done: *done,
                        total: *total,
                    });
                }
            }
            Event::FlowFinished { success, .. } => {
                self.finished_at = Some(now);
                self.state = if *success {
                    FlowState::Done
                } else {
                    FlowState::Failed
                };
                if *success {
                    self.current = None;
                }
            }
        }
    }

    /// Records a stop: the running step goes back to pending.
    pub fn mark_cancelled(&mut self, now: u64) {
        self.state = FlowState::Cancelled;
        self.cancel_pending = false;
        self.finished_at = Some(now);
        if let Some(index) = self.current.take()
            && let Some(entry) = self.step_mut(index)
            && entry.state == StepState::Running
        {
            entry.state = StepState::Pending;
            entry.started_at = None;
            entry.transfer = None;
            entry.message = Some("Stopped before it finished.".to_string());
        }
        self.message = Some("Stopped.".to_string());
    }

    /// Records a failure with its explanation.
    pub fn mark_failed(&mut self, failure: FlowFailure, now: u64) {
        self.state = FlowState::Failed;
        self.cancel_pending = false;
        self.finished_at.get_or_insert(now);
        if let Some(index) = failure.step_index {
            self.current = Some(index);
        }
        self.failure = Some(failure);
    }

    /// Records success.
    pub fn mark_done(&mut self, now: u64) {
        self.state = FlowState::Done;
        self.cancel_pending = false;
        self.current = None;
        self.finished_at.get_or_insert(now);
    }
}

/// Receives every snapshot change.
pub trait FlowSink: Send + Sync {
    /// Called with the new snapshot after it has been stored.
    fn flow_changed(&self, snapshot: &FlowSnapshot);
}

/// Makes a fresh backend for each flow context.
pub type BackendFactory = Arc<dyn Fn() -> Box<dyn Backend> + Send + Sync>;

/// The real backend over USB.
pub fn usb_backend() -> BackendFactory {
    Arc::new(|| Box::new(UsbBackend::default()))
}

/// Settings of the runner.
#[derive(Clone, Debug)]
pub struct RunnerOptions {
    /// Time limits for the flows.
    pub flow: FlowOptions,
    /// Look at the console again when a flow ends (off in tests: it scans USB).
    pub redetect: bool,
}

impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            flow: FlowOptions::default(),
            redetect: true,
        }
    }
}

enum Slot {
    Idle,
    Running(Running),
    Suspended(Box<Suspended>),
}

struct Running {
    kind: FlowKind,
    cancel: Arc<Notify>,
    cancel_requested: bool,
}

/// A flow that failed or was stopped: its context (package, what it learned) is kept so a
/// retry can continue, with the idempotency checks skipping the finished work.
struct Suspended {
    kind: FlowKind,
    ctx: Context,
}

/// Runs one flow at a time and keeps the latest [`FlowSnapshot`].
pub struct FlowRunner {
    state: Arc<AppState>,
    watcher: Arc<Watcher>,
    sink: Arc<dyn FlowSink>,
    backend: BackendFactory,
    options: RunnerOptions,
    slot: Mutex<Slot>,
    snapshot: Mutex<FlowSnapshot>,
}

impl FlowRunner {
    /// A runner over the shared state and the watcher, reporting to `sink`.
    pub fn new(
        state: Arc<AppState>,
        watcher: Arc<Watcher>,
        sink: Arc<dyn FlowSink>,
        backend: BackendFactory,
        options: RunnerOptions,
    ) -> Self {
        Self {
            state,
            watcher,
            sink,
            backend,
            options,
            slot: Mutex::new(Slot::Idle),
            snapshot: Mutex::new(FlowSnapshot::idle()),
        }
    }

    /// The latest snapshot.
    pub fn snapshot(&self) -> FlowSnapshot {
        self.snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Whether a flow is running.
    pub fn is_running(&self) -> bool {
        matches!(
            &*self.slot.lock().unwrap_or_else(PoisonError::into_inner),
            Slot::Running(_)
        )
    }

    /// Changes the snapshot, bumps its sequence number and announces it.
    fn publish(&self, change: impl FnOnce(&mut FlowSnapshot)) -> FlowSnapshot {
        let snapshot = {
            let mut guard = self.snapshot.lock().unwrap_or_else(PoisonError::into_inner);
            let seq = guard.seq;
            change(&mut guard);
            guard.seq = seq + 1;
            guard.clone()
        };
        self.sink.flow_changed(&snapshot);
        snapshot
    }

    /// Starts `kind`. `package` is attached to the context when given; flows that need one
    /// are refused without it. `mods` are the modules to install, `removals` the hub names
    /// of those to uninstall. A flow of the same kind that failed or was stopped is retried
    /// with its context, so finished work is skipped.
    pub fn start(
        self: &Arc<Self>,
        kind: FlowKind,
        package: Option<HmodPackage>,
        mods: Vec<ModArchive>,
        removals: Vec<String>,
    ) -> Result<FlowSnapshot, CommandError> {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        if let Slot::Running(running) = &*slot {
            return Err(CommandError::new(format!(
                "{} is still in progress. Wait for it to finish, or stop it first.",
                running.kind.title()
            )));
        }
        let (sender, receiver) = channel();
        let reporter = Reporter::new(sender.clone());
        let mut ctx = match std::mem::replace(&mut *slot, Slot::Idle) {
            Slot::Suspended(suspended) if suspended.kind.same_work(&kind) => {
                let mut ctx = suspended.ctx;
                ctx.reporter = reporter;
                ctx.expect_cycle = false;
                ctx.state = ConsoleState::Offline;
                if package.is_some() {
                    ctx.hmod = package;
                }
                if !mods.is_empty() {
                    ctx.mods = mods;
                }
                ctx.remove_mods = removals;
                ctx
            }
            _ => {
                let mut ctx = Context::new((self.backend)(), reporter, self.options.flow.clone());
                if let Some(package) = package {
                    ctx = ctx.with_hmod(package);
                }
                ctx.with_mods(mods).with_removals(removals)
            }
        };
        if matches!(
            kind,
            FlowKind::InstallModules { .. } | FlowKind::UpdateModules { .. }
        ) && ctx.mods.is_empty()
        {
            return Err(CommandError::new(
                "No emulator module is loaded. Download the modules first.",
            ));
        }
        if matches!(kind, FlowKind::RemoveModules { .. }) && ctx.remove_mods.is_empty() {
            return Err(CommandError::new(
                "No emulator module was chosen for removal.",
            ));
        }
        kind.apply_options(&mut ctx.options);
        if kind.needs_package() && ctx.hmod.is_none() {
            return Err(CommandError::new(
                "The kernel package (hakchi.hmod) is missing.",
            ));
        }
        if matches!(kind, FlowKind::Install | FlowKind::BaseMods) && ctx.base_mods.is_empty() {
            ctx.base_mods = basemods::BUNDLED
                .iter()
                .map(|m| m.load_bundled())
                .collect::<Result<_, _>>()
                .map_err(|error| CommandError::from_error(&error))?;
        }
        let flow = kind.build().with_events(sender);
        let steps = flow.steps();
        let cancel = Arc::new(Notify::new());
        *slot = Slot::Running(Running {
            kind: kind.clone(),
            cancel: Arc::clone(&cancel),
            cancel_requested: false,
        });
        drop(slot);
        let snapshot = self.publish(|s| *s = FlowSnapshot::starting(&kind, &steps, now_ms()));
        let runner = Arc::clone(self);
        tokio::spawn(async move { runner.drive(kind, flow, ctx, receiver, cancel).await });
        Ok(snapshot)
    }

    /// Asks the running flow to stop. It stops at once unless the current step writes to the
    /// console's flash, in which case it stops when that step ends.
    pub fn cancel(&self) -> Result<FlowSnapshot, CommandError> {
        let slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        let Slot::Running(running) = &*slot else {
            return Err(CommandError::new("Nothing is running."));
        };
        running.cancel.notify_one();
        drop(slot);
        Ok(self.snapshot())
    }

    /// Remembers the stop request; returns whether the flow may stop right now.
    fn request_stop(&self) -> bool {
        {
            let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
            if let Slot::Running(running) = &mut *slot {
                running.cancel_requested = true;
            }
        }
        let safe = self
            .snapshot()
            .current_step()
            .is_none_or(|step| safe_to_stop(&step.name));
        if !safe {
            warn!(
                "stop requested during a step that writes to the console; it will stop afterwards"
            );
            self.publish(|s| s.cancel_pending = true);
        }
        safe
    }

    /// Whether a pending stop request can be honoured now.
    fn stop_now(&self) -> bool {
        let requested = matches!(
            &*self.slot.lock().unwrap_or_else(PoisonError::into_inner),
            Slot::Running(Running {
                cancel_requested: true,
                ..
            })
        );
        requested
            && self
                .snapshot()
                .current_step()
                .is_none_or(|step| safe_to_stop(&step.name))
    }

    fn on_event(&self, kind: &FlowKind, total: usize, event: &Event) {
        if matches!(
            event,
            Event::Log {
                level: Level::Debug,
                ..
            }
        ) {
            return;
        }
        let now = now_ms();
        self.publish(|s| s.apply(event, now));
        if let Event::StepStarted { step } = event {
            self.watcher.publish(ConsoleStatus::Working {
                flow: kind.name().to_string(),
                title: kind.title().to_string(),
                step: Some(step_label(&step.name).to_string()),
                index: Some(step.index),
                total,
            });
        }
    }

    async fn drive(
        self: Arc<Self>,
        kind: FlowKind,
        mut flow: Flow<Context>,
        mut ctx: Context,
        mut receiver: EventReceiver,
        cancel: Arc<Notify>,
    ) {
        let pause = self.state.pause();
        self.state.drop_session().await;
        ctx.disconnect().await;
        let total = flow.len();
        self.watcher.publish(ConsoleStatus::Working {
            flow: kind.name().to_string(),
            title: kind.title().to_string(),
            step: None,
            index: None,
            total,
        });
        info!(flow = kind.name(), steps = total, "flow started");

        let mut run = Box::pin(flow.run(&mut ctx));
        let mut result = None;
        let mut stopped = false;
        loop {
            tokio::select! {
                outcome = &mut run => {
                    result = Some(outcome);
                    break;
                }
                event = receiver.recv() => {
                    let Some(event) = event else { break };
                    self.on_event(&kind, total, &event);
                    if self.stop_now() {
                        stopped = true;
                        break;
                    }
                }
                () = cancel.notified() => {
                    if self.request_stop() {
                        stopped = true;
                        break;
                    }
                }
            }
        }
        drop(run);
        while let Ok(event) = receiver.try_recv() {
            self.on_event(&kind, total, &event);
        }
        ctx.disconnect().await;

        let now = now_ms();
        let next = match (stopped, result) {
            (true, _) => {
                warn!(flow = kind.name(), "flow stopped on request");
                self.publish(|s| s.mark_cancelled(now));
                Slot::Suspended(Box::new(Suspended { kind, ctx }))
            }
            (false, Some(Ok(report))) => {
                info!(
                    flow = kind.name(),
                    steps = report.results.len(),
                    elapsed_ms = report.elapsed.as_millis() as u64,
                    "flow finished"
                );
                self.publish(|s| s.mark_done(now));
                Slot::Idle
            }
            (false, Some(Err(error))) => {
                warn!(flow = kind.name(), %error, "flow failed");
                let failure = FlowFailure::from_flow_error(&kind, &error);
                self.publish(|s| s.mark_failed(failure, now));
                Slot::Suspended(Box::new(Suspended { kind, ctx }))
            }
            (false, None) => {
                warn!(flow = kind.name(), "flow ended without a result");
                let failure = FlowFailure::internal("The flow ended without reporting a result.");
                self.publish(|s| s.mark_failed(failure, now));
                Slot::Suspended(Box::new(Suspended { kind, ctx }))
            }
        };
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = next;
        self.watcher.publish(ConsoleStatus::starting());
        drop(pause);
        if self.options.redetect {
            self.watcher.tick(true).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{net::SocketAddr, time::Duration};

    use async_trait::async_trait;
    use cartload_fel::{MembootOptions, MembootReport};
    use cartload_flow::Console;
    use cartload_hmod::{BootFiles, Version};

    use super::*;
    use crate::{
        logging::LogBuffer,
        status::StatusReport,
        watcher::{StatusSink, WatcherOptions},
    };

    /// Nothing attached: no network link, no FEL device.
    struct Nothing;

    #[async_trait]
    impl Backend for Nothing {
        async fn discover(&self) -> cartload_flow::Result<Option<SocketAddr>> {
            Ok(None)
        }
        async fn connect(&self, _: SocketAddr) -> cartload_flow::Result<Box<dyn Console>> {
            Err(cartload_flow::Error::Device("no console".into()))
        }
        async fn fel_device_present(&self) -> cartload_flow::Result<bool> {
            Ok(false)
        }
        async fn fel_memboot(
            &self,
            _: &BootFiles,
            _: &MembootOptions,
            _: &Reporter,
        ) -> cartload_flow::Result<MembootReport> {
            Err(cartload_fel::Error::DeviceNotFound.into())
        }
    }

    #[derive(Default)]
    struct Recorder {
        snapshots: Mutex<Vec<FlowSnapshot>>,
        reports: Mutex<Vec<StatusReport>>,
    }

    impl FlowSink for Recorder {
        fn flow_changed(&self, snapshot: &FlowSnapshot) {
            self.snapshots.lock().unwrap().push(snapshot.clone());
        }
    }

    impl StatusSink for Recorder {
        fn status_changed(&self, report: &StatusReport) {
            self.reports.lock().unwrap().push(report.clone());
        }
    }

    fn package() -> HmodPackage {
        HmodPackage::from_parts(
            "test.hmod",
            BootFiles {
                boot_image: b"ANDROID!".to_vec(),
                uboot: b"uboot".to_vec(),
                version: Version::parse("hakchiVersion='v1.0.4-126'").unwrap(),
            },
            b"package bytes".to_vec(),
        )
    }

    fn runner(fel_wait: Duration) -> (Arc<FlowRunner>, Arc<Recorder>, Arc<AppState>) {
        let state = Arc::new(AppState::new(Arc::new(LogBuffer::new(8))));
        let recorder = Arc::new(Recorder::default());
        let watcher = Arc::new(Watcher::new(
            Arc::clone(&state),
            recorder.clone(),
            WatcherOptions::default(),
        ));
        let options = RunnerOptions {
            flow: FlowOptions {
                fel_wait,
                poll_interval: Duration::from_millis(5),
                ..FlowOptions::default()
            },
            redetect: false,
        };
        let runner = Arc::new(FlowRunner::new(
            state.clone(),
            watcher,
            recorder.clone(),
            Arc::new(|| Box::new(Nothing)),
            options,
        ));
        (runner, recorder, state)
    }

    async fn wait_until(runner: &FlowRunner, done: impl Fn(&FlowSnapshot) -> bool) -> FlowSnapshot {
        for _ in 0..2000 {
            let snapshot = runner.snapshot();
            if done(&snapshot) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("condition not met; last snapshot {:?}", runner.snapshot());
    }

    fn info(index: usize, name: &str) -> StepInfo {
        StepInfo {
            index,
            name: name.into(),
            description: format!("{name} description"),
        }
    }

    #[test]
    fn snapshot_follows_the_engine_events() {
        let steps = vec![info(0, "memboot"), info(1, "wait-console"), info(2, "grow")];
        let mut snapshot = FlowSnapshot::starting(&FlowKind::Install, &steps, 1_000);
        assert_eq!(snapshot.state, FlowState::Running);
        assert!(snapshot.steps.iter().all(|s| s.state == StepState::Pending));
        assert_eq!(snapshot.steps[2].label, "Grow the data partition");

        snapshot.apply(
            &Event::FlowStarted {
                flow: "install".into(),
                steps: steps.clone(),
                resume_from: 0,
            },
            1_000,
        );
        snapshot.apply(
            &Event::StepStarted {
                step: steps[0].clone(),
            },
            1_100,
        );
        assert_eq!(snapshot.current, Some(0));
        assert_eq!(snapshot.steps[0].state, StepState::Running);
        assert_eq!(snapshot.steps[0].started_at, Some(1_100));
        snapshot.apply(
            &Event::Log {
                level: Level::Debug,
                message: "hidden".into(),
            },
            1_150,
        );
        assert_eq!(snapshot.message, None);
        snapshot.apply(
            &Event::Log {
                level: Level::Info,
                message: "waiting for FEL".into(),
            },
            1_200,
        );
        assert_eq!(
            snapshot.steps[0].message.as_deref(),
            Some("waiting for FEL")
        );
        snapshot.apply(
            &Event::Progress {
                label: "boot image".into(),
                done: 10,
                total: 100,
            },
            1_300,
        );
        assert_eq!(snapshot.steps[0].transfer.as_ref().unwrap().done, 10);
        snapshot.apply(
            &Event::StepFinished {
                step: steps[0].clone(),
                outcome: Outcome::Done,
                elapsed: Duration::from_millis(1_500),
            },
            2_600,
        );
        assert_eq!(snapshot.steps[0].state, StepState::Done);
        assert_eq!(snapshot.steps[0].elapsed_ms, Some(1_500));
        assert!(snapshot.steps[0].transfer.is_none());

        snapshot.apply(
            &Event::StepStarted {
                step: steps[1].clone(),
            },
            2_700,
        );
        snapshot.apply(
            &Event::StepFinished {
                step: steps[1].clone(),
                outcome: Outcome::AlreadyDone("the console is already reachable".into()),
                elapsed: Duration::from_millis(20),
            },
            2_720,
        );
        assert_eq!(snapshot.steps[1].state, StepState::Done);
        assert_eq!(
            snapshot.steps[1].message.as_deref(),
            Some("the console is already reachable")
        );

        snapshot.apply(
            &Event::StepStarted {
                step: steps[2].clone(),
            },
            2_800,
        );
        snapshot.apply(
            &Event::StepFailed {
                step: steps[2].clone(),
                error: "step 3 (grow) failed: boom".into(),
                elapsed: Duration::from_millis(5),
            },
            2_805,
        );
        snapshot.apply(
            &Event::FlowFinished {
                flow: "install".into(),
                success: false,
                elapsed: Duration::from_secs(2),
            },
            2_806,
        );
        assert_eq!(snapshot.state, FlowState::Failed);
        assert_eq!(snapshot.current, Some(2));
        assert_eq!(snapshot.steps[2].state, StepState::Failed);
        assert_eq!(snapshot.finished_at, Some(2_806));

        // A retry from the start resets every step.
        snapshot.apply(
            &Event::FlowStarted {
                flow: "install".into(),
                steps: steps.clone(),
                resume_from: 0,
            },
            3_000,
        );
        assert_eq!(snapshot.state, FlowState::Running);
        assert!(snapshot.steps.iter().all(|s| s.state == StepState::Pending));
        assert!(snapshot.failure.is_none());
    }

    #[test]
    fn cancelling_puts_the_running_step_back_to_pending() {
        let steps = vec![info(0, "memboot")];
        let mut snapshot = FlowSnapshot::starting(&FlowKind::Memboot, &steps, 0);
        snapshot.apply(
            &Event::StepStarted {
                step: steps[0].clone(),
            },
            10,
        );
        snapshot.mark_cancelled(20);
        assert_eq!(snapshot.state, FlowState::Cancelled);
        assert_eq!(snapshot.current, None);
        assert_eq!(snapshot.steps[0].state, StepState::Pending);
        assert_eq!(snapshot.steps[0].started_at, None);
        assert_eq!(snapshot.finished_at, Some(20));
    }

    #[test]
    fn flash_writing_steps_are_not_safe_to_stop() {
        for name in UNSAFE_TO_STOP {
            assert!(!safe_to_stop(name), "{name}");
        }
        for name in [
            "memboot",
            "wait-console",
            "upload",
            "reboot-grown",
            "verify",
        ] {
            assert!(safe_to_stop(name), "{name}");
        }
    }

    #[test]
    fn kinds_describe_and_build_themselves() {
        assert_eq!(FlowKind::Install.name(), "install");
        assert_eq!(FlowKind::Install.steps().len(), 17);
        assert_eq!(FlowKind::Memboot.steps().len(), 3);
        assert_eq!(FlowKind::BaseMods.steps().len(), 6);
        assert_eq!(FlowKind::BaseMods.name(), "base-mods");
        let uninstall = FlowKind::Uninstall {
            stock_kernel: Some("k.img".into()),
            force_stock_kernel: true,
        };
        assert_eq!(uninstall.steps().len(), 8);
        assert!(!uninstall.needs_package());
        let mut options = FlowOptions::default();
        uninstall.apply_options(&mut options);
        assert_eq!(
            options.stock_kernel.as_deref(),
            Some(std::path::Path::new("k.img"))
        );
        assert!(options.force_stock_kernel);
        let dump = FlowKind::DumpKernel {
            path: "kernel.img".into(),
        };
        assert_eq!(dump.steps().len(), 6);
        assert!(dump.same_work(&FlowKind::DumpKernel {
            path: "other.img".into()
        }));
        assert!(!dump.same_work(&FlowKind::Install));

        let json = serde_json::to_value(&uninstall).unwrap();
        assert_eq!(json["kind"], "uninstall");
        let parsed: FlowKind =
            serde_json::from_value(serde_json::json!({"kind": "uninstall"})).unwrap();
        assert_eq!(
            parsed,
            FlowKind::Uninstall {
                stock_kernel: None,
                force_stock_kernel: false
            }
        );
        let parsed: FlowKind = serde_json::from_value(
            serde_json::json!({"kind": "dump_kernel", "path": "/tmp/k.img"}),
        )
        .unwrap();
        assert_eq!(
            parsed,
            FlowKind::DumpKernel {
                path: "/tmp/k.img".into()
            }
        );
    }

    #[tokio::test]
    async fn install_without_a_package_is_refused() {
        let (runner, recorder, _) = runner(Duration::from_millis(50));
        let error = runner
            .start(FlowKind::Install, None, Vec::new(), Vec::new())
            .unwrap_err();
        assert!(error.message.contains("hakchi.hmod"), "{}", error.message);
        assert_eq!(runner.snapshot().state, FlowState::Idle);
        assert!(recorder.snapshots.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_flow_reports_the_cause_and_releases_the_watcher() {
        let (runner, recorder, state) = runner(Duration::from_millis(50));
        let first = runner
            .start(FlowKind::Memboot, Some(package()), Vec::new(), Vec::new())
            .unwrap();
        assert_eq!(first.state, FlowState::Running);
        assert_eq!(first.steps.len(), 3);
        assert!(runner.is_running());

        let snapshot = wait_until(&runner, |s| s.state != FlowState::Running).await;
        assert_eq!(snapshot.state, FlowState::Failed);
        assert_eq!(snapshot.current, Some(0));
        assert_eq!(snapshot.steps[0].state, StepState::Failed);
        assert!(snapshot.steps[0].elapsed_ms.is_some());
        assert_eq!(snapshot.steps[1].state, StepState::Pending);
        let failure = snapshot.failure.as_ref().expect("failure");
        assert_eq!(failure.step_index, Some(0));
        assert_eq!(failure.step_name.as_deref(), Some("memboot"));
        assert!(failure.title.contains("FEL mode"), "{}", failure.title);
        assert!(failure.next.contains("hold Reset"), "{}", failure.next);
        assert!(!state.is_paused(), "the watcher is released");
        assert!(!runner.is_running());

        // The sink saw a running snapshot first and the failed one last, with rising seq.
        {
            let snapshots = recorder.snapshots.lock().unwrap();
            assert_eq!(snapshots.first().unwrap().state, FlowState::Running);
            assert_eq!(snapshots.last().unwrap().state, FlowState::Failed);
            assert!(snapshots.windows(2).all(|w| w[0].seq < w[1].seq));
        }
        // The console status went "working" and back to "looking".
        {
            let reports = recorder.reports.lock().unwrap();
            assert!(reports.iter().any(|r| matches!(
                &r.status,
                ConsoleStatus::Working { flow, index: Some(0), .. } if flow == "memboot"
            )));
            assert!(matches!(
                reports.last().unwrap().status,
                ConsoleStatus::Disconnected { .. }
            ));
        }

        // A retry of the same kind keeps the package and runs again from the start.
        let again = runner
            .start(FlowKind::Memboot, None, Vec::new(), Vec::new())
            .unwrap();
        assert_eq!(again.state, FlowState::Running);
        assert!(again.failure.is_none());
        let snapshot = wait_until(&runner, |s| s.state != FlowState::Running).await;
        assert_eq!(snapshot.state, FlowState::Failed);

        // A different kind that needs a package gets a fresh context, so it is refused.
        let error = runner
            .start(FlowKind::Install, None, Vec::new(), Vec::new())
            .unwrap_err();
        assert!(error.message.contains("hakchi.hmod"));
    }

    #[tokio::test]
    async fn only_one_flow_at_a_time_and_cancel_stops_a_waiting_step() {
        let (runner, _, state) = runner(Duration::from_secs(60));
        runner
            .start(FlowKind::Memboot, Some(package()), Vec::new(), Vec::new())
            .unwrap();
        let error = runner
            .start(FlowKind::Memboot, Some(package()), Vec::new(), Vec::new())
            .unwrap_err();
        assert!(error.message.contains("in progress"), "{}", error.message);

        wait_until(&runner, |s| s.current_step().is_some()).await;
        assert!(state.is_paused());
        let after = runner.cancel().unwrap();
        assert_eq!(after.kind, Some(FlowKind::Memboot));
        let snapshot = wait_until(&runner, |s| s.state != FlowState::Running).await;
        assert_eq!(snapshot.state, FlowState::Cancelled);
        assert_eq!(snapshot.steps[0].state, StepState::Pending);
        assert_eq!(
            snapshot.steps[0].message.as_deref(),
            Some("Stopped before it finished.")
        );
        assert!(!state.is_paused());
        assert!(runner.cancel().is_err(), "nothing runs any more");
    }
}
