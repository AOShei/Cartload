//! What to tell the user when a flow fails: the likely cause and the next action, in plain
//! words, composed from the structured errors of `cartload-flow`. The raw error chain stays
//! available for a disclosure.

use std::time::Duration;

use cartload_flow::Error as FlowError;
use serde::{Deserialize, Serialize};

use crate::flow::FlowKind;

/// Broad class of a failure, for the icon and for tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// Something waited for did not happen in time.
    Timeout,
    /// The console is not in the state a step expects.
    WrongState,
    /// An input is missing or unusable.
    Precondition,
    /// The console reported a failure.
    Device,
    /// The FEL (USB boot) side failed.
    Usb,
    /// The network or SSH side failed.
    Connection,
    /// The kernel package is unusable.
    Package,
    /// A local file could not be read or written.
    File,
    /// Something in the app itself.
    Internal,
}

/// A failed flow, explained.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowFailure {
    /// Zero-based index of the step that failed, when a step did.
    pub step_index: Option<usize>,
    /// Name of that step.
    pub step_name: Option<String>,
    /// Broad class.
    pub kind: FailureKind,
    /// One line: what went wrong.
    pub title: String,
    /// The likely cause.
    pub cause: String,
    /// What to do next.
    pub next: String,
    /// The raw error and its causes, one per line.
    pub detail: String,
    /// The user can pick a stock kernel dump and retry (uninstall without a backup).
    pub wants_stock_kernel: bool,
    /// The chosen stock kernel is not a known image; the user may flash it anyway.
    pub wants_force: bool,
}

impl FlowFailure {
    /// Explains a flow error.
    pub fn from_flow_error(kind: &FlowKind, error: &FlowError) -> Self {
        let (step_index, step_name) = match error {
            FlowError::Step { index, name, .. } => (Some(*index), Some(name.clone())),
            _ => (None, None),
        };
        let mut failure = explain(kind, step_name.as_deref(), error.root());
        failure.step_index = step_index;
        failure.step_name = step_name;
        failure.detail = chain(error);
        failure
    }

    /// A failure of the app itself.
    pub fn internal(message: &str) -> Self {
        Self {
            step_index: None,
            step_name: None,
            kind: FailureKind::Internal,
            title: "Cartload ran into a problem.".to_string(),
            cause: message.to_string(),
            next: "Start again; if it repeats, copy the log and report it.".to_string(),
            detail: message.to_string(),
            wants_stock_kernel: false,
            wants_force: false,
        }
    }

    fn new(
        kind: FailureKind,
        title: impl Into<String>,
        cause: impl Into<String>,
        next: impl Into<String>,
    ) -> Self {
        Self {
            step_index: None,
            step_name: None,
            kind,
            title: title.into(),
            cause: cause.into(),
            next: next.into(),
            detail: String::new(),
            wants_stock_kernel: false,
            wants_force: false,
        }
    }
}

/// The error and every cause beneath it, one per line.
fn chain(error: &FlowError) -> String {
    let mut lines = vec![error.to_string()];
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        lines.push(cause.to_string());
        source = cause.source();
    }
    lines.join("\ncaused by: ")
}

/// Capitalises the first letter and ends with a full stop.
fn sentence(text: &str) -> String {
    let text = text.trim();
    let mut out = String::with_capacity(text.len() + 1);
    let mut chars = text.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
    if !out.is_empty() && !out.ends_with(['.', '!', '?']) {
        out.push('.');
    }
    out
}

/// The flow crate's hints talk about rerunning a command; the app has a button.
fn for_the_app(hint: &str) -> String {
    hint.replace("rerun the command to resume", "start again")
        .replace("rerun the command", "start again")
        .replace("rerun the same command", "start again")
}

fn explain(kind: &FlowKind, step: Option<&str>, root: &FlowError) -> FlowFailure {
    match root {
        FlowError::Timeout {
            what,
            timeout: limit,
            hint,
        } => timeout(what, *limit, hint),
        FlowError::WrongState(text) => wrong_state(text),
        FlowError::Precondition(text) => precondition(kind, text),
        FlowError::Device(text) => device(step, text),
        FlowError::Fel(error) => fel(error),
        FlowError::Console(error) => console(error),
        FlowError::Hmod(error) => FlowFailure::new(
            FailureKind::Package,
            "The kernel package is unusable.",
            sentence(&error.to_string()),
            "Download the package again in step 2, then start again.",
        ),
        FlowError::Io(error) => FlowFailure::new(
            FailureKind::File,
            "A file could not be read or written.",
            sentence(&error.to_string()),
            "Check the path and the free space, then start again.",
        ),
        other => FlowFailure::internal(&other.to_string()),
    }
}

fn timeout(what: &str, timeout: Duration, hint: &str) -> FlowFailure {
    let secs = timeout.as_secs();
    let (title, cause) = if what.contains("FEL mode") {
        (
            format!("The console did not show up in FEL mode within {secs} s."),
            "It is not in FEL mode, or the USB link is not working.".to_string(),
        )
    } else if what.contains("reappearing") {
        (
            format!("The console did not come up on the USB network within {secs} s."),
            if hint.contains("did not come back after rebooting") {
                "It did not come back after rebooting.".to_string()
            } else {
                "The boot from memory may not have taken, or the host has no link-local address \
                 on the console's network interface (hakchi0)."
                    .to_string()
            },
        )
    } else if what.contains("leaving") {
        (
            format!("The console did not reboot within {secs} s."),
            "It is still on the network, so the reboot command probably did not take.".to_string(),
        )
    } else if what.contains("installer") {
        (
            format!("The installer did not finish within {secs} s."),
            "The console may still be installing, or it got stuck.".to_string(),
        )
    } else {
        (
            format!(
                "{} within {secs} s.",
                sentence(&format!("{what} did not happen")).trim_end_matches('.')
            ),
            "The console may be stuck.".to_string(),
        )
    };
    FlowFailure::new(
        FailureKind::Timeout,
        title,
        cause,
        sentence(&for_the_app(hint)),
    )
}

fn wrong_state(text: &str) -> FlowFailure {
    if text.contains("not connected to the console") {
        return FlowFailure::new(
            FailureKind::WrongState,
            "The connection to the console was lost.",
            "A step needed the SSH session, but it was gone.",
            "Wait for the console to come back, then start again: finished steps are skipped.",
        );
    }
    if text.contains("/proc/atags") {
        return FlowFailure::new(
            FailureKind::WrongState,
            "The running kernel cannot boot another one.",
            sentence(text),
            "Power the console off and use FEL mode instead: hold Reset while powering on, then \
             start again.",
        );
    }
    if text.contains("expected the minimal memboot shell") {
        return FlowFailure::new(
            FailureKind::WrongState,
            "The console is not in the recovery shell.",
            sentence(text),
            "Start again: the boot from memory is repeated.",
        );
    }
    FlowFailure::new(
        FailureKind::WrongState,
        "The console is not in the state this step expects.",
        sentence(text),
        "Check the console's screen, then start again: finished steps are skipped.",
    )
}

fn precondition(kind: &FlowKind, text: &str) -> FlowFailure {
    if text.contains("hakchi.hmod") {
        return FlowFailure::new(
            FailureKind::Precondition,
            "The kernel package is needed.",
            "The console is not running hakchi, so it has to be booted through FEL, which needs \
             the package.",
            "Download the package in step 2, then start again.",
        );
    }
    if text.contains("no stock kernel backup") || text.contains("--stock-kernel") {
        let mut failure = FlowFailure::new(
            FailureKind::Precondition,
            "The console has no backup of its original kernel.",
            "The installer keeps one, but this console has none.",
            "Choose a kernel dump saved earlier from a console of the same model, then start \
             again.",
        );
        failure.wants_stock_kernel = matches!(kind, FlowKind::Uninstall { .. });
        return failure;
    }
    if text.contains("--force-stock-kernel") {
        let mut failure = FlowFailure::new(
            FailureKind::Precondition,
            "The chosen kernel file is not a known stock kernel.",
            sentence(text.split("; pass").next().unwrap_or(text)),
            "Pick the dump made from this console model, or flash this file anyway if you are \
             sure it is one.",
        );
        failure.wants_force = matches!(kind, FlowKind::Uninstall { .. });
        return failure;
    }
    if text.contains("Project Lunar") {
        return FlowFailure::new(
            FailureKind::Precondition,
            "Project Lunar is installed on this Mega Drive Mini.",
            "hakchi cannot be installed over it.",
            "Remove Project Lunar with its own tool first, then start again.",
        );
    }
    if text.contains("Mega Drive Mini partition layout") {
        return FlowFailure::new(
            FailureKind::Precondition,
            "Saving the stock kernel is not supported on the Mega Drive Mini.",
            sentence(text),
            "Nothing to do here: hakchi2 CE refuses it as well.",
        );
    }
    FlowFailure::new(
        FailureKind::Precondition,
        "Something the flow needs is missing.",
        sentence(text),
        "Fix that, then start again.",
    )
}

fn device(step: Option<&str>, text: &str) -> FlowFailure {
    if text.contains("the installer reported") {
        return FlowFailure::new(
            FailureKind::Device,
            "The installer stopped with an error.",
            sentence(text),
            "Switch the console off, put it in FEL mode (hold Reset while powering on) and \
             install again.",
        );
    }
    if text.contains("came back in the memboot shell") {
        return FlowFailure::new(
            FailureKind::Device,
            "The install did not complete.",
            sentence(text),
            "Start the install again: finished steps are skipped and the installer runs once \
             more.",
        );
    }
    if text.contains("versions differ") {
        return FlowFailure::new(
            FailureKind::Device,
            "The console runs a different version than the package.",
            sentence(text),
            "Start the install again; if it repeats, download the package again in step 2.",
        );
    }
    if text.contains("too old") || text.contains("no /var/version") {
        return FlowFailure::new(
            FailureKind::Device,
            "The installed system is too old to use.",
            sentence(text),
            "Install again.",
        );
    }
    if text.contains("MD5") && step == Some("upload") {
        return FlowFailure::new(
            FailureKind::Device,
            "The package did not arrive intact on the console.",
            sentence(text),
            "Start again: the upload is redone and checked.",
        );
    }
    if text.contains("no stock kernel backup on the console") {
        return FlowFailure::new(
            FailureKind::Device,
            "The console has no backup of its original kernel.",
            sentence(text),
            "On a console that never had hakchi, install it first: the installer saves the \
             backup.",
        );
    }
    if text.contains("sntool check") || text.contains("flashing the stock kernel failed") {
        return FlowFailure::new(
            FailureKind::Device,
            "The console refused the kernel image.",
            sentence(text),
            "Do not switch the console off. Use a dump made from this console model, then \
             start again.",
        );
    }
    FlowFailure::new(
        FailureKind::Device,
        "The console reported a failure.",
        sentence(text),
        "Check the console's screen and the log below, then start again: finished steps are \
         skipped.",
    )
}

fn fel(error: &cartload_fel::Error) -> FlowFailure {
    match error {
        cartload_fel::Error::DeviceNotFound => FlowFailure::new(
            FailureKind::Usb,
            "No console in FEL mode was found.",
            "It left FEL mode, or the USB link dropped.",
            "Hold Reset while powering the console on, keep it held for two seconds, then start \
             again.",
        ),
        cartload_fel::Error::PermissionDenied(_) => FlowFailure::new(
            FailureKind::Usb,
            "Cartload may not open the console over USB.",
            "The operating system refused access to the FEL device.",
            "On Linux, install the udev rule from docs/hardware-tests.md and reconnect the \
             console. On Windows, bind the WinUSB driver to it with Zadig.",
        ),
        other => FlowFailure::new(
            FailureKind::Usb,
            "The USB boot (FEL) transfer failed.",
            sentence(&other.to_string()),
            "Switch the console off, put it back in FEL mode (hold Reset while powering on) and \
             start again.",
        ),
    }
}

fn console(error: &cartload_console::Error) -> FlowFailure {
    match error {
        cartload_console::Error::AuthFailed { .. } => FlowFailure::new(
            FailureKind::Connection,
            "Something answered on the network, but it is not a console with a custom system.",
            sentence(&error.to_string()),
            "Make sure the console is the only device on the USB network, then start again.",
        ),
        cartload_console::Error::CommandTimeout { .. } => FlowFailure::new(
            FailureKind::Connection,
            "The console stopped answering.",
            sentence(&error.to_string()),
            "Check the console's screen; if it is frozen, switch it off and on, then start \
             again: finished steps are skipped.",
        ),
        other => FlowFailure::new(
            FailureKind::Connection,
            "The connection to the console failed.",
            sentence(&other.to_string()),
            "Wait a few seconds for the console to come back, then start again: finished steps \
             are skipped.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(index: usize, name: &str, source: FlowError) -> FlowError {
        FlowError::Step {
            index,
            name: name.into(),
            source: Box::new(source),
        }
    }

    #[test]
    fn fel_timeout_uses_the_flow_crates_hint() {
        let error = step(
            0,
            "memboot",
            FlowError::Timeout {
                what: "a console in FEL mode (USB 1f3a:efe8) appearing".into(),
                timeout: Duration::from_secs(120),
                hint: "power the console off, hold Reset, switch Power on, keep Reset held for \
                       two seconds and release it; check the USB cable and, on Linux, the udev \
                       rule for 1f3a:efe8"
                    .into(),
            },
        );
        let failure = FlowFailure::from_flow_error(&FlowKind::Install, &error);
        assert_eq!(failure.kind, FailureKind::Timeout);
        assert_eq!(failure.step_index, Some(0));
        assert_eq!(failure.step_name.as_deref(), Some("memboot"));
        assert_eq!(
            failure.title,
            "The console did not show up in FEL mode within 120 s."
        );
        assert_eq!(
            failure.next,
            "Power the console off, hold Reset, switch Power on, keep Reset held for two \
             seconds and release it; check the USB cable and, on Linux, the udev rule for \
             1f3a:efe8."
        );
        assert!(failure.detail.starts_with("step 1 (memboot) failed:"));
        assert!(!failure.wants_stock_kernel);
    }

    #[test]
    fn reboot_wait_and_installer_timeouts() {
        let error = step(
            11,
            "wait-console",
            FlowError::Timeout {
                what: "the console reappearing on the USB network".into(),
                timeout: Duration::from_secs(180),
                hint: "it did not come back after rebooting; check the console's screen and \
                       power, then rerun the command to resume"
                    .into(),
            },
        );
        let failure = FlowFailure::from_flow_error(&FlowKind::Install, &error);
        assert_eq!(
            failure.title,
            "The console did not come up on the USB network within 180 s."
        );
        assert_eq!(failure.cause, "It did not come back after rebooting.");
        assert_eq!(
            failure.next,
            "It did not come back after rebooting; check the console's screen and power, then \
             start again."
        );

        let error = step(
            10,
            "install",
            FlowError::Timeout {
                what: "the installer (`boot`)".into(),
                timeout: Duration::from_secs(900),
                hint: "the console may still be installing".into(),
            },
        );
        let failure = FlowFailure::from_flow_error(&FlowKind::Install, &error);
        assert_eq!(failure.title, "The installer did not finish within 900 s.");

        let error = FlowError::Timeout {
            what: "the slow step".into(),
            timeout: Duration::from_secs(5),
            hint: "the console may be stuck".into(),
        };
        let failure = FlowFailure::from_flow_error(&FlowKind::Memboot, &error);
        assert_eq!(failure.title, "The slow step did not happen within 5 s.");
        assert_eq!(failure.step_index, None);
    }

    #[test]
    fn preconditions_offer_the_right_follow_up() {
        let uninstall = FlowKind::Uninstall {
            stock_kernel: None,
            force_stock_kernel: false,
        };
        let error = step(
            3,
            "read-stock-kernel",
            FlowError::Precondition(
                "the console has no stock kernel backup; pass --stock-kernel <kernel.img> from \
                 an earlier dump-kernel run"
                    .into(),
            ),
        );
        let failure = FlowFailure::from_flow_error(&uninstall, &error);
        assert_eq!(failure.kind, FailureKind::Precondition);
        assert!(failure.wants_stock_kernel);
        assert!(!failure.next.contains("--stock-kernel"));

        let error = FlowError::Precondition(
            "/tmp/k.img is 4096 bytes, not a known stock kernel image (md5 abc); pass \
             --force-stock-kernel to flash it anyway"
                .into(),
        );
        let failure = FlowFailure::from_flow_error(&uninstall, &error);
        assert!(failure.wants_force);
        assert_eq!(
            failure.cause,
            "/tmp/k.img is 4096 bytes, not a known stock kernel image (md5 abc)."
        );

        let error = FlowError::Precondition(
            "the console runs a hakchi kernel; membooting the package from it needs hakchi.hmod: \
             pass --hmod <path>"
                .into(),
        );
        let failure = FlowFailure::from_flow_error(&uninstall, &error);
        assert_eq!(failure.title, "The kernel package is needed.");
        assert!(failure.next.contains("step 2"));

        let error = FlowError::Precondition("Project Lunar is installed on this Mega Drive Mini; hakchi cannot be installed over it (uninstall Project Lunar first)".into());
        let failure = FlowFailure::from_flow_error(&FlowKind::Install, &error);
        assert!(failure.title.starts_with("Project Lunar"));
    }

    #[test]
    fn device_wrong_state_usb_connection_package_and_file() {
        let failure = FlowFailure::from_flow_error(
            &FlowKind::Install,
            &step(
                10,
                "install",
                FlowError::Device("the installer reported: flash md5 mismatch!".into()),
            ),
        );
        assert_eq!(failure.title, "The installer stopped with an error.");
        assert_eq!(failure.cause, "The installer reported: flash md5 mismatch!");
        assert!(failure.next.contains("FEL mode"));

        let failure = FlowFailure::from_flow_error(
            &FlowKind::Install,
            &step(
                9,
                "upload",
                FlowError::Device(
                    "/hakchi/transfer/hakchi.hmod has MD5 x after the upload, expected y".into(),
                ),
            ),
        );
        assert_eq!(
            failure.title,
            "The package did not arrive intact on the console."
        );

        let failure = FlowFailure::from_flow_error(
            &FlowKind::Install,
            &FlowError::WrongState("not connected to the console".into()),
        );
        assert_eq!(failure.kind, FailureKind::WrongState);
        assert_eq!(failure.title, "The connection to the console was lost.");

        let failure = FlowFailure::from_flow_error(
            &FlowKind::Install,
            &FlowError::Fel(cartload_fel::Error::DeviceNotFound),
        );
        assert_eq!(failure.kind, FailureKind::Usb);
        assert_eq!(failure.title, "No console in FEL mode was found.");

        let failure = FlowFailure::from_flow_error(
            &FlowKind::Install,
            &FlowError::Console(cartload_console::Error::CommandTimeout {
                command: "boot".into(),
                timeout: Duration::from_secs(1),
            }),
        );
        assert_eq!(failure.kind, FailureKind::Connection);
        assert_eq!(failure.title, "The console stopped answering.");

        let failure = FlowFailure::from_flow_error(
            &FlowKind::Install,
            &FlowError::Io(std::io::Error::other("disk full")),
        );
        assert_eq!(failure.kind, FailureKind::File);
        assert_eq!(failure.cause, "Disk full.");
        assert_eq!(failure.detail, "I/O error: disk full\ncaused by: disk full");
    }

    #[test]
    fn sentences_are_tidied() {
        assert_eq!(sentence("check the cable"), "Check the cable.");
        assert_eq!(sentence("Done!"), "Done!");
        assert_eq!(sentence(""), "");
        assert_eq!(
            for_the_app("then rerun the command to resume"),
            "then start again"
        );
    }
}
