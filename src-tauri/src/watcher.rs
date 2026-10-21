//! Background watcher: every two seconds it looks at the USB bus and the USB network link,
//! decides which [`ConsoleStatus`] the console is in, opens and identifies an SSH session
//! when a hakchi kernel answers, and reports every change through a [`StatusSink`].
//!
//! The decision is a pure function ([`decide`]) over an [`Observation`], so the state machine
//! is tested without hardware; the loop and the actions (connect, identify, verify FEL) are
//! thin wrappers over the crates.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use cartload_console::{
    DiscoverOptions, DiscoverySource, Error as ConsoleError, Hakchi, Session, SessionOptions,
    discover,
};
use cartload_fel::{Device, UsbTransport};
use serde::Serialize;
use tokio::time::{Instant, MissedTickBehavior};
use tracing::{debug, info, warn};

use crate::{
    error::CommandError,
    state::AppState,
    status::{BootPhase, ConsoleStatus, FelCheck, Hint, Link, StatusReport, now_ms},
    usb::{self, UsbDevice, UsbScan},
};

/// Receives every status change.
pub trait StatusSink: Send + Sync {
    /// Called with the new report after it has been stored in the [`AppState`].
    fn status_changed(&self, report: &StatusReport);
}

/// Timing and probing settings.
#[derive(Clone, Debug)]
pub struct WatcherOptions {
    /// Time between two looks.
    pub interval: Duration,
    /// How the USB network link is probed.
    pub discover: DiscoverOptions,
    /// Limit for the TCP connect, key exchange and authentication, each.
    pub connect_timeout: Duration,
    /// Limit for one command of the identification sequence.
    pub command_timeout: Duration,
    /// How long after the FEL device disappears a kernel is still expected to come up.
    pub fel_grace: Duration,
    /// How long to wait before connecting again after a failed attempt.
    pub retry_delay: Duration,
    /// Whether a device with the FEL ids is opened once to confirm it is the boot ROM.
    pub verify_fel: bool,
    /// Per-transfer limit for that check.
    pub fel_timeout: Duration,
}

impl Default for WatcherOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(2),
            discover: DiscoverOptions::default(),
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(30),
            fel_grace: Duration::from_secs(45),
            retry_delay: Duration::from_secs(8),
            verify_fel: true,
            fel_timeout: Duration::from_secs(1),
        }
    }
}

/// What probing the USB network link found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkProbe {
    /// SSH answered.
    Found {
        /// Where.
        address: SocketAddr,
        /// Host interface the probe went out on, for the USB link.
        interface: Option<String>,
        /// Host address on that interface.
        host_address: Option<String>,
    },
    /// No link-local interface, so nothing to probe.
    NoInterface,
    /// Interfaces exist but nothing answered.
    NotFound {
        /// What was probed.
        probed: String,
    },
    /// Probing itself failed.
    Failed(String),
    /// Not probed this time (a session over the USB link is open).
    Skipped,
}

/// State of the stored session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionHealth {
    /// No session.
    None,
    /// A session that has not been closed.
    Open,
    /// A session the other side (or a keepalive) closed.
    Closed,
}

/// One look at the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Console-related USB devices.
    pub usb: UsbScan,
    /// The network probe.
    pub link: LinkProbe,
    /// The stored session.
    pub session: SessionHealth,
}

/// What to do after deciding the status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing; close the session if the status is not connected.
    Keep,
    /// Open a session and run the identification sequence.
    Connect {
        /// Where.
        address: SocketAddr,
        /// How the console is reached.
        link: Link,
    },
    /// Run the identification sequence again on the open session.
    Reidentify,
    /// Ask the device with the FEL ids whether it is the boot ROM.
    VerifyFel(UsbDevice),
}

/// What the watcher remembers between ticks.
#[derive(Debug, Default)]
pub struct Memory {
    /// No connection attempt before this time.
    pub retry_after: Option<Instant>,
    /// FEL checks by device key, valid while the device stays plugged in.
    pub fel_checks: Vec<(String, FelCheck)>,
}

impl Memory {
    fn fel_check(&self, key: &str) -> FelCheck {
        self.fel_checks
            .iter()
            .find(|(k, _)| k == key)
            .map_or(FelCheck::Unchecked, |(_, check)| check.clone())
    }

    fn remember_fel_check(&mut self, key: String, check: FelCheck) {
        self.fel_checks.retain(|(k, _)| *k != key);
        self.fel_checks.push((key, check));
    }

    fn may_connect(&self, now: Instant) -> bool {
        self.retry_after.is_none_or(|after| now >= after)
    }
}

/// Decides the next status from the previous one and an observation. Pure.
pub fn decide(
    prev: &ConsoleStatus,
    obs: &Observation,
    memory: &Memory,
    now: Instant,
    now_ms: u64,
    force_identify: bool,
    options: &WatcherOptions,
) -> (ConsoleStatus, Action) {
    if let ConsoleStatus::Connected { link, .. } = prev {
        let reachable = match link {
            Link::Usb { .. } => obs.usb.gadget_present(),
            Link::Network => matches!(obs.link, LinkProbe::Found { .. } | LinkProbe::Skipped),
        };
        if obs.session == SessionHealth::Open && reachable {
            let action = if force_identify {
                Action::Reidentify
            } else {
                Action::Keep
            };
            return (prev.clone(), action);
        }
    }

    if let LinkProbe::Found {
        address,
        interface,
        host_address,
    } = &obs.link
    {
        if memory.may_connect(now) {
            let link = if obs.usb.gadget_present() {
                Link::Usb {
                    interface: interface.clone().unwrap_or_default(),
                    host_address: host_address.clone().unwrap_or_default(),
                }
            } else {
                Link::Network
            };
            return (
                ConsoleStatus::Booting {
                    phase: BootPhase::Connecting,
                    address: Some(address.to_string()),
                    since: now_ms,
                },
                Action::Connect {
                    address: *address,
                    link,
                },
            );
        }
        if matches!(
            prev,
            ConsoleStatus::Disconnected {
                hint: Hint::ConnectFailed { .. }
            }
        ) {
            return (prev.clone(), Action::Keep);
        }
    }

    if obs.usb.gadget_present() {
        let since = match prev {
            ConsoleStatus::Booting {
                phase: BootPhase::Network,
                since,
                ..
            } => *since,
            _ => now_ms,
        };
        return (
            ConsoleStatus::Booting {
                phase: BootPhase::Network,
                address: None,
                since,
            },
            Action::Keep,
        );
    }

    if let Some(device) = obs.usb.fel.first() {
        let check = memory.fel_check(&device.key());
        let action = if check == FelCheck::Unchecked && options.verify_fel {
            Action::VerifyFel(device.clone())
        } else {
            Action::Keep
        };
        return (
            ConsoleStatus::Fel {
                device: device.clone(),
                check,
            },
            action,
        );
    }

    match prev {
        ConsoleStatus::Fel { .. } => {
            return (
                ConsoleStatus::Booting {
                    phase: BootPhase::LeavingFel,
                    address: None,
                    since: now_ms,
                },
                Action::Keep,
            );
        }
        ConsoleStatus::Booting {
            phase: BootPhase::LeavingFel,
            since,
            ..
        } if now_ms.saturating_sub(*since) < options.fel_grace.as_millis() as u64 => {
            return (prev.clone(), Action::Keep);
        }
        _ => {}
    }

    let hint = match &obs.link {
        LinkProbe::NotFound { probed } => Hint::LinkNoAnswer {
            interface: probed.clone(),
        },
        LinkProbe::Failed(reason) => Hint::ProbeFailed {
            reason: reason.clone(),
        },
        LinkProbe::NoInterface | LinkProbe::Skipped | LinkProbe::Found { .. } => Hint::Nothing,
    };
    (ConsoleStatus::Disconnected { hint }, Action::Keep)
}

/// Result of an explicit FEL probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FelProbe {
    /// The device that was opened.
    pub device: UsbDevice,
    /// What it answered.
    pub check: FelCheck,
}

/// The watcher: owns the loop and the actions.
pub struct Watcher {
    state: Arc<AppState>,
    sink: Arc<dyn StatusSink>,
    options: WatcherOptions,
    memory: Mutex<Memory>,
}

impl Watcher {
    /// A watcher over `state`, reporting to `sink`.
    pub fn new(state: Arc<AppState>, sink: Arc<dyn StatusSink>, options: WatcherOptions) -> Self {
        Self {
            state,
            sink,
            options,
            memory: Mutex::new(Memory::default()),
        }
    }

    /// The shared state.
    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    /// Runs forever: one [`Watcher::tick`] per interval, none while the state is paused.
    pub async fn run(self: Arc<Self>) {
        info!(
            interval_ms = self.options.interval.as_millis(),
            "console watcher started"
        );
        let mut interval = tokio::time::interval(self.options.interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            if self.state.is_paused() {
                continue;
            }
            self.tick(false).await;
        }
    }

    /// Stores and announces a status; returns its report.
    pub fn publish(&self, status: ConsoleStatus) -> StatusReport {
        let report = status.describe(now_ms());
        if self.state.set_report(report.clone()) {
            info!(state = ?report.status.state_name(), "{}", report.title);
            self.sink.status_changed(&report);
        }
        report
    }

    /// One look at the world, followed by whatever it calls for. With `force_identify` an
    /// open session is re-identified (the "refresh" command).
    pub async fn tick(&self, force_identify: bool) -> StatusReport {
        let _serialised = self.state.tick_lock.lock().await;
        let prev = self.state.report().status;
        let obs = self.observe(&prev).await;
        let (status, action) = {
            let memory = self.memory.lock().unwrap_or_else(PoisonError::into_inner);
            decide(
                &prev,
                &obs,
                &memory,
                Instant::now(),
                now_ms(),
                force_identify,
                &self.options,
            )
        };
        debug!(?action, "watcher decision");
        if !status.is_connected() && obs.session != SessionHealth::None {
            if obs.session == SessionHealth::Open {
                info!("console no longer reachable; closing the session");
            } else {
                info!("the console closed the session");
            }
            self.state.drop_session().await;
        }
        let report = self.publish(status.clone());
        match action {
            Action::Keep => report,
            Action::Reidentify => self.reidentify(status).await,
            Action::Connect { address, link } => self.connect_and_identify(address, link).await,
            Action::VerifyFel(device) => {
                let check = self.verify_fel(&device).await;
                self.remember(&device, check.clone());
                self.publish(ConsoleStatus::Fel { device, check })
            }
        }
    }

    /// Opens the first device with the FEL ids and asks for its version block, whatever the
    /// watcher thinks. The next tick shows the result.
    pub async fn fel_probe(&self) -> Result<FelProbe, CommandError> {
        let (device, check) = {
            let _serialised = self.state.tick_lock.lock().await;
            let scan = usb::scan().await?;
            let device =
                scan.fel.first().cloned().ok_or_else(|| {
                    CommandError::from_error(&cartload_fel::Error::DeviceNotFound)
                })?;
            let check = self.verify_fel(&device).await;
            self.remember(&device, check.clone());
            (device, check)
        };
        self.tick(false).await;
        Ok(FelProbe { device, check })
    }

    fn remember(&self, device: &UsbDevice, check: FelCheck) {
        self.memory
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remember_fel_check(device.key(), check);
    }

    fn set_retry(&self) {
        self.memory
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retry_after = Some(Instant::now() + self.options.retry_delay);
    }

    async fn observe(&self, prev: &ConsoleStatus) -> Observation {
        let usb = match usb::scan().await {
            Ok(scan) => scan,
            Err(error) => {
                warn!(%error, "could not list USB devices");
                UsbScan::default()
            }
        };
        let session = match self.state.session.lock().await.as_ref() {
            None => SessionHealth::None,
            Some(session) if session.is_closed() => SessionHealth::Closed,
            Some(_) => SessionHealth::Open,
        };
        let over_usb = matches!(
            prev,
            ConsoleStatus::Connected {
                link: Link::Usb { .. },
                ..
            }
        );
        let link = if session == SessionHealth::Open && over_usb {
            LinkProbe::Skipped
        } else {
            match discover(&self.options.discover).await {
                Ok(found) => {
                    let (interface, host_address) = match &found.source {
                        DiscoverySource::UsbLink {
                            interface,
                            host_address,
                        } => (Some(interface.clone()), Some(host_address.to_string())),
                        DiscoverySource::Mdns { .. } => (None, None),
                    };
                    LinkProbe::Found {
                        address: found.address,
                        interface,
                        host_address,
                    }
                }
                Err(ConsoleError::NoLinkLocalInterface) => LinkProbe::NoInterface,
                Err(ConsoleError::NotFound { probed, .. }) => LinkProbe::NotFound { probed },
                Err(error) => LinkProbe::Failed(error.to_string()),
            }
        };
        Observation { usb, link, session }
    }

    async fn connect_and_identify(&self, address: SocketAddr, link: Link) -> StatusReport {
        let options = SessionOptions {
            connect_timeout: self.options.connect_timeout,
            command_timeout: Some(self.options.command_timeout),
            ..SessionOptions::default()
        };
        let session = match Session::connect_with(address, options).await {
            Ok(session) => session,
            Err(error) => {
                let refused = matches!(error, ConsoleError::AuthFailed { .. });
                warn!(%address, %error, "could not open the SSH session");
                self.set_retry();
                return self.publish(ConsoleStatus::Disconnected {
                    hint: Hint::ConnectFailed {
                        address: address.to_string(),
                        reason: error.to_string(),
                        refused,
                    },
                });
            }
        };
        self.publish(ConsoleStatus::Booting {
            phase: BootPhase::Identifying,
            address: Some(address.to_string()),
            since: now_ms(),
        });
        match Hakchi::new(&session).info().await {
            Ok(info) => {
                info!(hwid = info.hwid, console = %info.console, "console identified");
                *self.state.session.lock().await = Some(session);
                self.publish(ConsoleStatus::Connected {
                    address: address.to_string(),
                    link,
                    since: now_ms(),
                    info: Box::new(info),
                })
            }
            Err(error) => {
                warn!(%address, %error, "could not identify the console");
                let _ = session.close().await;
                self.set_retry();
                self.publish(ConsoleStatus::Disconnected {
                    hint: Hint::ConnectFailed {
                        address: address.to_string(),
                        reason: error.to_string(),
                        refused: false,
                    },
                })
            }
        }
    }

    async fn reidentify(&self, prev: ConsoleStatus) -> StatusReport {
        let ConsoleStatus::Connected {
            address,
            link,
            since,
            ..
        } = prev
        else {
            return self.state.report();
        };
        let result = {
            let guard = self.state.session.lock().await;
            match guard.as_ref() {
                Some(session) => Hakchi::new(session).info().await,
                None => Err(ConsoleError::Closed),
            }
        };
        match result {
            Ok(info) => {
                info!(hwid = info.hwid, "console information refreshed");
                self.publish(ConsoleStatus::Connected {
                    address,
                    link,
                    since,
                    info: Box::new(info),
                })
            }
            Err(error) => {
                warn!(%error, "refreshing the console information failed");
                self.state.drop_session().await;
                self.set_retry();
                self.publish(ConsoleStatus::Disconnected {
                    hint: Hint::ConnectFailed {
                        address,
                        reason: error.to_string(),
                        refused: false,
                    },
                })
            }
        }
    }

    /// Opens the device and asks for its FEL version block. Read-only; the device is
    /// released right after.
    async fn verify_fel(&self, device: &UsbDevice) -> FelCheck {
        let info = match usb::find_fel_info(&device.key()).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                return FelCheck::Unavailable {
                    reason: "the device disappeared".into(),
                };
            }
            Err(error) => {
                return FelCheck::Unavailable {
                    reason: error.to_string(),
                };
            }
        };
        let mut transport = match UsbTransport::open(&info).await {
            Ok(transport) => transport,
            Err(error @ cartload_fel::Error::NoBulkEndpoints) => {
                return FelCheck::NotFel {
                    reason: error.to_string(),
                };
            }
            Err(error) => {
                warn!(%error, "could not open the FEL device");
                return FelCheck::Unavailable {
                    reason: error.to_string(),
                };
            }
        };
        transport.set_timeout(self.options.fel_timeout);
        let mut fel = Device::from_transport(transport);
        match fel.verify_device().await {
            Ok(response) => {
                info!("{response}");
                FelCheck::Fel {
                    soc_id: response.soc_id,
                    soc: response.soc_name().unwrap_or("unknown SoC").to_string(),
                    r16: response.is_r16(),
                }
            }
            Err(error) => {
                debug!(%error, "device with FEL ids did not answer as the boot ROM");
                FelCheck::NotFel {
                    reason: error.to_string(),
                }
            }
        }
    }
}

impl ConsoleStatus {
    /// The state's tag, for logging.
    pub fn state_name(&self) -> &'static str {
        match self {
            Self::Disconnected { .. } => "disconnected",
            Self::Fel { .. } => "fel",
            Self::Booting { .. } => "booting",
            Self::Connected { .. } => "connected",
        }
    }
}

#[cfg(test)]
mod tests {
    use cartload_console::ConsoleInfo;

    use super::*;

    fn fel_device() -> UsbDevice {
        UsbDevice {
            bus: "1".into(),
            address: 4,
            vendor_id: 0x1f3a,
            product_id: 0xefe8,
            manufacturer: None,
            product: None,
            serial: None,
        }
    }

    fn gadget() -> UsbDevice {
        UsbDevice {
            bus: "1".into(),
            address: 22,
            vendor_id: 0x04e8,
            product_id: 0x6863,
            manufacturer: Some("hakchi".into()),
            product: Some("hakchi".into()),
            serial: None,
        }
    }

    fn address() -> SocketAddr {
        "169.254.13.37:22".parse().expect("addr")
    }

    fn found() -> LinkProbe {
        LinkProbe::Found {
            address: address(),
            interface: Some("hakchi0".into()),
            host_address: Some("169.254.68.74".into()),
        }
    }

    fn connected() -> ConsoleStatus {
        ConsoleStatus::Connected {
            address: address().to_string(),
            link: Link::Usb {
                interface: "hakchi0".into(),
                host_address: "169.254.68.74".into(),
            },
            since: 1000,
            info: Box::new(ConsoleInfo::default()),
        }
    }

    fn observe(usb: UsbScan, link: LinkProbe, session: SessionHealth) -> Observation {
        Observation { usb, link, session }
    }

    fn run(
        prev: &ConsoleStatus,
        obs: &Observation,
        memory: &Memory,
        now_ms: u64,
    ) -> (ConsoleStatus, Action) {
        decide(
            prev,
            obs,
            memory,
            Instant::now(),
            now_ms,
            false,
            &WatcherOptions::default(),
        )
    }

    #[test]
    fn nothing_attached_is_disconnected() {
        let (status, action) = run(
            &ConsoleStatus::starting(),
            &observe(
                UsbScan::default(),
                LinkProbe::NoInterface,
                SessionHealth::None,
            ),
            &Memory::default(),
            5000,
        );
        assert_eq!(
            status,
            ConsoleStatus::Disconnected {
                hint: Hint::Nothing
            }
        );
        assert_eq!(action, Action::Keep);

        let (status, _) = run(
            &ConsoleStatus::starting(),
            &observe(
                UsbScan::default(),
                LinkProbe::NotFound {
                    probed: "enp8s0 (169.254.1.2)".into(),
                },
                SessionHealth::None,
            ),
            &Memory::default(),
            5000,
        );
        assert_eq!(
            status,
            ConsoleStatus::Disconnected {
                hint: Hint::LinkNoAnswer {
                    interface: "enp8s0 (169.254.1.2)".into()
                }
            }
        );
    }

    #[test]
    fn ssh_answer_leads_to_connect_over_the_usb_link() {
        let usb = UsbScan::classify([gadget()]);
        let (status, action) = run(
            &ConsoleStatus::starting(),
            &observe(usb, found(), SessionHealth::None),
            &Memory::default(),
            7000,
        );
        assert_eq!(
            status,
            ConsoleStatus::Booting {
                phase: BootPhase::Connecting,
                address: Some("169.254.13.37:22".into()),
                since: 7000,
            }
        );
        assert_eq!(
            action,
            Action::Connect {
                address: address(),
                link: Link::Usb {
                    interface: "hakchi0".into(),
                    host_address: "169.254.68.74".into(),
                },
            }
        );

        // Without the gadget (a console on Wi-Fi) the link is "network".
        let (_, action) = run(
            &ConsoleStatus::starting(),
            &observe(UsbScan::default(), found(), SessionHealth::None),
            &Memory::default(),
            7000,
        );
        assert!(matches!(
            action,
            Action::Connect {
                link: Link::Network,
                ..
            }
        ));
    }

    #[test]
    fn gadget_without_ssh_is_booting_and_keeps_its_start_time() {
        let usb = UsbScan::classify([gadget()]);
        let (first, _) = run(
            &ConsoleStatus::starting(),
            &observe(usb.clone(), LinkProbe::NoInterface, SessionHealth::None),
            &Memory::default(),
            1000,
        );
        assert_eq!(
            first,
            ConsoleStatus::Booting {
                phase: BootPhase::Network,
                address: None,
                since: 1000,
            }
        );
        let (second, _) = run(
            &first,
            &observe(
                usb,
                LinkProbe::NotFound {
                    probed: "hakchi0".into(),
                },
                SessionHealth::None,
            ),
            &Memory::default(),
            3000,
        );
        assert_eq!(second, first, "the phase start time is kept");
    }

    #[test]
    fn connected_stays_connected_while_the_gadget_is_there() {
        let usb = UsbScan::classify([gadget()]);
        let (status, action) = run(
            &connected(),
            &observe(usb.clone(), LinkProbe::Skipped, SessionHealth::Open),
            &Memory::default(),
            9000,
        );
        assert_eq!(status, connected());
        assert_eq!(action, Action::Keep);

        let (_, action) = decide(
            &connected(),
            &observe(usb, LinkProbe::Skipped, SessionHealth::Open),
            &Memory::default(),
            Instant::now(),
            9000,
            true,
            &WatcherOptions::default(),
        );
        assert_eq!(
            action,
            Action::Reidentify,
            "refresh re-runs the identification"
        );
    }

    #[test]
    fn unplugging_a_connected_console_disconnects_at_once() {
        let (status, action) = run(
            &connected(),
            &observe(
                UsbScan::default(),
                LinkProbe::NoInterface,
                SessionHealth::Open,
            ),
            &Memory::default(),
            9000,
        );
        assert_eq!(
            status,
            ConsoleStatus::Disconnected {
                hint: Hint::Nothing
            }
        );
        assert_eq!(action, Action::Keep);

        // A closed session with the gadget still enumerated: reconnect when SSH answers.
        let usb = UsbScan::classify([gadget()]);
        let (status, action) = run(
            &connected(),
            &observe(usb, found(), SessionHealth::Closed),
            &Memory::default(),
            9000,
        );
        assert!(matches!(
            status,
            ConsoleStatus::Booting {
                phase: BootPhase::Connecting,
                ..
            }
        ));
        assert!(matches!(action, Action::Connect { .. }));
    }

    #[test]
    fn fel_device_is_verified_once_then_remembered() {
        let usb = UsbScan::classify([fel_device()]);
        let (status, action) = run(
            &ConsoleStatus::starting(),
            &observe(usb.clone(), LinkProbe::NoInterface, SessionHealth::None),
            &Memory::default(),
            100,
        );
        assert_eq!(
            status,
            ConsoleStatus::Fel {
                device: fel_device(),
                check: FelCheck::Unchecked
            }
        );
        assert_eq!(action, Action::VerifyFel(fel_device()));

        let mut memory = Memory::default();
        let check = FelCheck::Fel {
            soc_id: 0x0016_6700,
            soc: "Allwinner R16".into(),
            r16: true,
        };
        memory.remember_fel_check(fel_device().key(), check.clone());
        let (status, action) = run(
            &status,
            &observe(usb, LinkProbe::NoInterface, SessionHealth::None),
            &memory,
            2100,
        );
        assert_eq!(
            status,
            ConsoleStatus::Fel {
                device: fel_device(),
                check
            }
        );
        assert_eq!(action, Action::Keep);

        // Verification can be switched off.
        let options = WatcherOptions {
            verify_fel: false,
            ..WatcherOptions::default()
        };
        let (_, action) = decide(
            &ConsoleStatus::starting(),
            &observe(
                UsbScan::classify([fel_device()]),
                LinkProbe::NoInterface,
                SessionHealth::None,
            ),
            &Memory::default(),
            Instant::now(),
            100,
            false,
            &options,
        );
        assert_eq!(action, Action::Keep);
    }

    #[test]
    fn leaving_fel_waits_for_the_kernel_then_gives_up() {
        let fel = ConsoleStatus::Fel {
            device: fel_device(),
            check: FelCheck::Unchecked,
        };
        let nothing = observe(
            UsbScan::default(),
            LinkProbe::NoInterface,
            SessionHealth::None,
        );
        let (leaving, _) = run(&fel, &nothing, &Memory::default(), 10_000);
        assert_eq!(
            leaving,
            ConsoleStatus::Booting {
                phase: BootPhase::LeavingFel,
                address: None,
                since: 10_000,
            }
        );
        let (still, _) = run(&leaving, &nothing, &Memory::default(), 40_000);
        assert_eq!(still, leaving);
        let (gone, _) = run(&leaving, &nothing, &Memory::default(), 56_000);
        assert_eq!(
            gone,
            ConsoleStatus::Disconnected {
                hint: Hint::Nothing
            }
        );
        // The kernel coming up wins over the grace period.
        let (booting, _) = run(
            &leaving,
            &observe(
                UsbScan::classify([gadget()]),
                LinkProbe::NoInterface,
                SessionHealth::None,
            ),
            &Memory::default(),
            20_000,
        );
        assert!(matches!(
            booting,
            ConsoleStatus::Booting {
                phase: BootPhase::Network,
                ..
            }
        ));
    }

    #[test]
    fn failed_connections_back_off() {
        let failed = ConsoleStatus::Disconnected {
            hint: Hint::ConnectFailed {
                address: "169.254.13.37:22".into(),
                reason: "refused".into(),
                refused: true,
            },
        };
        let memory = Memory {
            retry_after: Some(Instant::now() + Duration::from_secs(60)),
            fel_checks: Vec::new(),
        };
        let (status, action) = run(
            &failed,
            &observe(UsbScan::default(), found(), SessionHealth::None),
            &memory,
            1000,
        );
        assert_eq!(status, failed);
        assert_eq!(action, Action::Keep);

        let expired = Memory {
            retry_after: Some(Instant::now() - Duration::from_secs(1)),
            fel_checks: Vec::new(),
        };
        let (_, action) = run(
            &failed,
            &observe(UsbScan::default(), found(), SessionHealth::None),
            &expired,
            1000,
        );
        assert!(matches!(action, Action::Connect { .. }));
    }
}
