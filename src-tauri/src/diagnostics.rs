//! The diagnostics report that goes into a bug report, and where the log file is.
//!
//! The report is plain text: the build, the system, the Linux setup, the console as last
//! seen (including its identifiers: hardware id, serial, firmware), the last kernel flow and
//! sync, and the recent log lines at debug level. It never contains the TheGamesDB key.

use std::path::Path;

use serde::Serialize;

use crate::{
    error::CommandError,
    flow::FlowSnapshot,
    library::KeyStatus,
    logging::{self, format_file_line},
    state::AppState,
    status::now_ms,
    sync::SyncSnapshot,
};

/// Where the log file is.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LogFileInfo {
    /// The current log file (`None` when the system has no data folder).
    pub path: Option<String>,
    /// Whether it exists yet.
    pub exists: bool,
    /// Its size in bytes.
    pub size: u64,
}

/// The current log file's path, existence and size.
pub fn log_file_info() -> LogFileInfo {
    let path = logging::log_path();
    let meta = path.as_deref().and_then(|p| std::fs::metadata(p).ok());
    LogFileInfo {
        path: path.map(|p| p.display().to_string()),
        exists: meta.is_some(),
        size: meta.map(|m| m.len()).unwrap_or(0),
    }
}

/// Builds the report.
pub fn report(
    state: &AppState,
    flow: &FlowSnapshot,
    sync: &SyncSnapshot,
    key: Option<&KeyStatus>,
) -> String {
    let version = crate::commands::version();
    let mut out = String::new();
    out.push_str("Cartload diagnostics\n");
    out.push_str(&format!(
        "generated: {}\n",
        logging::format_timestamp(now_ms())
    ));
    out.push_str(&format!(
        "version: {} ({}{})\n",
        version.app,
        if version.debug { "debug" } else { "release" },
        if version.features.is_empty() {
            String::new()
        } else {
            format!(", features {}", version.features.join(" "))
        }
    ));
    out.push_str(&format!("crates: {}\n", version.crates.join(", ")));
    out.push_str(&format!("system: {}\n", system_description()));
    out.push_str(&format!(
        "log file: {}\n",
        logging::log_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(none)".to_string())
    ));
    out.push_str(&format!(
        "TheGamesDB key: {}\n",
        match key {
            Some(k) if k.environment => "from the environment",
            Some(k) if k.stored => "stored",
            Some(_) => "not set",
            None => "unknown",
        }
    ));

    out.push_str("\n== Linux setup ==\n");
    out.push_str(&pretty(&crate::setup::status()));

    out.push_str("\n== Console status ==\n");
    out.push_str(&pretty(&state.report()));

    out.push_str("\n== Last kernel flow ==\n");
    out.push_str(&pretty(flow));

    out.push_str("\n== Last sync ==\n");
    out.push_str(&pretty(sync));

    out.push_str("\n== Recent log ==\n");
    for line in state.log.recent() {
        out.push_str(&format_file_line(&line));
    }
    out
}

fn pretty<T: Serialize>(value: &T) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_else(|e| format!("({e})"));
    text.push('\n');
    text
}

/// `linux x86_64, EndeavourOS Linux` (the distribution from `/etc/os-release` when there is
/// one), `macos aarch64`, `windows x86_64`.
pub fn system_description() -> String {
    let mut text = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    if let Some(name) = os_release_name() {
        text.push_str(", ");
        text.push_str(&name);
    }
    text
}

fn os_release_name() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/os-release").ok()?;
    parse_os_release(&contents)
}

/// `PRETTY_NAME` from an `os-release` file.
pub fn parse_os_release(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
}

/// Opens `dir` in the desktop's file manager.
pub fn reveal(dir: &Path) -> Result<(), CommandError> {
    let (program, args): (&str, Vec<&str>) = match std::env::consts::OS {
        "macos" => ("open", vec![]),
        "windows" => ("explorer", vec![]),
        _ => ("xdg-open", vec![]),
    };
    std::process::Command::new(program)
        .args(args)
        .arg(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| {
            CommandError::new(format!(
                "Could not open the folder. It is at {}.",
                dir.display()
            ))
            .with_detail(format!("{program}: {e}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{LogBuffer, LogLevel};
    use std::sync::Arc;

    #[test]
    fn os_release_pretty_name() {
        assert_eq!(
            parse_os_release(
                "NAME=\"EndeavourOS\"\nPRETTY_NAME=\"EndeavourOS Linux\"\nID=endeavouros\n"
            ),
            Some("EndeavourOS Linux".to_string())
        );
        assert_eq!(parse_os_release("NAME=x\n"), None);
        assert_eq!(parse_os_release("PRETTY_NAME=\"\"\n"), None);
    }

    #[test]
    fn report_has_every_section_and_the_log() {
        let log = Arc::new(LogBuffer::new(8));
        log.push(LogLevel::Debug, "console", "exec command=uistop");
        let state = AppState::new(log);
        let text = report(
            &state,
            &FlowSnapshot::idle(),
            &SyncSnapshot::idle(),
            Some(&KeyStatus {
                stored: true,
                environment: false,
                remaining: None,
            }),
        );
        for section in [
            "Cartload diagnostics",
            "== Linux setup ==",
            "== Console status ==",
            "== Last kernel flow ==",
            "== Last sync ==",
            "== Recent log ==",
            "TheGamesDB key: stored",
            "console: exec command=uistop",
        ] {
            assert!(text.contains(section), "missing {section}");
        }
    }
}
