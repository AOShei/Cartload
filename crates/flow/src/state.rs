//! The states a console can be in, as seen from the host.

use cartload_console::ConsoleInfo;
use serde::Serialize;

/// What the host found on the USB network link.
///
/// hakchi2 CE distinguishes the same states in `hakchi.cs`: nothing online (stock firmware,
/// FEL mode, powered off, or the legacy clovershell kernel), a minimal memboot
/// (`cf_memboot=y`: the kernel runs from RAM and nothing was booted from NAND), and an
/// installed kernel, which is usable only when its versions are recent enough.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConsoleState {
    /// No hakchi kernel answers on the USB network.
    Offline,
    /// A hakchi kernel answered; `info` is what the device script reported.
    Online(Box<ConsoleInfo>),
}

impl ConsoleState {
    /// The console information when online.
    pub fn info(&self) -> Option<&ConsoleInfo> {
        match self {
            ConsoleState::Offline => None,
            ConsoleState::Online(info) => Some(info.as_ref()),
        }
    }

    /// Whether a hakchi kernel answered.
    pub fn is_online(&self) -> bool {
        self.info().is_some()
    }

    /// The minimal memboot shell: kernel in RAM, nothing booted from NAND.
    pub fn is_memboot_shell(&self) -> bool {
        self.info().is_some_and(|info| info.minimal_memboot)
    }

    /// A hakchi kernel booted from NAND with its rootfs (any version).
    pub fn is_installed(&self) -> bool {
        self.info()
            .is_some_and(|info| !info.minimal_memboot && info.versions.is_some())
    }

    /// Whether the installed kernel is recent enough for the host tooling.
    pub fn can_interact(&self) -> bool {
        self.info().is_some_and(|info| info.can_interact)
    }

    /// Whether the console uses the Mega Drive Mini partition layout.
    pub fn md_partitioning(&self) -> bool {
        self.info().is_some_and(|info| info.md_partitioning)
    }

    /// One line for the user.
    pub fn describe(&self) -> String {
        let Some(info) = self.info() else {
            return "offline: no hakchi kernel answers on the USB network (stock firmware, FEL \
                    mode, powered off, or a legacy kernel)"
                .to_string();
        };
        let layout = if info.md_partitioning {
            ", Mega Drive partition layout"
        } else {
            ""
        };
        if info.minimal_memboot {
            return format!("minimal memboot shell (hwid {}{layout})", info.hwid);
        }
        match &info.versions {
            None => format!(
                "hakchi kernel without /var/version, severely outdated (hwid {}{layout})",
                info.hwid
            ),
            Some(versions) => {
                let mut text = format!(
                    "hakchi {} (boot {}, kernel {}) installed",
                    versions.script, versions.boot, versions.kernel
                );
                if info.can_interact {
                    text.push_str(&format!(" on {}", info.console));
                } else if versions.requires_reflash() {
                    text.push_str(", too old: the kernel must be reinstalled");
                } else {
                    text.push_str(", too old: the rootfs must be updated");
                }
                text.push_str(&format!(" (hwid {}{layout})", info.hwid));
                text
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cartload_console::{ConsoleType, Versions};

    use super::*;

    fn versions(script: &str, boot: &str, kernel: &str) -> Versions {
        Versions {
            boot: boot.into(),
            kernel: kernel.into(),
            script: script.into(),
        }
    }

    #[test]
    fn offline() {
        let state = ConsoleState::Offline;
        assert!(!state.is_online());
        assert!(!state.is_memboot_shell());
        assert!(!state.is_installed());
        assert!(state.describe().starts_with("offline"));
    }

    #[test]
    fn memboot_shell() {
        let state = ConsoleState::Online(Box::new(ConsoleInfo {
            hwid: "0461872a872802079d2818c76c118000".into(),
            minimal_memboot: true,
            ..ConsoleInfo::default()
        }));
        assert!(state.is_online());
        assert!(state.is_memboot_shell());
        assert!(!state.is_installed());
        assert_eq!(
            state.describe(),
            "minimal memboot shell (hwid 0461872a872802079d2818c76c118000)"
        );
    }

    #[test]
    fn installed_and_usable() {
        let state = ConsoleState::Online(Box::new(ConsoleInfo {
            hwid: "abcd".into(),
            versions: Some(versions("v1.0.4-126", "1.0.3", "3.4.113.29-madmonkey")),
            can_interact: true,
            console: ConsoleType::SnesEur,
            ..ConsoleInfo::default()
        }));
        assert!(state.is_installed());
        assert!(state.can_interact());
        assert_eq!(
            state.describe(),
            "hakchi v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey) installed on SNES \
             Classic Mini (EUR) (hwid abcd)"
        );
    }

    #[test]
    fn installed_but_outdated() {
        let state = ConsoleState::Online(Box::new(ConsoleInfo {
            hwid: "abcd".into(),
            versions: Some(versions("v1.0.4-100", "1.0.3", "3.4.113.29-madmonkey")),
            can_interact: false,
            md_partitioning: true,
            ..ConsoleInfo::default()
        }));
        assert!(state.is_installed());
        assert!(!state.can_interact());
        assert!(state.md_partitioning());
        assert!(state.describe().contains("rootfs must be updated"));
        assert!(state.describe().contains("Mega Drive partition layout"));

        let state = ConsoleState::Online(Box::new(ConsoleInfo {
            hwid: "abcd".into(),
            versions: None,
            ..ConsoleInfo::default()
        }));
        assert!(!state.is_installed());
        assert!(state.describe().contains("without /var/version"));
    }
}
