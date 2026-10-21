//! Finding a console that runs the hakchi kernel.
//!
//! The primary path is the USB network gadget: the console always has `169.254.13.37/16`, so
//! whenever this host has a link-local address on an interface that is up, a TCP connect to
//! port 22 tells whether the console is there. The gadget interface reports `operstate unknown`
//! on Linux (no carrier detection), which counts as up. mDNS (`_ssh._tcp`, instance `hakchi`,
//! with `hwid`, `type` and `region` in the TXT record) is a second strategy for consoles on
//! Wi-Fi, behind [`DiscoverOptions::mdns`].

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use if_addrs::{IfAddr, IfOperStatus};
use mdns_sd::{IfKind, ResolvedService, ServiceDaemon, ServiceEvent};
use serde::Serialize;
use tokio::{io::AsyncReadExt, net::TcpSocket, time::timeout};
use tracing::{debug, info, warn};

use crate::{ConsoleType, Error, Result, parse::parse_hwid};

/// Address the hakchi ramdisk gives the console's USB network gadget (`S92rndis`).
pub const CONSOLE_ADDRESS: Ipv4Addr = Ipv4Addr::new(169, 254, 13, 37);
/// SSH port on the console.
pub const SSH_PORT: u16 = 22;
/// mDNS service type the console advertises (tinysvcmdns, IPv4 only).
pub const MDNS_SERVICE_TYPE: &str = "_ssh._tcp.local.";
/// mDNS instance name of the console's service.
pub const MDNS_INSTANCE: &str = "hakchi";

/// How to look for the console.
#[derive(Clone, Debug)]
pub struct DiscoverOptions {
    /// Address probed over the USB link.
    pub address: Ipv4Addr,
    /// Port probed over the USB link (and expected from mDNS).
    pub port: u16,
    /// Time allowed for one TCP connect.
    pub probe_timeout: Duration,
    /// Also browse mDNS, for consoles on Wi-Fi.
    pub mdns: bool,
    /// How long to wait for mDNS answers.
    pub mdns_timeout: Duration,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            address: CONSOLE_ADDRESS,
            port: SSH_PORT,
            probe_timeout: Duration::from_millis(500),
            mdns: false,
            mdns_timeout: Duration::from_secs(3),
        }
    }
}

impl DiscoverOptions {
    /// The socket address probed over the USB link.
    pub fn target(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.address), self.port)
    }
}

/// How a console was found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum DiscoverySource {
    /// TCP probe of the fixed address over a link-local interface.
    UsbLink {
        /// Host interface the probe went out on.
        interface: String,
        /// Host address on that interface.
        host_address: Ipv4Addr,
    },
    /// mDNS announcement.
    Mdns {
        /// Service instance name (`hakchi`).
        instance: String,
        /// Host name in the announcement.
        host: String,
    },
}

/// A console that answered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Discovered {
    /// Where SSH answers.
    pub address: SocketAddr,
    /// Which strategy found it.
    pub source: DiscoverySource,
    /// SSH identification line, when the probe read one (`SSH-2.0-dropbear_2018.76`).
    pub banner: Option<String>,
    /// Hardware id from the mDNS TXT record, normalised.
    pub hwid: Option<String>,
    /// Console type derived from the mDNS TXT record (`type` and `region`).
    pub console: Option<ConsoleType>,
    /// Raw `type` value from the mDNS TXT record (`snes`).
    pub console_type: Option<String>,
    /// Raw `region` value from the mDNS TXT record (`eur`).
    pub region: Option<String>,
}

impl Discovered {
    /// Copies the hardware id, console type and raw TXT values from another sighting of the
    /// same console (an mDNS announcement) into this one, where this one has none.
    pub fn merge_metadata(&mut self, other: &Discovered) {
        if self.banner.is_none() {
            self.banner.clone_from(&other.banner);
        }
        if self.hwid.is_none() {
            self.hwid.clone_from(&other.hwid);
        }
        if self.console.is_none() {
            self.console = other.console;
        }
        if self.console_type.is_none() {
            self.console_type.clone_from(&other.console_type);
        }
        if self.region.is_none() {
            self.region.clone_from(&other.region);
        }
    }
}

impl std::fmt::Display for Discovered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "console at {}", self.address)?;
        match &self.source {
            DiscoverySource::UsbLink {
                interface,
                host_address,
            } => write!(f, " via USB link on {interface} (host {host_address})")?,
            DiscoverySource::Mdns { instance, host } => {
                write!(f, " via mDNS ({instance} on {host})")?;
            }
        }
        if let Some(banner) = &self.banner {
            write!(f, ", {banner}")?;
        }
        if let Some(console) = self.console {
            write!(f, ", {console}")?;
        }
        if let Some(hwid) = &self.hwid {
            write!(f, ", hwid {hwid}")?;
        }
        Ok(())
    }
}

/// A host interface with a link-local IPv4 address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LinkLocalInterface {
    /// Interface name (`hakchi0` with the udev rule from `docs/hardware-tests.md`).
    pub name: String,
    /// The link-local address.
    pub address: Ipv4Addr,
    /// Operational state as the OS reports it (`up`, `unknown`, `down`, ...).
    pub operstate: String,
    /// Whether the interface is considered up for probing.
    pub usable: bool,
}

/// Whether an operational state counts as up. USB gadgets have no carrier detection and report
/// `unknown` on Linux, which must be accepted.
pub fn operstate_is_up(operstate: &str) -> bool {
    matches!(operstate.trim(), "up" | "unknown")
}

/// Lists the host interfaces that hold a 169.254.x.x address.
pub fn link_local_interfaces() -> Result<Vec<LinkLocalInterface>> {
    let mut interfaces = Vec::new();
    for interface in if_addrs::get_if_addrs()? {
        let IfAddr::V4(v4) = &interface.addr else {
            continue;
        };
        if !v4.ip.is_link_local() {
            continue;
        }
        let operstate = operstate(&interface);
        interfaces.push(LinkLocalInterface {
            name: interface.name.clone(),
            address: v4.ip,
            usable: operstate_is_up(&operstate),
            operstate,
        });
    }
    Ok(interfaces)
}

fn operstate(interface: &if_addrs::Interface) -> String {
    #[cfg(target_os = "linux")]
    if let Ok(state) =
        std::fs::read_to_string(format!("/sys/class/net/{}/operstate", interface.name))
    {
        return state.trim().to_string();
    }
    match &interface.oper_status {
        IfOperStatus::Up => "up".to_string(),
        IfOperStatus::Unknown => "unknown".to_string(),
        IfOperStatus::Down => "down".to_string(),
        IfOperStatus::Dormant => "dormant".to_string(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Tries a TCP connection to `target`, optionally bound to a local address so the attempt goes
/// out on a specific interface. On success returns the first line the server sent (the SSH
/// identification string), or an empty string if nothing arrived within a second.
pub async fn probe(
    target: SocketAddr,
    bind: Option<Ipv4Addr>,
    connect_timeout: Duration,
) -> std::io::Result<String> {
    let socket = TcpSocket::new_v4()?;
    if let Some(address) = bind
        && let Err(error) = socket.bind(SocketAddr::new(IpAddr::V4(address), 0))
    {
        debug!(%address, %error, "could not bind probe socket; probing unbound");
    }
    let mut stream = timeout(connect_timeout, socket.connect(target))
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("no answer from {target} within {connect_timeout:?}"),
            )
        })??;
    let mut buffer = [0u8; 256];
    let read = match timeout(Duration::from_secs(1), stream.read(&mut buffer)).await {
        Ok(Ok(read)) => read,
        _ => 0,
    };
    let banner = String::from_utf8_lossy(&buffer[..read])
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(banner)
}

/// Finds the console: probes the USB link on every usable link-local interface, then (if
/// enabled) browses mDNS. Returns the first console that answers.
pub async fn discover(options: &DiscoverOptions) -> Result<Discovered> {
    let target = options.target();
    let interfaces = link_local_interfaces()?;
    let mut probed = Vec::new();
    for interface in &interfaces {
        if !interface.usable {
            debug!(
                name = interface.name,
                operstate = interface.operstate,
                "skipping link-local interface that is not up"
            );
            continue;
        }
        probed.push(format!("{} ({})", interface.name, interface.address));
        match probe(target, Some(interface.address), options.probe_timeout).await {
            Ok(banner) => {
                let found = Discovered {
                    address: target,
                    source: DiscoverySource::UsbLink {
                        interface: interface.name.clone(),
                        host_address: interface.address,
                    },
                    banner: Some(banner).filter(|b| !b.is_empty()),
                    hwid: None,
                    console: None,
                    console_type: None,
                    region: None,
                };
                info!("{found}");
                return Ok(found);
            }
            Err(error) => debug!(
                name = interface.name,
                %target,
                %error,
                "USB link probe failed"
            ),
        }
    }

    if options.mdns {
        if let Some(found) = browse_mdns(options, true).await?.into_iter().next() {
            return Ok(found);
        }
        probed.push("mDNS".to_string());
    }

    if probed.is_empty() {
        return Err(Error::NoLinkLocalInterface);
    }
    Err(Error::NotFound {
        address: target,
        probed: probed.join(", "),
    })
}

/// Runs every strategy to completion and returns all consoles that answered (the USB link
/// first, then mDNS announcements, without duplicates by address).
pub async fn discover_all(options: &DiscoverOptions) -> Result<Vec<Discovered>> {
    let mut found = Vec::new();
    match discover(&DiscoverOptions {
        mdns: false,
        ..options.clone()
    })
    .await
    {
        Ok(console) => found.push(console),
        Err(Error::NoLinkLocalInterface | Error::NotFound { .. }) => {}
        Err(error) => return Err(error),
    }
    if options.mdns {
        for console in browse_mdns(options, false).await? {
            match found.iter_mut().find(|f| f.address == console.address) {
                Some(existing) => existing.merge_metadata(&console),
                None => found.push(console),
            }
        }
    }
    Ok(found)
}

/// Browses mDNS for the console's `_ssh._tcp` announcement until the timeout, returning every
/// console announced.
pub async fn discover_mdns(options: &DiscoverOptions) -> Result<Vec<Discovered>> {
    browse_mdns(options, false).await
}

async fn browse_mdns(options: &DiscoverOptions, stop_at_first: bool) -> Result<Vec<Discovered>> {
    let mdns = |error: mdns_sd::Error| Error::Mdns(error.to_string());
    let daemon = ServiceDaemon::new().map_err(mdns)?;
    // The console only announces IPv4; skip IPv6 sockets to keep the noise down.
    if let Err(error) = daemon.disable_interface(IfKind::IPv6) {
        debug!(%error, "could not disable IPv6 for mDNS");
    }
    let receiver = daemon.browse(MDNS_SERVICE_TYPE).map_err(mdns)?;
    let deadline = Instant::now() + options.mdns_timeout;
    let mut found: Vec<Discovered> = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let Ok(Ok(event)) = timeout(remaining, receiver.recv_async()).await else {
            break;
        };
        match event {
            ServiceEvent::ServiceResolved(service) => {
                debug!(fullname = service.fullname, "mDNS service resolved");
                let Some(console) = discovered_from_mdns(&service) else {
                    continue;
                };
                if found.iter().any(|f| f.address == console.address) {
                    continue;
                }
                info!("{console}");
                found.push(console);
                if stop_at_first {
                    break;
                }
            }
            other => debug!(?other, "mDNS event"),
        }
    }
    if let Err(error) = daemon.stop_browse(MDNS_SERVICE_TYPE) {
        debug!(%error, "stop_browse failed");
    }
    if let Err(error) = daemon.shutdown() {
        warn!(%error, "mDNS daemon shutdown failed");
    }
    Ok(found)
}

fn discovered_from_mdns(service: &ResolvedService) -> Option<Discovered> {
    let instance = service.fullname.split('.').next().unwrap_or("").to_string();
    let txt = &service.txt_properties;
    let hwid = txt
        .get_property_val_str("hwid")
        .and_then(|raw| parse_hwid(raw).ok());
    if instance != MDNS_INSTANCE && hwid.is_none() {
        return None;
    }
    let address = service
        .addresses
        .iter()
        .map(mdns_sd::ScopedIp::to_ip_addr)
        .find(IpAddr::is_ipv4)?;
    let console_type = txt.get_property_val_str("type").map(str::to_string);
    let region = txt.get_property_val_str("region").map(str::to_string);
    Some(Discovered {
        address: SocketAddr::new(address, service.port),
        source: DiscoverySource::Mdns {
            instance,
            host: service.host.clone(),
        },
        banner: None,
        hwid,
        console: console_from_txt(console_type.as_deref(), region.as_deref()),
        console_type,
        region,
    })
}

/// Derives the console type from the `type` and `region` TXT values.
pub fn console_from_txt(console_type: Option<&str>, region: Option<&str>) -> Option<ConsoleType> {
    Some(ConsoleType::from_type_and_region(console_type?, region?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operstate_unknown_counts_as_up() {
        assert!(operstate_is_up("up"));
        assert!(operstate_is_up("unknown\n"));
        assert!(!operstate_is_up("down"));
        assert!(!operstate_is_up("dormant"));
        assert!(!operstate_is_up("lowerlayerdown"));
        assert!(!operstate_is_up(""));
    }

    #[test]
    fn defaults_target_the_console_address() {
        let options = DiscoverOptions::default();
        assert_eq!(options.target().to_string(), "169.254.13.37:22");
        assert!(!options.mdns);
    }

    #[test]
    fn txt_record_from_trace_gives_snes_eur() {
        assert_eq!(
            console_from_txt(Some("snes"), Some("eur")),
            Some(ConsoleType::SnesEur)
        );
        assert_eq!(console_from_txt(Some("snes"), None), None);
        assert_eq!(
            console_from_txt(Some("md"), Some("asia")),
            Some(ConsoleType::MdAsia)
        );
        assert_eq!(
            console_from_txt(Some("nope"), Some("eur")),
            Some(ConsoleType::Unknown)
        );
    }

    #[test]
    fn link_local_enumeration_does_not_fail() {
        // No assumptions about the host, only that enumeration works and the fields agree.
        for interface in link_local_interfaces().expect("enumerating interfaces") {
            assert!(interface.address.is_link_local());
            assert_eq!(interface.usable, operstate_is_up(&interface.operstate));
        }
    }

    #[tokio::test]
    async fn probe_times_out_on_an_unroutable_address() {
        // 192.0.2.0/24 (TEST-NET-1) is never routed; the connect must give up on the timeout.
        let target = SocketAddr::from(([192, 0, 2, 1], 22));
        let started = Instant::now();
        let result = probe(target, None, Duration::from_millis(200)).await;
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn probe_reads_the_banner_of_a_local_listener() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let address = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = tokio::io::AsyncWriteExt::write_all(
                    &mut socket,
                    b"SSH-2.0-dropbear_2018.76\r\n",
                )
                .await;
            }
        });
        let banner = probe(address, None, Duration::from_secs(1))
            .await
            .expect("connect");
        assert_eq!(banner, "SSH-2.0-dropbear_2018.76");
    }

    #[test]
    fn merge_metadata_fills_gaps_only() {
        let address: SocketAddr = "169.254.13.37:22".parse().expect("addr");
        let mut usb = Discovered {
            address,
            source: DiscoverySource::UsbLink {
                interface: "hakchi0".into(),
                host_address: Ipv4Addr::new(169, 254, 68, 74),
            },
            banner: Some("SSH-2.0-dropbear_2018.76".into()),
            hwid: None,
            console: None,
            console_type: None,
            region: None,
        };
        let mdns = Discovered {
            address,
            source: DiscoverySource::Mdns {
                instance: "hakchi".into(),
                host: "madmonkey.local.".into(),
            },
            banner: None,
            hwid: Some("0461872a872802079d2818c76c118000".into()),
            console: Some(ConsoleType::SnesEur),
            console_type: Some("snes".into()),
            region: Some("eur".into()),
        };
        usb.merge_metadata(&mdns);
        assert_eq!(usb.banner.as_deref(), Some("SSH-2.0-dropbear_2018.76"));
        assert_eq!(
            usb.hwid.as_deref(),
            Some("0461872a872802079d2818c76c118000")
        );
        assert_eq!(usb.console, Some(ConsoleType::SnesEur));
        assert_eq!(usb.region.as_deref(), Some("eur"));
        assert!(matches!(usb.source, DiscoverySource::UsbLink { .. }));
    }

    #[test]
    fn discovered_display_mentions_source() {
        let found = Discovered {
            address: "169.254.13.37:22".parse().expect("addr"),
            source: DiscoverySource::UsbLink {
                interface: "hakchi0".into(),
                host_address: Ipv4Addr::new(169, 254, 29, 194),
            },
            banner: Some("SSH-2.0-dropbear_2018.76".into()),
            hwid: None,
            console: None,
            console_type: None,
            region: None,
        };
        assert_eq!(
            found.to_string(),
            "console at 169.254.13.37:22 via USB link on hakchi0 (host 169.254.29.194), \
             SSH-2.0-dropbear_2018.76"
        );
    }
}
