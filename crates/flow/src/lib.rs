//! Resumable, observable flows that take a Classic Mini console from stock firmware (or a
//! broken install) to a working hakchi kernel, and back.
//!
//! A [`Flow`] is an ordered list of [`Step`]s. Each step has a name, a description, an
//! idempotency check and a `run`; the engine runs them in order, reports progress and log
//! lines through a [`tokio::sync::mpsc`] channel ([`Event`]), and keeps its position on a
//! failed step so the flow can be resumed without redoing finished work.
//!
//! The flows ([`flows::install_kernel`], [`flows::memboot`], [`flows::uninstall_kernel`],
//! [`flows::dump_stock_kernel`]) follow the sequences hakchi2 CE runs in `MembootTasks.cs`:
//! memboot through FEL (or kexec from a running kernel), wait for the console on the USB
//! network, identify it, grow the data partition, upload `hakchi.hmod`, run the device-side
//! installer (`boot`), wait for the reboot, reconnect and verify. The device knowledge is in
//! `docs/device-protocol.md`.
//!
//! Nothing here needs hardware to compile or test: the steps talk to the console through the
//! [`Backend`] and [`Console`] traits, implemented for real USB by [`UsbBackend`] and by a
//! simulated console in the tests.

pub mod backend;
mod context;
pub mod engine;
mod error;
pub mod flows;
pub mod kernels;
pub mod partitions;
mod state;
pub mod steps;

pub use backend::{Backend, Console, Link, UsbBackend};
pub use context::{Context, FlowOptions, HmodPackage, StockKernelImage, device_failure};
pub use engine::{
    Event, EventReceiver, EventSender, Flow, FlowReport, Level, Outcome, Reporter, Step, StepInfo,
    StepResult, channel,
};
pub use error::{Error, Result};
pub use state::ConsoleState;

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
}
