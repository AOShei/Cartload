//! The console's state as the app presents it: the states the watcher distinguishes, and the
//! one plain sentence (what the console is, what to do next) that goes with each of them.
//!
//! The text is composed here, in Rust, so that every state has one wording, tested without a
//! browser; the frontend lays it out and keeps the raw data for the other pages.

use std::time::{SystemTime, UNIX_EPOCH};

use cartload_console::{ConsoleInfo, ConsoleType, Firmware, Region};
use serde::{Deserialize, Serialize};

use crate::usb::UsbDevice;

/// Unix time in milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// What the watcher found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConsoleStatus {
    /// Neither a FEL device nor a hakchi kernel is reachable.
    Disconnected {
        /// Why, as far as the watcher can tell.
        hint: Hint,
    },
    /// A device with the Allwinner boot ROM ids is attached.
    Fel {
        /// The device.
        device: UsbDevice,
        /// Whether it really answers as the FEL boot ROM.
        check: FelCheck,
    },
    /// A hakchi kernel is coming up, or a console is being connected and identified.
    Booting {
        /// What is being waited for.
        phase: BootPhase,
        /// Address being connected to, when known.
        address: Option<String>,
        /// Unix time in milliseconds when this phase started.
        since: u64,
    },
    /// An SSH session is open and the console has been identified.
    Connected {
        /// SSH address.
        address: String,
        /// How the console is reached.
        link: Link,
        /// Unix time in milliseconds when the session was established.
        since: u64,
        /// Everything the connect sequence reported.
        info: Box<ConsoleInfo>,
    },
    /// A kernel flow owns the console; the watcher is paused until it ends.
    Working {
        /// The flow's name (`install`, `memboot`, `uninstall`, `dump-kernel`).
        flow: String,
        /// What the flow does (`Installing hakchi`).
        title: String,
        /// Label of the running step, once one has started.
        step: Option<String>,
        /// Zero-based index of the running step.
        index: Option<usize>,
        /// Number of steps.
        total: usize,
    },
}

/// Why nothing is connected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Hint {
    /// The watcher has not looked yet.
    Starting,
    /// Nothing console-like on USB, no SSH answer.
    Nothing,
    /// A link-local interface exists but nothing answers on it.
    LinkNoAnswer {
        /// The interfaces probed.
        interface: String,
    },
    /// Something answered on the network but the SSH session could not be set up.
    ConnectFailed {
        /// Address that answered.
        address: String,
        /// The error.
        reason: String,
        /// The server refused the hakchi credentials (so it is not a hakchi kernel).
        refused: bool,
    },
    /// Looking for the console failed on the host side.
    ProbeFailed {
        /// The error.
        reason: String,
    },
}

/// Result of asking a device with the FEL ids whether it is the boot ROM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FelCheck {
    /// Not asked yet.
    Unchecked,
    /// It answered the FEL version query.
    Fel {
        /// SoC id it reported.
        soc_id: u32,
        /// SoC name, when known.
        soc: String,
        /// Whether it is the Allwinner R16 of the Classic Mini consoles.
        r16: bool,
    },
    /// It did not answer as the boot ROM: probably stock firmware in normal boot.
    NotFel {
        /// The error.
        reason: String,
    },
    /// The device could not be opened (permissions, busy).
    Unavailable {
        /// The error.
        reason: String,
    },
}

/// What a booting console is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootPhase {
    /// The hakchi gadget is enumerated but SSH does not answer yet.
    Network,
    /// SSH answered; the session is being opened.
    Connecting,
    /// The session is open; the connect sequence is running.
    Identifying,
    /// The FEL device just disappeared; a membooted kernel may be coming up.
    LeavingFel,
}

/// How the console is reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Link {
    /// The USB network gadget.
    Usb {
        /// Host interface (`hakchi0`).
        interface: String,
        /// Host address on that interface.
        host_address: String,
    },
    /// Some other network path (Wi-Fi mod); no USB gadget was seen.
    Network,
}

/// How the status card is coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// Nothing to act on.
    Neutral,
    /// Good to go.
    Ready,
    /// Something is in progress.
    Busy,
    /// Needs attention.
    Warning,
    /// Something is wrong on the host side.
    Error,
}

/// One detected fact for the details list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detail {
    /// Short label.
    pub label: String,
    /// The value, already formatted.
    pub value: String,
}

impl Detail {
    fn new(label: &str, value: impl Into<String>) -> Self {
        Self {
            label: label.to_string(),
            value: value.into(),
        }
    }
}

/// The status with its explanation, as sent to the frontend.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusReport {
    /// The state and its data.
    pub status: ConsoleStatus,
    /// Colouring.
    pub tone: Tone,
    /// One line: what the console is.
    pub title: String,
    /// One or two sentences: what to do next.
    pub next: String,
    /// The detected details, in display order.
    pub details: Vec<Detail>,
    /// Whether Cartload supports the connected console. The Mega Drive Mini is recognised
    /// but not supported: it uses another emulator and menu format, and Project Lunar
    /// serves it; the app does nothing to it.
    pub supported: bool,
    /// Unix time in milliseconds when this report was produced.
    pub changed_at: u64,
}

impl StatusReport {
    /// Whether two reports say the same thing (ignoring when they were produced).
    pub fn same_as(&self, other: &Self) -> bool {
        self.status == other.status
            && self.tone == other.tone
            && self.title == other.title
            && self.next == other.next
            && self.details == other.details
    }
}

/// Marketing name of a console model, without the region.
pub fn model_name(console: ConsoleType) -> &'static str {
    match console {
        ConsoleType::Nes => "NES Classic Mini",
        ConsoleType::Famicom => "Famicom Mini",
        ConsoleType::SnesEur => "SNES Classic Mini",
        ConsoleType::SnesUsa => "SNES Classic",
        ConsoleType::SuperFamicom => "Super Famicom Mini",
        ConsoleType::ShonenJump => "Famicom Mini (Shonen Jump edition)",
        ConsoleType::MdJpn | ConsoleType::MdEur | ConsoleType::MdAsia => "Mega Drive Mini",
        ConsoleType::MdUsa => "Genesis Mini",
        ConsoleType::Unknown => "Unknown console",
    }
}

/// Short region code (`EUR`), or `None` when unknown.
pub fn region_code(region: Region) -> Option<&'static str> {
    match region {
        Region::Usa => Some("USA"),
        Region::Eur => Some("EUR"),
        Region::Jpn => Some("JPN"),
        Region::Asia => Some("ASIA"),
        Region::Unknown => None,
    }
}

/// A byte count for people: `310 MB`, `1.5 GB`, `96 KB` (binary units, decimal labels, as
/// hakchi2 CE shows them).
pub fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KIB * KIB * KIB {
        format!("{:.1} GB", b / (KIB * KIB * KIB))
    } else if b >= KIB * KIB {
        format!("{:.0} MB", b / (KIB * KIB))
    } else if b >= KIB {
        format!("{:.0} KB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Seconds elapsed since a unix-millisecond timestamp.
fn seconds_since(since: u64, now: u64) -> u64 {
    now.saturating_sub(since) / 1000
}

/// How long the network phase may take before the text suggests a host-side cause.
pub const NETWORK_WAIT_WARNING: u64 = 30;

impl ConsoleStatus {
    /// The initial status, before the watcher has looked.
    pub fn starting() -> Self {
        Self::Disconnected {
            hint: Hint::Starting,
        }
    }

    /// Whether a session is open.
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// The console information, when connected.
    pub fn info(&self) -> Option<&ConsoleInfo> {
        match self {
            Self::Connected { info, .. } => Some(info),
            _ => None,
        }
    }

    /// Composes the report for this status at time `now` (unix milliseconds).
    pub fn describe(&self, now: u64) -> StatusReport {
        let (tone, title, next, details) = match self {
            Self::Disconnected { hint } => describe_disconnected(hint),
            Self::Fel { device, check } => describe_fel(device, check),
            Self::Booting {
                phase,
                address,
                since,
            } => describe_booting(*phase, address.as_deref(), seconds_since(*since, now)),
            Self::Connected {
                address,
                link,
                info,
                ..
            } => describe_connected(address, link, info),
            Self::Working {
                flow,
                title,
                step,
                index,
                total,
            } => describe_working(flow, title, step.as_deref(), *index, *total),
        };
        let unsupported = self.info().is_some_and(|info| {
            info.console.family() == cartload_console::ConsoleFamily::MegaDrive
        });
        let (tone, title, next) = if unsupported {
            (
                Tone::Warning,
                format!(
                    "{} connected: not supported by Cartload.",
                    model_name(self.info().map_or(ConsoleType::Unknown, |i| i.console))
                ),
                "Cartload does not support the Mega Drive Mini. Nothing will be installed on or \
                 synced to this console."
                    .into(),
            )
        } else {
            (tone, title, next)
        };
        StatusReport {
            status: self.clone(),
            tone,
            title,
            next,
            details,
            supported: !unsupported,
            changed_at: now,
        }
    }
}

type Description = (Tone, String, String, Vec<Detail>);

fn describe_disconnected(hint: &Hint) -> Description {
    const NOTHING: &str = "Nothing found. Connect the console with USB and turn it on.";
    match hint {
        Hint::Starting => (
            Tone::Busy,
            "Looking for the console…".into(),
            "Connect it with USB and turn it on.".into(),
            vec![],
        ),
        Hint::Nothing => (
            Tone::Neutral,
            NOTHING.into(),
            "A console with hakchi installed shows up a few seconds after power-on. To install \
             hakchi, hold Reset while turning the console on so it starts in FEL mode."
                .into(),
            vec![],
        ),
        Hint::LinkNoAnswer { interface } => (
            Tone::Neutral,
            NOTHING.into(),
            format!(
                "A link-local network interface ({interface}) exists but no console answers on \
                 it. Turn the console on, or give it a few seconds."
            ),
            vec![],
        ),
        Hint::ConnectFailed {
            address,
            reason,
            refused: true,
        } => (
            Tone::Warning,
            format!("Something answers at {address}, but it is not a hakchi kernel."),
            "It refused the hakchi login. If this is the console, install the kernel from the \
             Kernel page."
                .into(),
            vec![
                Detail::new("Address", address.clone()),
                Detail::new("Error", reason.clone()),
            ],
        ),
        Hint::ConnectFailed {
            address,
            reason,
            refused: false,
        } => (
            Tone::Warning,
            format!("Console found at {address}, but the connection failed."),
            "Trying again in a few seconds. If it keeps failing, switch the console off and on."
                .into(),
            vec![
                Detail::new("Address", address.clone()),
                Detail::new("Error", reason.clone()),
            ],
        ),
        Hint::ProbeFailed { reason } => (
            Tone::Error,
            "Cartload could not look for the console.".into(),
            "Check that this user may list USB devices and network interfaces, then try again."
                .into(),
            vec![Detail::new("Error", reason.clone())],
        ),
    }
}

fn describe_fel(device: &UsbDevice, check: &FelCheck) -> Description {
    let mut details = vec![
        Detail::new(
            "USB",
            format!("bus {}, device {}", device.bus, device.address),
        ),
        Detail::new("USB id", device.id_text()),
    ];
    // The USB manufacturer string is not shown: the app names consoles by model only.
    if let Some(product) = &device.product {
        details.push(Detail::new("Product", product.clone()));
        if let Some(model) = cartload_console::ConsoleType::from_usb_product(product) {
            details.push(Detail::new("Model", model.to_string()));
        }
    }
    match check {
        FelCheck::Unchecked => (
            Tone::Busy,
            "Console attached over USB: checking for FEL mode…".into(),
            "This takes a moment.".into(),
            details,
        ),
        FelCheck::Fel { soc, r16: true, .. } => {
            details.push(Detail::new("Chip", soc.clone()));
            (
                Tone::Ready,
                "Console in FEL mode: ready to install the kernel.".into(),
                "Open the Kernel page to install hakchi, or to boot it from memory without \
                 installing anything."
                    .into(),
                details,
            )
        }
        FelCheck::Fel {
            soc,
            soc_id,
            r16: false,
        } => {
            details.push(Detail::new("Chip", format!("{soc} ({soc_id:#010x})")));
            (
                Tone::Warning,
                "A device in FEL mode is attached, but it is not a Classic Mini console.".into(),
                format!(
                    "Its chip is {soc}; the consoles use the Allwinner R16. Check what is \
                     plugged in."
                ),
                details,
            )
        }
        FelCheck::NotFel { reason } => {
            details.push(Detail::new("Error", reason.clone()));
            let looks_stock = device.looks_like_stock_firmware();
            (
                Tone::Warning,
                "Console attached over USB, but not in FEL mode.".into(),
                format!(
                    "{}Switch it off, hold Reset, switch it on while still holding Reset for \
                     two seconds, then release Reset.",
                    if looks_stock {
                        "It is on stock firmware in normal boot. "
                    } else {
                        "It is probably on stock firmware. "
                    }
                ),
                details,
            )
        }
        FelCheck::Unavailable { reason } => {
            details.push(Detail::new("Error", reason.clone()));
            (
                Tone::Error,
                "Console attached in FEL mode, but Cartload may not open it.".into(),
                "On Linux, open Settings and press Install under \"Linux setup\", then reconnect \
                 the console. On Windows, bind the WinUSB driver to it with Zadig."
                    .into(),
                details,
            )
        }
    }
}

fn describe_booting(phase: BootPhase, address: Option<&str>, elapsed: u64) -> Description {
    let waiting = vec![Detail::new("Waiting for", format!("{elapsed} s"))];
    let address = address.unwrap_or("the console");
    match phase {
        BootPhase::Network => (
            Tone::Busy,
            "hakchi kernel found on USB: waiting for its network link…".into(),
            if elapsed < NETWORK_WAIT_WARNING {
                "This usually takes a few seconds.".into()
            } else {
                format!(
                    "Still waiting after {elapsed} s. The host may have no link-local address on \
                     the console's network interface (hakchi0); see docs/hardware-tests.md for \
                     the NetworkManager setup."
                )
            },
            waiting,
        ),
        BootPhase::Connecting => (
            Tone::Busy,
            format!("Console found at {address}: connecting…"),
            "Opening the SSH session.".into(),
            waiting,
        ),
        BootPhase::Identifying => (
            Tone::Busy,
            format!("Console found at {address}: identifying…"),
            "Reading its model, versions and storage.".into(),
            waiting,
        ),
        BootPhase::LeavingFel => (
            Tone::Busy,
            "Console left FEL mode: waiting for the hakchi kernel to come up…".into(),
            "After a memboot this takes about ten seconds. If the console was switched off \
             instead, this clears shortly."
                .into(),
            waiting,
        ),
    }
}

/// `Installing hakchi: step 5 of 13, grow the data partition…`, or just the title with an
/// ellipsis before the first step starts.
pub fn working_sentence(
    title: &str,
    step: Option<&str>,
    index: Option<usize>,
    total: usize,
) -> String {
    match (step, index) {
        (Some(step), Some(index)) => {
            let mut chars = step.chars();
            let step = match chars.next() {
                Some(first) => first.to_lowercase().chain(chars).collect::<String>(),
                None => String::new(),
            };
            format!("{title}: step {} of {total}, {step}…", index + 1)
        }
        _ => format!("{title}…"),
    }
}

fn describe_working(
    flow: &str,
    title: &str,
    step: Option<&str>,
    index: Option<usize>,
    total: usize,
) -> Description {
    let page = if flow == "sync" { "Library" } else { "Kernel" };
    (
        Tone::Busy,
        working_sentence(title, step, index, total),
        format!(
            "Leave the console and the USB cable alone until it finishes. The {page} page shows \
             every step."
        ),
        vec![],
    )
}

fn describe_connected(address: &str, link: &Link, info: &ConsoleInfo) -> Description {
    let mut details = Vec::new();
    let link_text = match link {
        Link::Usb {
            interface,
            host_address,
        } => format!("USB ({interface}, host {host_address})"),
        Link::Network => "network".to_string(),
    };

    if info.minimal_memboot {
        details.push(Detail::new("Hardware id", info.hwid.clone()));
        details.push(Detail::new("Address", address));
        details.push(Detail::new("Link", link_text));
        return (
            Tone::Ready,
            "Console running the hakchi recovery kernel from memory.".into(),
            "Nothing has been installed yet. Open the Kernel page to install hakchi, or switch \
             the console off and on to return to its own system."
                .into(),
            details,
        );
    }

    if let Some(versions) = &info.versions {
        details.push(Detail::new("hakchi", versions.script.clone()));
        details.push(Detail::new("Kernel", versions.kernel.clone()));
        details.push(Detail::new("Boot", versions.boot.clone()));
    }

    if !info.can_interact {
        details.push(Detail::new("Hardware id", info.hwid.clone()));
        details.push(Detail::new("Address", address));
        details.push(Detail::new("Link", link_text));
        let (title, next) = match &info.versions {
            None => (
                "Console with a very old hakchi kernel.".to_string(),
                "Reinstall the kernel from the Kernel page.".to_string(),
            ),
            Some(versions) if versions.requires_reflash() => (
                format!(
                    "Console with hakchi {}: the kernel is too old to use.",
                    versions.script
                ),
                "Reinstall the kernel from the Kernel page.".to_string(),
            ),
            Some(versions) => (
                format!(
                    "Console with hakchi {}: its system files are too old.",
                    versions.script
                ),
                "Update hakchi from the Kernel page.".to_string(),
            ),
        };
        return (Tone::Warning, title, next, details);
    }

    let model = model_name(info.console);
    let region = region_code(info.region());
    let mut title = model.to_string();
    if let Some(region) = region {
        title.push_str(", ");
        title.push_str(region);
    }
    if let Some(versions) = &info.versions {
        title.push_str(", hakchi ");
        title.push_str(&versions.script);
    }
    if let Some(storage) = &info.storage {
        title.push_str(", ");
        title.push_str(&format_size(storage.usage.free));
        title.push_str(" free");
    }

    let mut next = "Ready. Open the Library page to add and sync games.".to_string();
    if let Some(Firmware::Custom(path)) = &info.firmware {
        next.push_str(&format!(" The console runs a custom firmware ({path})."));
    }

    let mut front = vec![Detail::new("Model", model)];
    if let Some(region) = region {
        front.push(Detail::new("Region", region));
    }
    front.push(Detail::new("Hardware id", info.hwid.clone()));
    front.append(&mut details);
    details = front;
    if let Some(firmware) = &info.firmware {
        details.push(Detail::new("Firmware", firmware.to_string()));
    }
    if let Some(path) = &info.game_sync_path {
        details.push(Detail::new("Games storage", path.clone()));
    }
    if let Some(storage) = &info.storage {
        details.push(Detail::new(
            "Storage",
            format!(
                "{} total, {} used, {} free",
                format_size(storage.usage.total),
                format_size(storage.usage.used),
                format_size(storage.usage.free)
            ),
        ));
        details.push(Detail::new(
            "Games on console",
            format_size(storage.games.all),
        ));
        details.push(Detail::new(
            "Save states",
            format!(
                "{}{}",
                format_size(storage.saves),
                if storage.external_saves {
                    " (on external storage)"
                } else {
                    ""
                }
            ),
        ));
    }
    details.push(Detail::new(
        "Original games",
        info.original_games.len().to_string(),
    ));
    if info.md_partitioning {
        details.push(Detail::new("Partitioning", "Mega Drive layout"));
    }
    details.push(Detail::new("Address", address));
    details.push(Detail::new("Link", link_text));
    (Tone::Ready, title, next, details)
}

#[cfg(test)]
mod tests {
    use cartload_console::{GamesUsage, StorageReport, StorageUsage, Versions};

    use super::*;

    fn device() -> UsbDevice {
        UsbDevice {
            bus: "1".into(),
            address: 9,
            vendor_id: 0x1f3a,
            product_id: 0xefe8,
            manufacturer: None,
            product: None,
            serial: None,
        }
    }

    fn snes_eur() -> ConsoleInfo {
        ConsoleInfo {
            hwid: "0461872a872802079d2818c76c118000".into(),
            versions: Some(Versions {
                boot: "1.0.3".into(),
                kernel: "3.4.113.29-madmonkey".into(),
                script: "v1.0.4-126".into(),
            }),
            can_interact: true,
            console: ConsoleType::SnesEur,
            system_code: Some("snes-eur".into()),
            firmware: Some(Firmware::Nand),
            game_sync_path: Some("/var/lib/hakchi/games".into()),
            storage: Some(StorageReport {
                path: "/var/lib/hakchi/games".into(),
                usage: StorageUsage {
                    total: 369_737_728,
                    used: 32_718_848,
                    free: 325_582_848,
                },
                saves: 104_857,
                external_saves: false,
                games: GamesUsage {
                    all: 3_145_728,
                    ..GamesUsage::default()
                },
            }),
            original_games: (0..21).map(|i| format!("CLV-P-{i:05}")).collect(),
            ..ConsoleInfo::default()
        }
    }

    fn connected(info: ConsoleInfo) -> ConsoleStatus {
        ConsoleStatus::Connected {
            address: "169.254.13.37:22".into(),
            link: Link::Usb {
                interface: "hakchi0".into(),
                host_address: "169.254.68.74".into(),
            },
            since: 1_700_000_000_000,
            info: Box::new(info),
        }
    }

    #[test]
    fn nothing_found_reads_like_the_brief() {
        let report = ConsoleStatus::Disconnected {
            hint: Hint::Nothing,
        }
        .describe(1);
        assert_eq!(
            report.title,
            "Nothing found. Connect the console with USB and turn it on."
        );
        assert_eq!(report.tone, Tone::Neutral);
        assert!(report.next.contains("hold Reset"));
        assert!(report.details.is_empty());
        assert_eq!(report.changed_at, 1);
    }

    #[test]
    fn fel_ready_and_not_fel() {
        let ready = ConsoleStatus::Fel {
            device: device(),
            check: FelCheck::Fel {
                soc_id: 0x0016_6700,
                soc: "Allwinner R16".into(),
                r16: true,
            },
        }
        .describe(0);
        assert_eq!(
            ready.title,
            "Console in FEL mode: ready to install the kernel."
        );
        assert_eq!(ready.tone, Tone::Ready);
        assert!(ready.details.iter().any(|d| d.label == "Chip"));

        let mut stock = device();
        stock.product = Some("CLV-S-SNPY".into());
        let not_fel = ConsoleStatus::Fel {
            device: stock,
            check: FelCheck::NotFel {
                reason: "timeout".into(),
            },
        }
        .describe(0);
        assert_eq!(not_fel.tone, Tone::Warning);
        assert!(not_fel.title.contains("not in FEL mode"));
        assert!(
            not_fel
                .next
                .starts_with("It is on stock firmware in normal boot.")
        );
        assert!(not_fel.next.contains("hold Reset"));
    }

    #[test]
    fn booting_network_warns_after_thirty_seconds() {
        let status = ConsoleStatus::Booting {
            phase: BootPhase::Network,
            address: None,
            since: 10_000,
        };
        let early = status.describe(15_000);
        assert_eq!(early.tone, Tone::Busy);
        assert_eq!(early.next, "This usually takes a few seconds.");
        assert_eq!(early.details[0].value, "5 s");
        let late = status.describe(50_000);
        assert!(late.next.contains("Still waiting after 40 s"));
        assert!(late.next.contains("hakchi0"));
        assert!(!early.same_as(&late));
    }

    #[test]
    fn connected_title_matches_the_brief() {
        let report = connected(snes_eur()).describe(0);
        assert_eq!(
            report.title,
            "SNES Classic Mini, EUR, hakchi v1.0.4-126, 310 MB free"
        );
        assert_eq!(report.tone, Tone::Ready);
        assert!(report.next.starts_with("Ready."));
        let labels: Vec<&str> = report.details.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Model",
                "Region",
                "Hardware id",
                "hakchi",
                "Kernel",
                "Boot",
                "Firmware",
                "Games storage",
                "Storage",
                "Games on console",
                "Save states",
                "Original games",
                "Address",
                "Link"
            ]
        );
        let storage = report
            .details
            .iter()
            .find(|d| d.label == "Storage")
            .expect("storage");
        assert_eq!(storage.value, "353 MB total, 31 MB used, 310 MB free");
        assert_eq!(report.details[11].value, "21");
        assert_eq!(
            report.details[13].value,
            "USB (hakchi0, host 169.254.68.74)"
        );
    }

    #[test]
    fn memboot_shell_and_outdated_kernel() {
        let shell = connected(ConsoleInfo {
            hwid: "abcd".into(),
            minimal_memboot: true,
            ..ConsoleInfo::default()
        })
        .describe(0);
        assert!(shell.title.contains("recovery kernel from memory"));
        assert!(shell.next.contains("Kernel page"));
        assert_eq!(shell.details[0].value, "abcd");

        let outdated = connected(ConsoleInfo {
            hwid: "abcd".into(),
            versions: Some(Versions {
                boot: "1.0.1".into(),
                kernel: "3.4.112-00".into(),
                script: "v1.0.2-000".into(),
            }),
            can_interact: false,
            ..ConsoleInfo::default()
        })
        .describe(0);
        assert_eq!(outdated.tone, Tone::Warning);
        assert_eq!(
            outdated.title,
            "Console with hakchi v1.0.2-000: the kernel is too old to use."
        );
        assert_eq!(outdated.next, "Reinstall the kernel from the Kernel page.");

        let ancient = connected(ConsoleInfo {
            hwid: "abcd".into(),
            versions: None,
            ..ConsoleInfo::default()
        })
        .describe(0);
        assert_eq!(ancient.title, "Console with a very old hakchi kernel.");
    }

    #[test]
    fn sizes_and_names() {
        assert_eq!(format_size(325_582_848), "310 MB");
        assert_eq!(format_size(1_610_612_736), "1.5 GB");
        assert_eq!(format_size(98_304), "96 KB");
        assert_eq!(format_size(12), "12 B");
        assert_eq!(model_name(ConsoleType::MdUsa), "Genesis Mini");
        assert_eq!(region_code(Region::Asia), Some("ASIA"));
        assert_eq!(region_code(Region::Unknown), None);
    }

    #[test]
    fn status_serializes_with_a_state_tag() {
        let json = serde_json::to_value(ConsoleStatus::starting()).expect("json");
        assert_eq!(json["state"], "disconnected");
        assert_eq!(json["hint"]["kind"], "starting");
        let json = serde_json::to_value(connected(snes_eur())).expect("json");
        assert_eq!(json["state"], "connected");
        assert_eq!(json["link"]["kind"], "usb");
        assert_eq!(json["info"]["console"], "SnesEur");
        assert_eq!(json["info"]["firmware"], "Nand");
        let back: ConsoleStatus = serde_json::from_value(json).expect("round trip");
        assert!(back.is_connected());
        assert_eq!(
            back.info().map(|i| i.hwid.as_str()),
            Some("0461872a872802079d2818c76c118000")
        );
    }

    #[test]
    fn working_reads_as_one_busy_sentence() {
        let report = ConsoleStatus::Working {
            flow: "install".into(),
            title: "Installing hakchi".into(),
            step: Some("Grow the data partition".into()),
            index: Some(4),
            total: 13,
        }
        .describe(7);
        assert_eq!(report.tone, Tone::Busy);
        assert_eq!(
            report.title,
            "Installing hakchi: step 5 of 13, grow the data partition…"
        );
        assert!(report.next.contains("Leave the console"));
        assert!(!report.status.is_connected());
        assert_eq!(report.status.state_name(), "working");
        let report = ConsoleStatus::Working {
            flow: "memboot".into(),
            title: "Booting hakchi from memory".into(),
            step: None,
            index: None,
            total: 3,
        }
        .describe(8);
        assert_eq!(report.title, "Booting hakchi from memory…");
        let json = serde_json::to_value(&report.status).expect("json");
        assert_eq!(json["state"], "working");
        assert_eq!(json["total"], 3);
    }
}
