//! Prepare, upload and run the device-side install (hakchi2 CE `HandleHakchi(Install)`,
//! `TransferHakchiHmod` and `BootHakchi`).

use async_trait::async_trait;
use cartload_console::ExecOutput;

use crate::{
    Context, Error, Outcome, Result, Step, device_failure,
    steps::{commands, remote_md5, upload},
};

/// Appends `cf_install=y` and `cf_update=y` to `/hakchi/config` and creates
/// `/hakchi/transfer`. Every SSH command on the memboot shell sources that config, so the
/// installer run later sees the flags.
#[derive(Clone, Copy, Debug, Default)]
pub struct PrepareInstall;

#[async_trait]
impl Step<Context> for PrepareInstall {
    fn name(&self) -> &str {
        "prepare"
    }

    fn description(&self) -> &str {
        "Flag the install in /hakchi/config and create /hakchi/transfer"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        Ok(ctx
            .succeeds(commands::INSTALL_FLAGGED)
            .await?
            .then(|| "cf_install and cf_update are set and /hakchi/transfer exists".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        for command in [
            commands::FLAG_INSTALL,
            commands::FLAG_UPDATE,
            commands::MKDIR_TRANSFER,
        ] {
            ctx.exec_checked(command, ctx.options.command_timeout)
                .await?;
        }
        Ok(Outcome::Done)
    }
}

/// Copies `hakchi.hmod` to `/hakchi/transfer/hakchi.hmod` over SSH and checks its MD5.
#[derive(Clone, Copy, Debug, Default)]
pub struct UploadHmod;

#[async_trait]
impl Step<Context> for UploadHmod {
    fn name(&self) -> &str {
        "upload"
    }

    fn description(&self) -> &str {
        "Copy hakchi.hmod to /hakchi/transfer on the console"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        let local = ctx.hmod()?.md5.clone();
        Ok((remote_md5(ctx, commands::HMOD_PATH).await? == Some(local))
            .then(|| "hakchi.hmod is already in /hakchi/transfer with the same MD5".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        let hmod = ctx.hmod()?;
        ctx.reporter.info(format!(
            "uploading {} ({} bytes, hakchi {})",
            hmod.path.display(),
            hmod.bytes.len(),
            hmod.files.version
        ));
        upload(ctx, commands::HMOD_PATH, &hmod.bytes, "hakchi.hmod").await?;
        let remote = remote_md5(ctx, commands::HMOD_PATH).await?;
        if remote.as_deref() != Some(hmod.md5.as_str()) {
            return Err(Error::Device(format!(
                "{} has MD5 {} after the upload, expected {}",
                commands::HMOD_PATH,
                remote.unwrap_or_else(|| "(missing)".to_string()),
                hmod.md5
            )));
        }
        Ok(Outcome::Done)
    }
}

/// Runs `boot`, the device-side installer: it backs up the stock kernel, flashes u-boot and
/// the hakchi kernel, copies the rootfs to the data partition and reboots. The SSH connection
/// drops when the console reboots, so the command ends without an exit code.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunInstaller;

#[async_trait]
impl Step<Context> for RunInstaller {
    fn name(&self) -> &str {
        "install"
    }

    fn description(&self) -> &str {
        "Run the device-side installer (boot): back up the stock kernel, flash u-boot and the \
         hakchi kernel, install the rootfs, reboot"
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        let timeout = ctx.options.install_timeout;
        ctx.reporter.info(
            "running the installer on the console; it takes a few minutes and the connection \
             drops when the console reboots",
        );
        let output = match ctx.exec_with(commands::BOOT, None, timeout).await {
            Ok(output) => output,
            Err(Error::Console(cartload_console::Error::CommandTimeout { .. })) => {
                return Err(Error::Timeout {
                    what: "the installer (`boot`)".to_string(),
                    timeout,
                    hint: "the console may still be installing; watch its screen, wait for it \
                           to reboot, then rerun the command: finished steps are skipped and \
                           the install is verified"
                        .to_string(),
                });
            }
            Err(Error::Console(error)) => {
                ctx.reporter
                    .debug(format!("installer connection ended: {error}"));
                ExecOutput::default()
            }
            Err(error) => return Err(error),
        };
        let stdout = output.stdout_lossy();
        let stderr = output.stderr_lossy();
        for line in stdout.lines().chain(stderr.lines()) {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            ctx.reporter.info(format!("console: {line}"));
            if commands::FAILURE_MARKERS.iter().any(|m| line.contains(m)) {
                return Err(Error::Device(format!("the installer reported: {line}")));
            }
        }
        if matches!(output.exit_code, Some(code) if code != 0) {
            return Err(device_failure(commands::BOOT, &output));
        }
        ctx.expect_cycle = true;
        ctx.disconnect().await;
        Ok(Outcome::Done)
    }
}
