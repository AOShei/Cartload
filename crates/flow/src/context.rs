//! The shared state the hardware steps work on.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use cartload_console::{ExecOutput, Hakchi, OutputLine};
use cartload_fel::MembootOptions;
use cartload_hmod::{BootFiles, Package, basemods::LoadedMod, modhub::ModArchive};
use tracing::debug;

use crate::{
    ConsoleState, Error, Reporter, Result,
    backend::{Backend, Console, Link},
    kernels::{self, StockKernel},
};

/// Time limits and options shared by all flows. Every wait has a limit; the defaults follow
/// hakchi2 CE (`MembootWaitDelay` is 120 s) where it has one.
#[derive(Clone, Debug)]
pub struct FlowOptions {
    /// How long to wait for a console in FEL mode to appear on USB.
    pub fel_wait: Duration,
    /// How long to wait for the console to come up on the USB network after a memboot.
    pub console_wait: Duration,
    /// How long to wait for the console to come back after a reboot or kexec.
    pub reboot_wait: Duration,
    /// How long to wait for the console to drop off the network after a reboot command.
    pub offline_wait: Duration,
    /// Pause between probes while waiting.
    pub poll_interval: Duration,
    /// Time limit for ordinary device commands.
    pub command_timeout: Duration,
    /// Time limit for the device-side installer (`boot`), which flashes and copies the rootfs.
    pub install_timeout: Duration,
    /// Time limit for flash reads and writes (`getBackup2`, `flashBoot2`, `sunxi-part grow`).
    pub flash_timeout: Duration,
    /// Options for the FEL memboot (shell override, u-boot variant).
    pub memboot: MembootOptions,
    /// A stock kernel dump to flash when the console has no backup of its own.
    pub stock_kernel: Option<PathBuf>,
    /// Flash `stock_kernel` even when its MD5 is not in the known list.
    pub force_stock_kernel: bool,
}

impl Default for FlowOptions {
    fn default() -> Self {
        Self {
            fel_wait: Duration::from_secs(120),
            console_wait: Duration::from_secs(120),
            reboot_wait: Duration::from_secs(180),
            offline_wait: Duration::from_secs(30),
            poll_interval: Duration::from_secs(1),
            command_timeout: Duration::from_secs(60),
            install_timeout: Duration::from_secs(900),
            flash_timeout: Duration::from_secs(300),
            memboot: MembootOptions::default(),
            stock_kernel: None,
            force_stock_kernel: false,
        }
    }
}

/// `hakchi.hmod` as the flows use it: the boot files for memboot and the raw bytes to upload.
#[derive(Clone, Debug)]
pub struct HmodPackage {
    /// Where the package came from.
    pub path: PathBuf,
    /// `boot/boot.img`, `boot/uboot.bin` and `var/version`.
    pub files: BootFiles,
    /// The whole package, as uploaded to the console.
    pub bytes: Vec<u8>,
    /// Lowercase hex MD5 of `bytes`.
    pub md5: String,
}

impl HmodPackage {
    /// Reads and validates the package.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let package = Package::open(&path)?;
        package.validate()?;
        let files = package.boot_files()?;
        let bytes = std::fs::read(&path)?;
        let md5 = kernels::md5_hex(&bytes);
        Ok(Self {
            path,
            files,
            bytes,
            md5,
        })
    }

    /// Builds a package from parts (tests and callers that already hold the bytes).
    pub fn from_parts(path: impl Into<PathBuf>, files: BootFiles, bytes: Vec<u8>) -> Self {
        let md5 = kernels::md5_hex(&bytes);
        Self {
            path: path.into(),
            files,
            bytes,
            md5,
        }
    }
}

/// A stock kernel image held in memory for flashing.
#[derive(Clone, Debug)]
pub struct StockKernelImage {
    /// The image.
    pub bytes: Vec<u8>,
    /// Lowercase hex MD5 of `bytes`.
    pub md5: String,
    /// The known image it matches, if any.
    pub known: Option<&'static StockKernel>,
    /// Where it came from, for messages.
    pub origin: String,
}

impl StockKernelImage {
    /// Wraps `bytes` after checking they are an Android boot image.
    pub fn new(bytes: Vec<u8>, origin: impl Into<String>) -> Result<Self> {
        cartload_fel::memboot::BootImageHeader::parse(&bytes).map_err(|error| {
            Error::Precondition(format!("the stock kernel image is not usable: {error}"))
        })?;
        let md5 = kernels::md5_hex(&bytes);
        Ok(Self {
            known: kernels::identify(&md5),
            bytes,
            md5,
            origin: origin.into(),
        })
    }

    /// `N bytes, kernel-dp-... (SNES Classic Mini (EUR))`.
    pub fn describe(&self) -> String {
        format!(
            "{} bytes, {}",
            self.bytes.len(),
            kernels::describe(&self.md5)
        )
    }
}

/// Everything the hardware steps share: the backend, the reporter, the options, the package,
/// the current SSH session and what the flow learned so far.
pub struct Context {
    /// Access to the console.
    pub backend: Box<dyn Backend>,
    /// Where log lines and progress go.
    pub reporter: Reporter,
    /// Time limits and options.
    pub options: FlowOptions,
    /// The kernel package, when the flow has one.
    pub hmod: Option<HmodPackage>,
    /// hakchi2 CE base mods to install (loaded from the cache), when the flow installs any.
    pub base_mods: Vec<LoadedMod>,
    /// Emulator modules from the Mod Hub (RetroArch and cores) to install, when the flow
    /// installs any.
    pub mods: Vec<ModArchive>,
    /// The open SSH session, if any.
    pub console: Option<Box<dyn Console>>,
    /// What the last state detection found.
    pub state: ConsoleState,
    /// The data partition was grown in this run, so the kernel must reboot to see it.
    pub grew: bool,
    /// A reboot or kexec was just triggered: the next wait must see the console go away
    /// before it looks for it again.
    pub expect_cycle: bool,
    /// Stock kernel read from the console or from a file, waiting to be flashed.
    pub stock_kernel: Option<StockKernelImage>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("hmod", &self.hmod.as_ref().map(|h| &h.path))
            .field(
                "base_mods",
                &self
                    .base_mods
                    .iter()
                    .map(LoadedMod::name)
                    .collect::<Vec<_>>(),
            )
            .field(
                "mods",
                &self
                    .mods
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .field("connected", &self.console.is_some())
            .field("state", &self.state)
            .field("grew", &self.grew)
            .field("expect_cycle", &self.expect_cycle)
            .finish_non_exhaustive()
    }
}

impl Context {
    /// A context with no package and no session.
    pub fn new(backend: Box<dyn Backend>, reporter: Reporter, options: FlowOptions) -> Self {
        Self {
            backend,
            reporter,
            options,
            hmod: None,
            base_mods: Vec::new(),
            mods: Vec::new(),
            console: None,
            state: ConsoleState::Offline,
            grew: false,
            expect_cycle: false,
            stock_kernel: None,
        }
    }

    /// Attaches the kernel package.
    pub fn with_hmod(mut self, hmod: HmodPackage) -> Self {
        self.hmod = Some(hmod);
        self
    }

    /// Attaches base mods to install.
    pub fn with_base_mods(mut self, mods: Vec<LoadedMod>) -> Self {
        self.base_mods = mods;
        self
    }

    /// Attaches emulator modules to install.
    pub fn with_mods(mut self, mods: Vec<ModArchive>) -> Self {
        self.mods = mods;
        self
    }

    /// The kernel package, or a precondition error telling the user to pass one.
    pub fn hmod(&self) -> Result<&HmodPackage> {
        self.hmod.as_ref().ok_or_else(|| {
            Error::Precondition(
                "this step needs hakchi.hmod (boot/boot.img and boot/uboot.bin); pass --hmod \
                 <path>"
                    .to_string(),
            )
        })
    }

    /// The open session, or an error when there is none.
    pub fn console(&self) -> Result<&dyn Console> {
        self.console
            .as_deref()
            .ok_or_else(|| Error::WrongState("not connected to the console".to_string()))
    }

    /// A [`Shell`](cartload_console::Shell) on the open session with the ordinary command
    /// time limit.
    pub fn link(&self) -> Result<Link<'_>> {
        self.link_with_timeout(self.options.command_timeout)
    }

    /// A [`Shell`](cartload_console::Shell) on the open session with a custom time limit.
    pub fn link_with_timeout(&self, timeout: Duration) -> Result<Link<'_>> {
        Ok(Link::new(self.console()?, timeout))
    }

    /// Runs a command on the open session with the ordinary time limit.
    pub async fn exec(&self, command: &str) -> Result<ExecOutput> {
        self.exec_with(command, None, self.options.command_timeout)
            .await
    }

    /// Runs a command with stdin and a time limit.
    pub async fn exec_with(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<ExecOutput> {
        Ok(self.console()?.exec(command, stdin, timeout).await?)
    }

    /// Runs a command with a time limit, handing every line of output to `on_line` as it
    /// arrives.
    pub async fn exec_streaming(
        &self,
        command: &str,
        timeout: Duration,
        on_line: &mut (dyn FnMut(OutputLine) + Send),
    ) -> Result<ExecOutput> {
        Ok(self
            .console()?
            .exec_streaming(command, None, timeout, on_line)
            .await?)
    }

    /// Runs a command and turns a non-zero exit into [`Error::Device`] carrying the command,
    /// the exit code and what the console printed.
    pub async fn exec_checked(&self, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let output = self.exec_with(command, None, timeout).await?;
        if output.success() {
            Ok(output)
        } else {
            Err(device_failure(command, &output))
        }
    }

    /// Whether a command succeeds (exit code 0).
    pub async fn succeeds(&self, command: &str) -> Result<bool> {
        Ok(self.exec(command).await?.success())
    }

    /// Probes the USB link once without opening a session.
    pub async fn probe(&self) -> Result<Option<SocketAddr>> {
        self.backend.discover().await
    }

    /// Closes the session, if any.
    pub async fn disconnect(&mut self) {
        if let Some(console) = self.console.take() {
            console.close().await;
        }
    }

    /// Replaces the session with a fresh one. `Ok(false)` when nothing answers on the link;
    /// an SSH failure on a console that did answer is an error, so callers that poll can
    /// decide to retry.
    pub async fn connect(&mut self) -> Result<bool> {
        self.disconnect().await;
        let Some(address) = self.probe().await? else {
            return Ok(false);
        };
        let console = self.backend.connect(address).await?;
        debug!(%address, "connected");
        self.console = Some(console);
        Ok(true)
    }

    /// Reconnects and runs the hakchi2 CE identification sequence, recording the result in
    /// [`Context::state`].
    pub async fn refresh_state(&mut self) -> Result<&ConsoleState> {
        if !self.connect().await? {
            self.state = ConsoleState::Offline;
            return Ok(&self.state);
        }
        let info = {
            let link = self.link()?;
            Hakchi::new(&link).info().await?
        };
        self.state = ConsoleState::Online(Box::new(info));
        Ok(&self.state)
    }
}

/// [`Error::Device`] for a command that exited non-zero, with what the console printed.
pub fn device_failure(command: &str, output: &ExecOutput) -> Error {
    let mut text = format!("`{command}` ");
    match output.exit_code {
        Some(code) => text.push_str(&format!("exited with code {code}")),
        None => text.push_str("ended without an exit code (connection lost?)"),
    }
    let stderr = output.stderr_trimmed();
    let stdout = output.stdout_trimmed();
    if !stderr.is_empty() {
        text.push_str(&format!(": {stderr}"));
    } else if !stdout.is_empty() {
        text.push_str(&format!(": {stdout}"));
    }
    Error::Device(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_failure_prefers_stderr_then_stdout() {
        let output = ExecOutput {
            exit_code: Some(1),
            stdout: b"out\n".to_vec(),
            stderr: b"err\n".to_vec(),
        };
        assert_eq!(
            device_failure("hakchi x", &output).to_string(),
            "the console reported a failure: `hakchi x` exited with code 1: err"
        );
        let output = ExecOutput {
            exit_code: None,
            stdout: b"partial".to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(
            device_failure("boot", &output).to_string(),
            "the console reported a failure: `boot` ended without an exit code (connection \
             lost?): partial"
        );
    }

    #[test]
    fn stock_kernel_image_needs_a_boot_image() {
        let error = StockKernelImage::new(vec![0; 16], "file").expect_err("not an image");
        assert!(error.to_string().contains("not usable"));
    }

    #[test]
    fn defaults_have_every_wait_bounded() {
        let options = FlowOptions::default();
        assert!(options.fel_wait > Duration::ZERO);
        assert!(options.console_wait > Duration::ZERO);
        assert!(options.reboot_wait > Duration::ZERO);
        assert!(options.offline_wait > Duration::ZERO);
        assert!(options.install_timeout > options.command_timeout);
    }
}
