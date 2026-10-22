//! Structured log lines for the UI.
//!
//! A `tracing` layer turns every event from the app and the `cartload-*` crates into a
//! [`LogLine`], keeps the most recent ones in a ring buffer ([`LogBuffer`], so a page that
//! opens late can show the backlog) and feeds each one through a channel to whoever listens:
//! the Tauri shell emits them as `log` events, the headless example prints them.
//!
//! A second layer ([`FileLayer`]) writes the same lines to a log file in the data directory
//! ([`log_dir`]), rotated by size, so a problem can be reported after the window is gone.

use std::{
    collections::VecDeque,
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
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

/// Name of the current log file inside [`log_dir`].
pub const LOG_FILE: &str = "cartload.log";
/// A log file larger than this is rotated before the next line is written.
pub const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
/// Rotated files kept (`cartload.log.1` is the newest).
pub const KEPT_LOG_FILES: u32 = 3;

/// Where the log files live: `~/.local/share/cartload/logs` on Linux, the equivalent
/// application data folder elsewhere.
pub fn log_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("cartload").join("logs"))
}

/// The current log file.
pub fn log_path() -> Option<PathBuf> {
    log_dir().map(|dir| dir.join(LOG_FILE))
}

/// `2026-10-22T09:41:03.120Z` for a unix time in milliseconds.
pub fn format_timestamp(ms: u64) -> String {
    let seconds = i64::try_from(ms / 1000).unwrap_or(i64::MAX);
    let millis = ms % 1000;
    let days = seconds.div_euclid(86_400);
    let in_day = seconds.rem_euclid(86_400);
    let (year, month, day) = cartload_library::sync::civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        in_day / 3600,
        (in_day % 3600) / 60,
        in_day % 60
    )
}

/// One line as written to the file: timestamp, level, source and message.
pub fn format_file_line(line: &LogLine) -> String {
    format!(
        "{} {:<5} {}: {}\n",
        format_timestamp(line.time),
        format!("{:?}", line.level).to_lowercase(),
        line.source,
        line.message
    )
}

/// Appends lines to a file and rotates it when it grows past a limit.
pub struct FileWriter {
    path: PathBuf,
    file: Option<File>,
    written: u64,
    max_bytes: u64,
    kept: u32,
}

impl FileWriter {
    /// Opens (or creates) `path` for appending, creating its directory.
    pub fn open(path: impl Into<PathBuf>, max_bytes: u64, kept: u32) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path,
            file: Some(file),
            written,
            max_bytes,
            kept,
        })
    }

    /// The file being written.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes `text`, rotating first when the file is already past the limit. A write that
    /// fails disables the writer: logging must never take the app down.
    pub fn write(&mut self, text: &str) {
        if self.written >= self.max_bytes {
            self.rotate();
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };
        if file.write_all(text.as_bytes()).is_err() {
            self.file = None;
            return;
        }
        self.written += text.len() as u64;
    }

    /// `cartload.log` becomes `cartload.log.1`, pushing the older ones along; the oldest
    /// beyond `kept` is dropped. Then a fresh file is opened.
    fn rotate(&mut self) {
        self.file = None;
        for index in (1..=self.kept).rev() {
            let from = if index == 1 {
                self.path.clone()
            } else {
                rotated_path(&self.path, index - 1)
            };
            let to = rotated_path(&self.path, index);
            if from.exists() {
                let _ = std::fs::rename(&from, &to);
            }
        }
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok();
        self.written = 0;
    }
}

/// `cartload.log.N`.
pub fn rotated_path(path: &Path, index: u32) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push('.');
    name.push_str(&index.to_string());
    path.with_file_name(name)
}

/// The `tracing` layer that writes to the log file.
pub struct FileLayer {
    buffer: Arc<LogBuffer>,
    writer: Mutex<FileWriter>,
}

impl FileLayer {
    /// A layer writing to `writer`. Lines are stamped through `buffer`'s clock and sequence
    /// so the file matches the panel (the line is not kept in the buffer twice).
    pub fn new(buffer: Arc<LogBuffer>, writer: FileWriter) -> Self {
        Self {
            buffer,
            writer: Mutex::new(writer),
        }
    }
}

impl<S: Subscriber> Layer<S> for FileLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let line = LogLine {
            seq: self.buffer.next_seq.load(Ordering::Relaxed),
            time: crate::status::now_ms(),
            level: (*metadata.level()).into(),
            source: source_name(metadata.target()),
            message: visitor.finish(),
        };
        self.writer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .write(&format_file_line(&line));
    }
}

/// Logs a panic as an error line (so it reaches the panel and the file) before the default
/// hook prints it.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        tracing::error!(location, "panic: {payload}");
        default(info);
    }));
}

/// What reaches the UI: everything at `info` and above, plus `debug` from the app and the
/// cartload crates (each command sent to the console, USB details).
pub const UI_FILTER: &str = "info,cartload_app_lib=debug,cartload_console=debug,\
                             cartload_fel=debug,cartload_hmod=debug,cartload_library=debug,\
                             cartload_flow=debug";

/// Installs the process-wide tracing subscriber: readable output on stderr (level from
/// `RUST_LOG`, default `info`), the UI feed, and the log file in [`log_dir`] (same filter as
/// the UI; skipped, with a note on stderr, when the directory cannot be written). Returns the
/// ring buffer and the receiving end of the live feed; see [`pump`]. Only the first call
/// installs anything, later calls still return a working (but unfed) pair.
pub fn init(capacity: usize) -> (Arc<LogBuffer>, mpsc::UnboundedReceiver<LogLine>) {
    let buffer = Arc::new(LogBuffer::new(capacity));
    let (sender, receiver) = mpsc::unbounded_channel();
    let ui = UiLayer::new(Arc::clone(&buffer), sender).with_filter(EnvFilter::new(UI_FILTER));
    let file =
        log_path().and_then(
            |path| match FileWriter::open(&path, MAX_LOG_BYTES, KEPT_LOG_FILES) {
                Ok(writer) => Some(
                    FileLayer::new(Arc::clone(&buffer), writer)
                        .with_filter(EnvFilter::new(UI_FILTER)),
                ),
                Err(error) => {
                    eprintln!("log file {} not written: {error}", path.display());
                    None
                }
            },
        );
    let stderr = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));
    if tracing_subscriber::registry()
        .with(stderr)
        .with(ui)
        .with(file)
        .try_init()
        .is_err()
    {
        eprintln!("tracing subscriber already installed; the UI log feed is not connected");
    }
    install_panic_hook();
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
    fn timestamps_are_iso_utc() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            format_timestamp(1_761_126_063_120),
            "2025-10-22T09:41:03.120Z"
        );
    }

    #[test]
    fn file_lines_align_levels() {
        let line = LogLine {
            seq: 1,
            time: 1_761_126_063_120,
            level: LogLevel::Warn,
            source: "console".to_string(),
            message: "slow".to_string(),
        };
        assert_eq!(
            format_file_line(&line),
            "2025-10-22T09:41:03.120Z warn  console: slow\n"
        );
    }

    #[test]
    fn file_writer_rotates_by_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cartload.log");
        let mut writer = FileWriter::open(&path, 20, 2).expect("open");
        writer.write("0123456789\n"); // 11 bytes
        writer.write("0123456789\n"); // 22 bytes: past the limit after this write
        writer.write("third\n"); // rotates first
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "third\n");
        let first = std::fs::read_to_string(rotated_path(&path, 1)).expect("rotated");
        assert_eq!(first.lines().count(), 2);
        // A second and third rotation push the oldest file out.
        writer.write("x".repeat(25).as_str());
        writer.write("fourth\n");
        writer.write("x".repeat(25).as_str());
        writer.write("fifth\n");
        assert!(rotated_path(&path, 1).exists());
        assert!(rotated_path(&path, 2).exists());
        assert!(!rotated_path(&path, 3).exists());
        assert_eq!(
            rotated_path(&path, 2).file_name().expect("name"),
            "cartload.log.2"
        );
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
