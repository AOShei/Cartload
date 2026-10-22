//! Linux system setup: the udev rule that lets an ordinary user open the console over USB
//! and names its network gadget `hakchi0`, and the NetworkManager profile that gives that
//! interface a link-local address. Both need root, obtained through `pkexec`, so the desktop
//! shows its usual authentication prompt. Other platforms report "not applicable".

use std::path::Path;

use serde::Serialize;
use tracing::{info, warn};

use crate::error::CommandError;

/// Where the rule goes. `70-` sorts before the desktop's `73-seat-late` rule so `uaccess`
/// tags apply, and the name says who put it there.
pub const RULE_PATH: &str = "/etc/udev/rules.d/70-cartload.rules";
/// Where the NetworkManager profile goes.
pub const PROFILE_PATH: &str = "/etc/NetworkManager/system-connections/cartload-usb.nmconnection";

/// The udev rule: USB access to the console in FEL mode (and the legacy clovershell, same
/// id) for the logged-in user, and the fixed interface name for the hakchi network gadget.
pub const RULE: &str = include_str!("../linux/70-cartload.rules");

/// The NetworkManager profile: a link-local address on `hakchi0` (the console sits at
/// 169.254.13.37 and runs no DHCP server), trusted so mDNS answers are not dropped.
pub const PROFILE: &str = include_str!("../linux/cartload-usb.nmconnection");

/// What is in place on this machine.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SetupStatus {
    /// `linux`, `macos`, `windows`.
    pub platform: String,
    /// Whether this setup applies (Linux only).
    pub applies: bool,
    /// A udev rule granting USB access to the console is present (ours or hakchi2 CE's).
    pub usb_rule: bool,
    /// A rule naming the network gadget `hakchi0` is present.
    pub name_rule: bool,
    /// NetworkManager is available (`nmcli` found).
    pub network_manager: bool,
    /// A NetworkManager profile gives `hakchi0` a link-local address.
    pub network_profile: bool,
    /// `pkexec` is available, so the app can ask for root itself.
    pub can_install: bool,
    /// Everything needed is in place.
    pub complete: bool,
}

/// Looks at the machine. Never fails: anything unreadable counts as absent.
pub fn status() -> SetupStatus {
    let platform = std::env::consts::OS.to_string();
    if platform != "linux" {
        return SetupStatus {
            platform,
            ..SetupStatus::default()
        };
    }
    // The admin directory and the package directory (the .deb puts the rule in the latter).
    let mut rules = read_rules_dir(Path::new("/etc/udev/rules.d"));
    rules.extend(read_rules_dir(Path::new("/usr/lib/udev/rules.d")));
    let (usb_rule, name_rule) = rules_present(&rules);
    let network_manager = which("nmcli");
    let network_profile = network_manager && profile_present(&nmcli_profiles());
    let can_install = which("pkexec");
    SetupStatus {
        platform,
        applies: true,
        usb_rule,
        name_rule,
        network_manager,
        network_profile,
        can_install,
        complete: usb_rule && name_rule && (network_profile || !network_manager),
    }
}

/// Installs what is missing, asking for root through `pkexec`, and reports the new status.
pub fn install() -> Result<SetupStatus, CommandError> {
    let before = status();
    if !before.applies {
        return Err(CommandError::new("This setup applies to Linux only."));
    }
    if before.complete {
        return Ok(before);
    }
    if !before.can_install {
        return Err(CommandError::new(
            "pkexec was not found, so Cartload cannot ask for administrator rights.",
        )
        .with_detail(format!(
            "Install the rule by hand: write it to {RULE_PATH}, then run `udevadm control --reload-rules` and `udevadm trigger`."
        )));
    }
    let dir = std::env::temp_dir().join(format!("cartload-setup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| {
        CommandError::new("Could not create a temporary directory.").with_detail(e.to_string())
    })?;
    let rule_src = dir.join("70-cartload.rules");
    let profile_src = dir.join("cartload-usb.nmconnection");
    std::fs::write(&rule_src, RULE).map_err(|e| {
        CommandError::new("Could not write the rule file.").with_detail(e.to_string())
    })?;
    std::fs::write(&profile_src, PROFILE).map_err(|e| {
        CommandError::new("Could not write the profile file.").with_detail(e.to_string())
    })?;
    let script = install_script(
        &rule_src.to_string_lossy(),
        &profile_src.to_string_lossy(),
        !(before.usb_rule && before.name_rule),
        before.network_manager && !before.network_profile,
    );
    info!(
        rule = !(before.usb_rule && before.name_rule),
        profile = before.network_manager && !before.network_profile,
        "asking for administrator rights to install the Linux setup"
    );
    let output = std::process::Command::new("pkexec")
        .arg("/bin/sh")
        .arg("-c")
        .arg(&script)
        .output()
        .map_err(|e| CommandError::new("Could not start pkexec.").with_detail(e.to_string()))?;
    let _ = std::fs::remove_dir_all(&dir);
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        warn!(code, %stderr, "the setup script failed");
        let message = match code {
            126 | 127 => "The administrator prompt was cancelled; nothing was installed.",
            _ => "The setup script failed; nothing may have been installed.",
        };
        return Err(
            CommandError::new(message).with_detail(if stderr.is_empty() {
                format!("exit code {code}")
            } else {
                stderr
            }),
        );
    }
    info!("Linux setup installed");
    Ok(status())
}

/// The shell run as root: copies what is needed into place and reloads the services.
pub fn install_script(rule_src: &str, profile_src: &str, rule: bool, profile: bool) -> String {
    let mut script = String::from("set -e\n");
    if rule {
        script.push_str(&format!(
            "install -m 0644 '{rule_src}' '{RULE_PATH}'\nudevadm control --reload-rules\nudevadm trigger\n"
        ));
    }
    if profile {
        script.push_str(&format!(
            "if [ -d /etc/NetworkManager/system-connections ]; then install -m 0600 -o root -g root '{profile_src}' '{PROFILE_PATH}'; nmcli connection reload || true; fi\n"
        ));
    }
    script
}

/// Whether a rule granting access to `1f3a:efe8` and one naming the gadget `hakchi0` are
/// among the given rule file contents (hakchi2 CE's `99-hakchi.rules` counts).
pub fn rules_present(contents: &[String]) -> (bool, bool) {
    let mut usb = false;
    let mut name = false;
    for text in contents {
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("1f3a")
                && compact.contains("efe8")
                && (compact.contains("MODE=\"0666\"") || compact.contains("TAG+=\"uaccess\""))
            {
                usb = true;
            }
            if compact.contains("04e8")
                && compact.contains("6863")
                && compact.contains("NAME=\"hakchi0\"")
            {
                name = true;
            }
        }
    }
    (usb, name)
}

/// Whether any `nmcli` profile line (`name:interface:method`) binds `hakchi0` to link-local.
pub fn profile_present(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        let mut parts = line.rsplitn(3, ':');
        let method = parts.next().unwrap_or("").trim();
        let interface = parts.next().unwrap_or("").trim();
        interface == "hakchi0" && method == "link-local"
    })
}

fn read_rules_dir(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rules"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect()
}

fn nmcli_profiles() -> Vec<String> {
    let Ok(output) = std::process::Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "connection.id,connection.interface-name,ipv4.method",
            "connection",
            "show",
        ])
        .output()
    else {
        return Vec::new();
    };
    // `nmcli -t -f <fields> connection show` prints one line per profile only for the
    // `NAME`-style fields; the setting-style fields need a per-profile query.
    let names = String::from_utf8_lossy(&output.stdout);
    let list = std::process::Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let mut lines = Vec::new();
    for name in list.lines().map(str::trim).filter(|n| !n.is_empty()) {
        if let Ok(o) = std::process::Command::new("nmcli")
            .args([
                "-t",
                "-f",
                "connection.interface-name,ipv4.method",
                "connection",
                "show",
                name,
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut interface = "";
            let mut method = "";
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("connection.interface-name:") {
                    interface = v.trim();
                } else if let Some(v) = line.strip_prefix("ipv4.method:") {
                    method = v.trim();
                }
            }
            lines.push(format!("{name}:{interface}:{method}"));
        }
    }
    let _ = names;
    lines
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ce_rule_file_counts() {
        let ce = "# comment\nSUBSYSTEM==\"usb\", ATTR{idVendor}==\"1f3a\", ATTR{idProduct}==\"efe8\", MODE=\"0666\", TAG+=\"uaccess\"\nSUBSYSTEM==\"net\", ACTION==\"add\", ATTRS{idVendor}==\"04e8\", ATTRS{idProduct}==\"6863\", ATTRS{manufacturer}==\"hakchi\", NAME=\"hakchi0\"\n".to_string();
        assert_eq!(rules_present(&[ce]), (true, true));
        assert_eq!(rules_present(&[RULE.to_string()]), (true, true));
        assert_eq!(
            rules_present(&["# 1f3a efe8 hakchi0 only in a comment".to_string()]),
            (false, false)
        );
        assert_eq!(rules_present(&[]), (false, false));
    }

    #[test]
    fn profile_lines() {
        assert!(profile_present(&[
            "hakchi USB:hakchi0:link-local".to_string()
        ]));
        assert!(!profile_present(&[
            "Wired connection 1:enp8s0:auto".to_string()
        ]));
        assert!(!profile_present(&["hakchi USB:hakchi0:auto".to_string()]));
        assert!(profile_present(&[
            "a:b:c".to_string(),
            "Cartload USB:hakchi0:link-local".to_string()
        ]));
    }

    #[test]
    fn status_never_fails() {
        let s = status();
        eprintln!("setup status on this machine: {s:?}");
        assert_eq!(s.applies, s.platform == "linux");
    }

    #[test]
    fn script_installs_only_what_is_missing() {
        let both = install_script("/tmp/r", "/tmp/p", true, true);
        assert!(both.contains("udevadm trigger"));
        assert!(both.contains("nmcli connection reload"));
        let rule_only = install_script("/tmp/r", "/tmp/p", true, false);
        assert!(!rule_only.contains("nmcli"));
        let none = install_script("/tmp/r", "/tmp/p", false, false);
        assert_eq!(none, "set -e\n");
    }
}
