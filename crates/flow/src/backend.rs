//! What the flows need from the outside world: the USB network link to a running hakchi
//! kernel, and FEL for a console in boot ROM mode.
//!
//! [`UsbBackend`] is the real implementation on `cartload-console` and `cartload-fel`. The
//! traits exist so that the flows can be tested against a simulated console.

use std::{net::SocketAddr, time::Duration};

use async_trait::async_trait;
use cartload_console::{
    DiscoverOptions, ExecOutput, OutputLine, OutputStream, Session, SessionOptions, Shell, discover,
};
use cartload_fel::{
    Device, MembootEvent, MembootOptions, MembootReport, MembootStage, find_devices, memboot,
};
use cartload_hmod::BootFiles;
use tracing::debug;

use crate::{Reporter, Result};

/// An open SSH session to the console.
#[async_trait]
pub trait Console: Send + Sync {
    /// Address of the SSH server.
    fn address(&self) -> SocketAddr;

    /// Runs a shell command with optional stdin under a time limit. Overrunning the limit
    /// is reported as [`cartload_console::Error::CommandTimeout`]. A command whose connection
    /// drops (the console rebooted) returns whatever output arrived with no exit code.
    async fn exec(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> cartload_console::Result<ExecOutput>;

    /// Like [`Console::exec`], but hands every complete line of output to `on_line` as it
    /// arrives. The default runs the command to the end and replays its lines; a real
    /// session streams them.
    async fn exec_streaming(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
        timeout: Duration,
        on_line: &mut (dyn FnMut(OutputLine) + Send),
    ) -> cartload_console::Result<ExecOutput> {
        let output = self.exec(command, stdin, timeout).await?;
        for (stream, bytes) in [
            (OutputStream::Stdout, &output.stdout),
            (OutputStream::Stderr, &output.stderr),
        ] {
            for text in String::from_utf8_lossy(bytes).lines() {
                on_line(OutputLine {
                    stream,
                    text: text.trim_end_matches('\r').to_string(),
                });
            }
        }
        Ok(output)
    }

    /// Closes the session. Best effort: the console may already be gone.
    async fn close(self: Box<Self>);
}

/// Access to the console over USB.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Probes the USB network link once. `Ok(None)` when nothing answers: the console is
    /// powered off, on stock firmware, in FEL mode, or runs a legacy kernel without a network.
    async fn discover(&self) -> Result<Option<SocketAddr>>;

    /// Opens an SSH session to a console found by [`Backend::discover`].
    async fn connect(&self, address: SocketAddr) -> Result<Box<dyn Console>>;

    /// Whether a USB device with the FEL id (`1f3a:efe8`) is attached. Stock firmware in
    /// normal boot has the same id; only a memboot attempt tells them apart.
    async fn fel_device_present(&self) -> Result<bool>;

    /// Boots `files` from RAM through FEL. The device must be present.
    async fn fel_memboot(
        &self,
        files: &BootFiles,
        options: &MembootOptions,
        reporter: &Reporter,
    ) -> Result<MembootReport>;
}

/// Adapts a [`Console`] to the console crate's [`Shell`] trait, so that
/// [`Hakchi`](cartload_console::Hakchi) wrappers run on it with one time limit per command.
pub struct Link<'a> {
    console: &'a dyn Console,
    timeout: Duration,
}

impl<'a> Link<'a> {
    /// A shell on `console` whose commands must finish within `timeout`.
    pub fn new(console: &'a dyn Console, timeout: Duration) -> Self {
        Self { console, timeout }
    }

    /// The underlying console.
    pub fn console(&self) -> &'a dyn Console {
        self.console
    }
}

impl Shell for Link<'_> {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        stdin: Option<&'a [u8]>,
    ) -> impl Future<Output = cartload_console::Result<ExecOutput>> + Send + 'a {
        self.console.exec(command, stdin, self.timeout)
    }
}

/// The real backend: `cartload-console` for the network link, `cartload-fel` for FEL.
#[derive(Clone, Debug)]
pub struct UsbBackend {
    /// How the console is looked for on the USB link.
    pub discover: DiscoverOptions,
    /// Time limit for the TCP connect, key exchange and authentication, each.
    pub connect_timeout: Duration,
}

impl Default for UsbBackend {
    fn default() -> Self {
        Self {
            discover: DiscoverOptions::default(),
            connect_timeout: Duration::from_secs(5),
        }
    }
}

struct SessionConsole {
    session: Session,
}

#[async_trait]
impl Console for SessionConsole {
    fn address(&self) -> SocketAddr {
        self.session.address()
    }

    async fn exec(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> cartload_console::Result<ExecOutput> {
        tokio::time::timeout(timeout, self.session.exec_with_stdin(command, stdin))
            .await
            .map_err(|_| cartload_console::Error::CommandTimeout {
                command: command.to_string(),
                timeout,
            })?
    }

    async fn exec_streaming(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
        timeout: Duration,
        on_line: &mut (dyn FnMut(OutputLine) + Send),
    ) -> cartload_console::Result<ExecOutput> {
        tokio::time::timeout(
            timeout,
            self.session.exec_streaming(command, stdin, on_line),
        )
        .await
        .map_err(|_| cartload_console::Error::CommandTimeout {
            command: command.to_string(),
            timeout,
        })?
    }

    async fn close(self: Box<Self>) {
        if let Err(error) = self.session.close().await {
            debug!(%error, "closing the SSH session");
        }
    }
}

#[async_trait]
impl Backend for UsbBackend {
    async fn discover(&self) -> Result<Option<SocketAddr>> {
        match discover(&self.discover).await {
            Ok(found) => Ok(Some(found.address)),
            Err(
                cartload_console::Error::NoLinkLocalInterface
                | cartload_console::Error::NotFound { .. },
            ) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn connect(&self, address: SocketAddr) -> Result<Box<dyn Console>> {
        // No per-session command limit: every step passes its own through `Console::exec`.
        let options = SessionOptions {
            connect_timeout: self.connect_timeout,
            command_timeout: None,
            ..SessionOptions::default()
        };
        let session = Session::connect_with(address, options).await?;
        Ok(Box::new(SessionConsole { session }))
    }

    async fn fel_device_present(&self) -> Result<bool> {
        Ok(!find_devices().await?.is_empty())
    }

    async fn fel_memboot(
        &self,
        files: &BootFiles,
        options: &MembootOptions,
        reporter: &Reporter,
    ) -> Result<MembootReport> {
        let info = find_devices()
            .await?
            .into_iter()
            .next()
            .ok_or(cartload_fel::Error::DeviceNotFound)?;
        reporter.info(format!(
            "FEL device {:04x}:{:04x} on bus {} device {:03}",
            info.vendor_id(),
            info.product_id(),
            info.bus_id(),
            info.device_address()
        ));
        let mut device = Device::open_info(&info).await?;
        let mut last_percent = u64::MAX;
        let report = memboot(
            &mut device,
            cartload_hmod::fes1(),
            &files.uboot,
            &files.boot_image,
            options,
            |event| match event {
                MembootEvent::Stage(stage) => reporter.info(stage_text(stage)),
                MembootEvent::Upload(progress) => {
                    let done = progress.done as u64;
                    let total = progress.total as u64;
                    let percent = (done * 100).checked_div(total).unwrap_or(100);
                    if last_percent == u64::MAX || percent / 10 > last_percent / 10 || done == total
                    {
                        last_percent = percent;
                        reporter.progress("boot image", done, total);
                    }
                }
            },
        )
        .await?;
        Ok(report)
    }
}

/// User-facing text for a memboot stage.
pub fn stage_text(stage: MembootStage) -> &'static str {
    match stage {
        MembootStage::PreparingImages => "preparing the boot images",
        MembootStage::VerifyingDevice => "verifying the FEL device",
        MembootStage::InitialisingDram => "initialising DRAM with fes1.bin",
        MembootStage::UploadingKernel => "uploading the boot image",
        MembootStage::StartingUboot => "starting u-boot (boota)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_is_a_shell() {
        fn assert_shell<S: Shell>() {}
        assert_shell::<Link<'_>>();
    }

    #[test]
    fn stage_texts_are_distinct() {
        let stages = [
            MembootStage::PreparingImages,
            MembootStage::VerifyingDevice,
            MembootStage::InitialisingDram,
            MembootStage::UploadingKernel,
            MembootStage::StartingUboot,
        ];
        let texts: std::collections::BTreeSet<_> = stages.iter().map(|s| stage_text(*s)).collect();
        assert_eq!(texts.len(), stages.len());
    }
}
