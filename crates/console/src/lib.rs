//! Talking to a console that runs the hakchi kernel: discovery, SSH and FTP sessions, and typed
//! wrappers for the device-side `hakchi` script.
//!
//! A booted hakchi kernel enumerates as a USB network gadget (`04e8:6863`) with the fixed
//! address `169.254.13.37`, runs dropbear on port 22 (`root`, empty password) and busybox
//! `ftpd` on port 21. The usual flow is:
//!
//! 1. [`discover()`] probes the fixed address over every link-local interface (and mDNS for
//!    Wi-Fi consoles when [`DiscoverOptions::mdns`] is set).
//! 2. [`Session::connect`] opens SSH; [`Session::exec`] runs commands, [`Session::upload`]
//!    writes small files, [`Session::ftp`] opens an FTP connection for bulk transfers.
//! 3. [`Hakchi`] wraps the `hakchi` script: [`Hakchi::info`] runs the connect sequence that
//!    hakchi2 CE runs and returns a [`ConsoleInfo`].
//!
//! The device knowledge comes from `docs/device-protocol.md`, the ramdisk of the hakchi
//! kernel and hakchi2 CE (`hakchi.cs`, `SshClient/`, `MemoryStats.cs`). Nothing here needs
//! hardware to compile or test: [`Hakchi`] is generic over a [`Shell`], and the parsers in
//! [`parse`] are plain functions.

mod discover;
mod error;
mod ftp;
mod hakchi;
pub mod parse;
mod session;

pub use discover::{
    CONSOLE_ADDRESS, DiscoverOptions, Discovered, DiscoverySource, LinkLocalInterface,
    MDNS_INSTANCE, MDNS_SERVICE_TYPE, SSH_PORT, console_from_txt, discover, discover_all,
    discover_mdns, link_local_interfaces, operstate_is_up, probe,
};
pub use error::{Error, Result};
pub use ftp::{FTP_PORT, Ftp, Progress, path_prefixes};
pub use hakchi::{ConsoleInfo, Hakchi, Shell, commands};
pub use parse::{
    ConsoleFamily, ConsoleType, DirSize, Firmware, GamesUsage, Region, StorageReport, StorageUsage,
    Version, Versions,
};
pub use session::{
    ExecOutput, HostKey, LineSplitter, OutputLine, OutputStream, PASSWORD, Session, SessionOptions,
    USERNAME,
};

/// Crate name and version, for CLI banners and diagnostics.
pub const fn version() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_mentions_crate_name() {
        assert!(version().starts_with(env!("CARGO_PKG_NAME")));
    }

    #[test]
    fn constants_match_the_protocol_notes() {
        assert_eq!(CONSOLE_ADDRESS.to_string(), "169.254.13.37");
        assert_eq!(SSH_PORT, 22);
        assert_eq!(FTP_PORT, 21);
        assert_eq!(USERNAME, "root");
        assert_eq!(PASSWORD, "");
        assert_eq!(MDNS_SERVICE_TYPE, "_ssh._tcp.local.");
    }

    #[test]
    fn session_futures_are_send() {
        fn assert_send<T: Send>(_: T) {}
        let address: std::net::SocketAddr = "169.254.13.37:22".parse().expect("addr");
        assert_send(Session::connect(address));
        assert_send(discover(&DiscoverOptions::default()));
    }
}
