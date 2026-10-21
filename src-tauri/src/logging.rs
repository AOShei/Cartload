//! Structured log lines for the UI.
//!
//! A `tracing` layer turns every event from the app and the `cartload-*` crates into a
//! [`LogLine`], keeps the most recent ones in a ring buffer ([`LogBuffer`], so a page that
//! opens late can show the backlog) and feeds each one through a channel to whoever listens:
//! the Tauri shell emits them as `log` events, the headless example prints them.

use std::{
    collections::VecDeque,
    fmt::Write as _,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    EnvFilter, Layer, layer::Context, layer::SubscriberExt, util::SubscriberInitExt,
};

/// Severity of a [`LogLine`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Protocol-level noise.
    Trace,
    /// Each command sent to the console, USB details.
    Debug,
    /// Normal activity.
    Info,
    /// Something unexpected that was handled.
    Warn,
    /// Something failed.
    Error,
}

impl From<Level> for LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::TRACE => Self::Trace,
            Level::DEBUG => Self::Debug,
            Level::INFO => Self::Info,
            Level::WARN => Self::Warn,
            Level::ERROR => Self::Error,
        }
    }
}

/// One line of activity, as shown in the live log panel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    /// Increases by one per line, so a listener can merge the buffered backlog with live
    /// events without duplicates.
    pub seq: u64,
    /// Unix time in milliseconds.
    pub time: u64,
    /// Severity.
    pub level: LogLevel,
    /// Where the line came from: `console`, `fel`, `watcher`, `ssh`, ...
    pub source: String,
    /// The text, with the event's fields appended as `key=value`.
    pub message: String,
}

/// Ring buffer of the most recent lines.
pub struct LogBuffer {
    lines: Mutex<VecDeque<LogLine>>,
    next_seq: AtomicU64,
    capacity: usize,
}

impl LogBuffer {
    /// Lines kept by default.
    pub const DEFAULT_CAPACITY: usize = 500;

    /// A buffer keeping the last `capacity` lines.
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            next_seq: AtomicU64::new(1),
            capacity: capacity.max(1),
        }
    }

    /// Appends a line, stamping its sequence number and time, and returns it.
    pub fn push(
        &self,
        level: LogLevel,
        source: impl Into<String>,
        message: impl Into<String>,
    ) -> LogLine {
        let line = LogLine {
            seq: self.next_seq.fetch_add(1, Ordering::Relaxed),
            time: crate::status::now_ms(),
            level,
            source: source.into(),
            message: message.into(),
        };
        let mut lines = self.lines.lock().unwrap_or_else(PoisonError::into_inner);
        while lines.len() >= self.capacity {
            lines.pop_front();
        }
        lines.push_back(line.clone());
        line
    }

    /// The buffered lines, oldest first.
    pub fn recent(&self) -> Vec<LogLine> {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    /// Number of buffered lines.
    pub fn len(&self) -> usize {
        self.lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether nothing has been logged yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Maps a tracing target (`cartload_console::session`) to the short source name shown in the
/// log panel (`console`). The app's own modules show as their module name (`watcher`).
pub fn source_name(target: &str) -> String {
    let mut parts = target.split("::");
    let first = parts.next().unwrap_or(target);
    match first {
        "cartload_app_lib" | "cartload_app" => parts.next().unwrap_or("app").to_string(),
        "russh" => "ssh".to_string(),
        "nusb" => "usb".to_string(),
        other => other.strip_prefix("cartload_").unwrap_or(other).to_string(),
    }
}

/// Collects an event's `message` and its other fields as `key=value`.
#[derive(Default)]
struct MessageVisitor {
    message: String,
    fields: String,
}

impl MessageVisitor {
    fn push_field(&mut self, name: &str, value: &str) {
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        self.fields.push_str(name);
        self.fields.push('=');
        self.fields.push_str(value);
    }

    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (_, true) => self.message,
            (true, false) => self.fields,
            (false, false) => format!("{} {}", self.message, self.fields),
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            self.push_field(field.name(), &format!("{value:?}"));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.push_field(field.name(), value);
        }
    }
}

/// The `tracing` layer that feeds the UI.
pub struct UiLayer {
    buffer: Arc<LogBuffer>,
    sender: mpsc::UnboundedSender<LogLine>,
}

impl UiLayer {
    /// A layer writing into `buffer` and sending every line to `sender`.
    pub fn new(buffer: Arc<LogBuffer>, sender: mpsc::UnboundedSender<LogLine>) -> Self {
        Self { buffer, sender }
    }
}

impl<S: Subscriber> Layer<S> for UiLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let line = self.buffer.push(
            (*metadata.level()).into(),
            source_name(metadata.target()),
            visitor.finish(),
        );
        let _ = self.sender.send(line);
    }
}

/// What reaches the UI: everything at `info` and above, plus `debug` from the app and the
/// cartload crates (each command sent to the console, USB details).
pub const UI_FILTER: &str = "info,cartload_app_lib=debug,cartload_console=debug,\
                             cartload_fel=debug,cartload_hmod=debug,cartload_library=debug,\
                             cartload_flow=debug";

/// Installs the process-wide tracing subscriber: readable output on stderr (level from
/// `RUST_LOG`, default `info`) and the UI feed. Returns the ring buffer and the receiving end
/// of the live feed; see [`pump`]. Only the first call installs anything, later calls still
/// return a working (but unfed) pair.
pub fn init(capacity: usize) -> (Arc<LogBuffer>, mpsc::UnboundedReceiver<LogLine>) {
    let buffer = Arc::new(LogBuffer::new(capacity));
    let (sender, receiver) = mpsc::unbounded_channel();
    let ui = UiLayer::new(Arc::clone(&buffer), sender).with_filter(EnvFilter::new(UI_FILTER));
    let stderr = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));
    if tracing_subscriber::registry()
        .with(stderr)
        .with(ui)
        .try_init()
        .is_err()
    {
        eprintln!("tracing subscriber already installed; the UI log feed is not connected");
    }
    (buffer, receiver)
}

/// Drains the live feed, handing every line to `deliver`, until the sender side is dropped.
pub async fn pump(
    mut receiver: mpsc::UnboundedReceiver<LogLine>,
    deliver: impl Fn(&LogLine) + Send + 'static,
) {
    while let Some(line) = receiver.recv().await {
        deliver(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_caps_and_numbers_lines() {
        let buffer = LogBuffer::new(3);
        assert!(buffer.is_empty());
        for i in 0..5 {
            let line = buffer.push(LogLevel::Info, "test", format!("line {i}"));
            assert_eq!(line.seq, i + 1);
        }
        let recent = buffer.recent();
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].message, "line 2");
        assert_eq!(recent[2].message, "line 4");
        assert_eq!(recent[2].seq, 5);
        assert!(recent[0].time > 1_700_000_000_000);
    }

    #[test]
    fn source_names_are_short() {
        assert_eq!(source_name("cartload_console::session"), "console");
        assert_eq!(source_name("cartload_fel"), "fel");
        assert_eq!(source_name("cartload_app_lib::watcher"), "watcher");
        assert_eq!(source_name("cartload_app_lib"), "app");
        assert_eq!(source_name("russh::client"), "ssh");
        assert_eq!(source_name("nusb::platform"), "usb");
        assert_eq!(source_name("mdns_sd"), "mdns_sd");
    }

    #[test]
    fn visitor_appends_fields_to_the_message() {
        let mut visitor = MessageVisitor::default();
        visitor.message.push_str("SSH session established");
        visitor.push_field("address", "169.254.13.37:22");
        visitor.push_field("user", "root");
        assert_eq!(
            visitor.finish(),
            "SSH session established address=169.254.13.37:22 user=root"
        );
        let mut only_fields = MessageVisitor::default();
        only_fields.push_field("exit_code", "Some(0)");
        assert_eq!(only_fields.finish(), "exit_code=Some(0)");
    }

    #[test]
    fn levels_order_and_serialize_lowercase() {
        assert!(LogLevel::Error > LogLevel::Warn);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert_eq!(
            serde_json::to_string(&LogLevel::from(Level::WARN)).expect("json"),
            "\"warn\""
        );
    }
}
