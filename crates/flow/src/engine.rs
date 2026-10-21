//! The flow engine: an ordered list of steps with idempotency checks, a resume cursor and an
//! event channel for progress and log lines.
//!
//! The engine knows nothing about consoles. A [`Flow`] is generic over its context type `C`;
//! the hardware flows use [`Context`](crate::Context), tests use whatever they like.

use std::{fmt, time::Duration};

use async_trait::async_trait;
use serde::Serialize;
use tokio::{sync::mpsc, time::Instant};

use crate::{Error, Result};

/// Severity of an [`Event::Log`] line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Detail a UI hides by default.
    Debug,
    /// Normal progress information.
    Info,
    /// Something unexpected that did not stop the flow.
    Warn,
}

/// What a step reports when it finishes without error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
pub enum Outcome {
    /// The step did its work.
    Done,
    /// The idempotency check found the work already done, so the step did not run.
    AlreadyDone(String),
    /// The step ran and found nothing to do on this console.
    NotNeeded(String),
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Done => f.write_str("done"),
            Outcome::AlreadyDone(reason) => write!(f, "already done: {reason}"),
            Outcome::NotNeeded(reason) => write!(f, "not needed: {reason}"),
        }
    }
}

/// Position, name and description of a step in a flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StepInfo {
    /// Zero-based position in the flow.
    pub index: usize,
    /// Short identifier, stable across versions (for example `memboot`).
    pub name: String,
    /// One sentence for the user.
    pub description: String,
}

/// What happened to a step that finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StepResult {
    /// The step.
    pub step: StepInfo,
    /// How it ended.
    pub outcome: Outcome,
    /// Wall-clock time the step took, including its idempotency check.
    pub elapsed: Duration,
}

/// Summary of a completed flow run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FlowReport {
    /// The flow's name.
    pub flow: String,
    /// Every step that finished, including steps finished by earlier (resumed) runs.
    pub results: Vec<StepResult>,
    /// Wall-clock time of the last run.
    pub elapsed: Duration,
}

/// Progress and log messages a flow emits while it runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A run started. `resume_from` is the index of the first step that will run.
    FlowStarted {
        /// The flow's name.
        flow: String,
        /// All steps of the flow, in order.
        steps: Vec<StepInfo>,
        /// Index of the first step of this run (0 unless resuming).
        resume_from: usize,
    },
    /// A step started (its idempotency check runs first).
    StepStarted {
        /// The step.
        step: StepInfo,
    },
    /// A step finished without error.
    StepFinished {
        /// The step.
        step: StepInfo,
        /// How it ended.
        outcome: Outcome,
        /// Time taken.
        elapsed: Duration,
    },
    /// A step failed; the run stops here.
    StepFailed {
        /// The step.
        step: StepInfo,
        /// The error, rendered.
        error: String,
        /// Time taken.
        elapsed: Duration,
    },
    /// A log line from the running step.
    Log {
        /// Severity.
        level: Level,
        /// The text.
        message: String,
    },
    /// Progress of a transfer inside the running step.
    Progress {
        /// What is being transferred.
        label: String,
        /// Bytes done.
        done: u64,
        /// Bytes in total.
        total: u64,
    },
    /// The run ended.
    FlowFinished {
        /// The flow's name.
        flow: String,
        /// Whether every step finished.
        success: bool,
        /// Time taken by this run.
        elapsed: Duration,
    },
}

/// Sending half of the event channel.
pub type EventSender = mpsc::UnboundedSender<Event>;
/// Receiving half of the event channel.
pub type EventReceiver = mpsc::UnboundedReceiver<Event>;

/// Creates an event channel. Unbounded so that steps never block on a slow consumer.
pub fn channel() -> (EventSender, EventReceiver) {
    mpsc::unbounded_channel()
}

/// Handle through which the engine and the steps emit events. Cloneable; a reporter without
/// a channel ([`Reporter::silent`]) drops everything (log lines still go to `tracing`).
#[derive(Clone, Debug, Default)]
pub struct Reporter {
    sender: Option<EventSender>,
}

impl Reporter {
    /// A reporter that sends to `sender`.
    pub fn new(sender: EventSender) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    /// A reporter with no channel.
    pub fn silent() -> Self {
        Self::default()
    }

    /// Sends an event. A closed receiver is not an error: the flow keeps running.
    pub fn send(&self, event: Event) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }

    /// Emits a log line, to the channel and to `tracing`.
    pub fn log(&self, level: Level, message: impl Into<String>) {
        let message = message.into();
        match level {
            Level::Debug => tracing::debug!("{message}"),
            Level::Info => tracing::info!("{message}"),
            Level::Warn => tracing::warn!("{message}"),
        }
        self.send(Event::Log { level, message });
    }

    /// Emits a debug-level log line.
    pub fn debug(&self, message: impl Into<String>) {
        self.log(Level::Debug, message);
    }

    /// Emits an info-level log line.
    pub fn info(&self, message: impl Into<String>) {
        self.log(Level::Info, message);
    }

    /// Emits a warning.
    pub fn warn(&self, message: impl Into<String>) {
        self.log(Level::Warn, message);
    }

    /// Emits transfer progress.
    pub fn progress(&self, label: &str, done: u64, total: u64) {
        self.send(Event::Progress {
            label: label.to_string(),
            done,
            total,
        });
    }
}

/// One unit of work in a [`Flow`].
///
/// `already_done` is the idempotency check: the engine calls it before `run`, and when it
/// returns `Some(reason)` the step is reported as [`Outcome::AlreadyDone`] and skipped. That is
/// what makes a flow resumable: rerunning it after a failure redoes only unfinished work.
#[async_trait]
pub trait Step<C: Send>: Send + Sync {
    /// Short stable identifier (for example `memboot`).
    fn name(&self) -> &str;

    /// One sentence for the user describing what the step does.
    fn description(&self) -> &str;

    /// Upper bound on `run`; the engine turns an overrun into [`Error::Timeout`]. Steps that
    /// wait for the console implement their own, more specific, timeouts instead.
    fn timeout(&self) -> Option<Duration> {
        None
    }

    /// Whether the step's effect is already in place. `Some(reason)` skips the step.
    async fn already_done(&self, _ctx: &mut C) -> Result<Option<String>> {
        Ok(None)
    }

    /// Does the work.
    async fn run(&self, ctx: &mut C) -> Result<Outcome>;
}

/// An ordered list of steps with a resume cursor.
///
/// [`Flow::run`] executes steps from the cursor on. When a step fails the cursor stays on it,
/// so calling `run` again continues there (the failed step's `already_done` check runs first).
/// A fresh process can resume too: it builds the same flow and runs it from the start, and the
/// idempotency checks skip what is finished.
pub struct Flow<C> {
    name: String,
    steps: Vec<Box<dyn Step<C>>>,
    reporter: Reporter,
    next: usize,
    results: Vec<StepResult>,
}

impl<C: Send> fmt::Debug for Flow<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flow")
            .field("name", &self.name)
            .field(
                "steps",
                &self.steps.iter().map(|s| s.name()).collect::<Vec<_>>(),
            )
            .field("next", &self.next)
            .finish_non_exhaustive()
    }
}

impl<C: Send> Flow<C> {
    /// An empty flow.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
            reporter: Reporter::silent(),
            next: 0,
            results: Vec::new(),
        }
    }

    /// Sets the reporter the engine emits events through.
    pub fn with_reporter(mut self, reporter: Reporter) -> Self {
        self.reporter = reporter;
        self
    }

    /// Sets the event channel the engine emits events through.
    pub fn with_events(self, sender: EventSender) -> Self {
        self.with_reporter(Reporter::new(sender))
    }

    /// Appends a step.
    pub fn step<S: Step<C> + 'static>(mut self, step: S) -> Self {
        self.steps.push(Box::new(step));
        self
    }

    /// Appends a boxed step.
    pub fn push(&mut self, step: Box<dyn Step<C>>) {
        self.steps.push(step);
    }

    /// The flow's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the flow has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// All steps, in order.
    pub fn steps(&self) -> Vec<StepInfo> {
        self.steps
            .iter()
            .enumerate()
            .map(|(index, step)| StepInfo {
                index,
                name: step.name().to_string(),
                description: step.description().to_string(),
            })
            .collect()
    }

    /// Index of the next step to run.
    pub fn position(&self) -> usize {
        self.next
    }

    /// Whether every step has finished.
    pub fn is_finished(&self) -> bool {
        self.next >= self.steps.len()
    }

    /// Results of the steps that finished so far.
    pub fn results(&self) -> &[StepResult] {
        &self.results
    }

    /// The reporter.
    pub fn reporter(&self) -> &Reporter {
        &self.reporter
    }

    /// Moves the cursor back to the first step and forgets earlier results.
    pub fn reset(&mut self) {
        self.next = 0;
        self.results.clear();
    }

    /// Runs the steps from the cursor on. Stops at the first failure and leaves the cursor
    /// there, wrapping the cause in [`Error::Step`].
    pub async fn run(&mut self, ctx: &mut C) -> Result<FlowReport> {
        let started = Instant::now();
        self.reporter.send(Event::FlowStarted {
            flow: self.name.clone(),
            steps: self.steps(),
            resume_from: self.next,
        });
        while self.next < self.steps.len() {
            let index = self.next;
            let step = &self.steps[index];
            let info = StepInfo {
                index,
                name: step.name().to_string(),
                description: step.description().to_string(),
            };
            self.reporter
                .send(Event::StepStarted { step: info.clone() });
            let step_started = Instant::now();
            let result = Self::execute(step.as_ref(), ctx).await;
            let elapsed = step_started.elapsed();
            match result {
                Ok(outcome) => {
                    self.reporter.send(Event::StepFinished {
                        step: info.clone(),
                        outcome: outcome.clone(),
                        elapsed,
                    });
                    self.results.push(StepResult {
                        step: info,
                        outcome,
                        elapsed,
                    });
                    self.next += 1;
                }
                Err(error) => {
                    let error = Error::Step {
                        index,
                        name: info.name.clone(),
                        source: Box::new(error),
                    };
                    self.reporter.send(Event::StepFailed {
                        step: info,
                        error: error.to_string(),
                        elapsed,
                    });
                    self.reporter.send(Event::FlowFinished {
                        flow: self.name.clone(),
                        success: false,
                        elapsed: started.elapsed(),
                    });
                    return Err(error);
                }
            }
        }
        let elapsed = started.elapsed();
        self.reporter.send(Event::FlowFinished {
            flow: self.name.clone(),
            success: true,
            elapsed,
        });
        Ok(FlowReport {
            flow: self.name.clone(),
            results: self.results.clone(),
            elapsed,
        })
    }

    async fn execute(step: &dyn Step<C>, ctx: &mut C) -> Result<Outcome> {
        if let Some(reason) = step.already_done(ctx).await? {
            return Ok(Outcome::AlreadyDone(reason));
        }
        match step.timeout() {
            Some(limit) => tokio::time::timeout(limit, step.run(ctx))
                .await
                .map_err(|_| Error::Timeout {
                    what: format!("the {} step", step.name()),
                    timeout: limit,
                    hint: "the console may be stuck; check it and rerun the command to resume"
                        .to_string(),
                })?,
            None => step.run(ctx).await,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    /// Test context: the order in which steps ran.
    #[derive(Default)]
    struct Ctx {
        trace: Vec<String>,
    }

    struct Record {
        name: &'static str,
        outcome: Outcome,
        runs: Arc<AtomicUsize>,
    }

    fn record(name: &'static str) -> (Record, Arc<AtomicUsize>) {
        let runs = Arc::new(AtomicUsize::new(0));
        (
            Record {
                name,
                outcome: Outcome::Done,
                runs: Arc::clone(&runs),
            },
            runs,
        )
    }

    #[async_trait]
    impl Step<Ctx> for Record {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "records that it ran"
        }
        async fn run(&self, ctx: &mut Ctx) -> Result<Outcome> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            ctx.trace.push(self.name.to_string());
            Ok(self.outcome.clone())
        }
    }

    /// Fails the first time it runs, succeeds afterwards.
    struct FailOnce {
        failures_left: AtomicUsize,
        runs: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Step<Ctx> for FailOnce {
        fn name(&self) -> &str {
            "flaky"
        }
        fn description(&self) -> &str {
            "fails once"
        }
        async fn run(&self, ctx: &mut Ctx) -> Result<Outcome> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            ctx.trace.push("flaky".into());
            if self.failures_left.load(Ordering::SeqCst) > 0 {
                self.failures_left.fetch_sub(1, Ordering::SeqCst);
                return Err(Error::Device("transient".into()));
            }
            Ok(Outcome::Done)
        }
    }

    /// Reports itself as already done after it ran once (like a real idempotent step).
    struct Idempotent {
        done: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Step<Ctx> for Idempotent {
        fn name(&self) -> &str {
            "idempotent"
        }
        fn description(&self) -> &str {
            "skips when its work exists"
        }
        async fn already_done(&self, _ctx: &mut Ctx) -> Result<Option<String>> {
            Ok((self.done.load(Ordering::SeqCst) > 0).then(|| "work exists".to_string()))
        }
        async fn run(&self, ctx: &mut Ctx) -> Result<Outcome> {
            self.done.fetch_add(1, Ordering::SeqCst);
            ctx.trace.push("idempotent".into());
            Ok(Outcome::Done)
        }
    }

    /// Sleeps longer than its engine-level time limit.
    struct Slow;

    #[async_trait]
    impl Step<Ctx> for Slow {
        fn name(&self) -> &str {
            "slow"
        }
        fn description(&self) -> &str {
            "never finishes in time"
        }
        fn timeout(&self) -> Option<Duration> {
            Some(Duration::from_secs(5))
        }
        async fn run(&self, _ctx: &mut Ctx) -> Result<Outcome> {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Ok(Outcome::Done)
        }
    }

    /// A waiting step that gives up with its own message, like the hardware steps do.
    struct Waiter;

    #[async_trait]
    impl Step<Ctx> for Waiter {
        fn name(&self) -> &str {
            "wait-console"
        }
        fn description(&self) -> &str {
            "waits for the console"
        }
        async fn run(&self, _ctx: &mut Ctx) -> Result<Outcome> {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Err(Error::Timeout {
                what: "the console reappearing on the USB network".into(),
                timeout: Duration::from_secs(60),
                hint: "check the USB cable and that FEL mode was entered".into(),
            })
        }
    }

    fn drain(receiver: &mut EventReceiver) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    #[tokio::test]
    async fn runs_steps_in_order_and_reports_every_event() {
        let (sender, mut receiver) = channel();
        let (a, a_runs) = record("a");
        let (b, _) = record("b");
        let c = Record {
            name: "c",
            outcome: Outcome::NotNeeded("nothing to grow".into()),
            runs: Arc::new(AtomicUsize::new(0)),
        };
        let mut flow = Flow::new("test")
            .with_events(sender)
            .step(a)
            .step(b)
            .step(c);
        assert_eq!(flow.len(), 3);
        assert_eq!(
            flow.steps()
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );

        let mut ctx = Ctx::default();
        let report = flow.run(&mut ctx).await.unwrap();

        assert_eq!(ctx.trace, ["a", "b", "c"]);
        assert_eq!(a_runs.load(Ordering::SeqCst), 1);
        assert!(flow.is_finished());
        assert_eq!(report.flow, "test");
        assert_eq!(report.results.len(), 3);
        assert_eq!(
            report.results[2].outcome,
            Outcome::NotNeeded("nothing to grow".into())
        );

        let events = drain(&mut receiver);
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                Event::FlowStarted { resume_from: 0, .. } => "flow-started",
                Event::StepStarted { .. } => "step-started",
                Event::StepFinished { .. } => "step-finished",
                Event::FlowFinished { success: true, .. } => "flow-finished",
                other => panic!("unexpected event {other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "flow-started",
                "step-started",
                "step-finished",
                "step-started",
                "step-finished",
                "step-started",
                "step-finished",
                "flow-finished",
            ]
        );
        match &events[0] {
            Event::FlowStarted { steps, .. } => assert_eq!(steps.len(), 3),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn already_done_skips_the_step_and_says_why() {
        let done = Arc::new(AtomicUsize::new(0));
        let mut flow = Flow::new("test").step(Idempotent {
            done: Arc::clone(&done),
        });
        let mut ctx = Ctx::default();
        let first = flow.run(&mut ctx).await.unwrap();
        assert_eq!(first.results[0].outcome, Outcome::Done);

        flow.reset();
        let second = flow.run(&mut ctx).await.unwrap();
        assert_eq!(
            second.results[0].outcome,
            Outcome::AlreadyDone("work exists".into())
        );
        assert_eq!(
            ctx.trace,
            ["idempotent"],
            "run was not called the second time"
        );
        assert_eq!(
            second.results[0].outcome.to_string(),
            "already done: work exists"
        );
    }

    #[tokio::test]
    async fn resume_continues_from_the_failed_step() {
        let (sender, mut receiver) = channel();
        let (a, a_runs) = record("a");
        let (b, b_runs) = record("b");
        let flaky_runs = Arc::new(AtomicUsize::new(0));
        let flaky = FailOnce {
            failures_left: AtomicUsize::new(1),
            runs: Arc::clone(&flaky_runs),
        };
        let (d, d_runs) = record("d");
        let mut flow = Flow::new("test")
            .with_events(sender)
            .step(a)
            .step(b)
            .step(flaky)
            .step(d);
        let mut ctx = Ctx::default();

        let error = flow.run(&mut ctx).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "step 3 (flaky) failed: the console reported a failure: transient"
        );
        assert!(matches!(error, Error::Step { index: 2, .. }));
        assert_eq!(flow.position(), 2);
        assert!(!flow.is_finished());
        assert_eq!(flow.results().len(), 2);
        let events = drain(&mut receiver);
        assert!(matches!(
            events.last(),
            Some(Event::FlowFinished { success: false, .. })
        ));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::StepFailed { step, .. } if step.index == 2))
        );

        let report = flow.run(&mut ctx).await.unwrap();
        assert_eq!(ctx.trace, ["a", "b", "flaky", "flaky", "d"]);
        assert_eq!(a_runs.load(Ordering::SeqCst), 1);
        assert_eq!(b_runs.load(Ordering::SeqCst), 1);
        assert_eq!(flaky_runs.load(Ordering::SeqCst), 2);
        assert_eq!(d_runs.load(Ordering::SeqCst), 1);
        assert_eq!(report.results.len(), 4, "earlier results are kept");
        let events = drain(&mut receiver);
        assert!(matches!(
            events.first(),
            Some(Event::FlowStarted { resume_from: 2, .. })
        ));
        assert!(flow.is_finished());
    }

    #[tokio::test(start_paused = true)]
    async fn engine_timeout_propagates_with_the_step_name() {
        let mut flow = Flow::new("test").step(Slow);
        let mut ctx = Ctx::default();
        let error = flow.run(&mut ctx).await.unwrap_err();
        assert!(error.is_timeout());
        assert_eq!(
            error.to_string(),
            "step 1 (slow) failed: the slow step did not happen within 5 s; the console may be \
             stuck; check it and rerun the command to resume"
        );
        assert_eq!(flow.position(), 0, "a timed-out step is rerun on resume");
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiting_steps_own_timeout_propagates_unchanged() {
        let (a, _) = record("a");
        let mut flow = Flow::new("install").step(a).step(Waiter);
        let mut ctx = Ctx::default();
        let error = flow.run(&mut ctx).await.unwrap_err();
        assert!(error.is_timeout());
        match error.root() {
            Error::Timeout { timeout, hint, .. } => {
                assert_eq!(*timeout, Duration::from_secs(60));
                assert_eq!(hint, "check the USB cable and that FEL mode was entered");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(
            error.to_string(),
            "step 2 (wait-console) failed: the console reappearing on the USB network did not \
             happen within 60 s; check the USB cable and that FEL mode was entered"
        );
        assert_eq!(flow.position(), 1);
    }

    #[tokio::test]
    async fn reset_runs_everything_again() {
        let (a, a_runs) = record("a");
        let mut flow = Flow::new("test").step(a);
        let mut ctx = Ctx::default();
        flow.run(&mut ctx).await.unwrap();
        flow.run(&mut ctx).await.unwrap();
        assert_eq!(
            a_runs.load(Ordering::SeqCst),
            1,
            "a finished flow does nothing"
        );
        flow.reset();
        flow.run(&mut ctx).await.unwrap();
        assert_eq!(a_runs.load(Ordering::SeqCst), 2);
        assert_eq!(flow.results().len(), 1);
    }

    #[test]
    fn silent_reporter_drops_events_and_closed_receiver_is_ignored() {
        Reporter::silent().info("nobody listens");
        let (sender, receiver) = channel();
        drop(receiver);
        Reporter::new(sender).progress("x", 1, 2);
    }

    #[test]
    fn events_serialise_for_the_ui() {
        let event = Event::StepFinished {
            step: StepInfo {
                index: 0,
                name: "memboot".into(),
                description: "d".into(),
            },
            outcome: Outcome::AlreadyDone("in the memboot shell".into()),
            elapsed: Duration::from_millis(1500),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"step_finished\""));
        assert!(json.contains("\"kind\":\"already_done\""));
    }
}
