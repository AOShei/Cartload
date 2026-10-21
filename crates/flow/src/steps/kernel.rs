//! Stock kernel handling: read the backup, flash it back, remove the install, dump to a file
//! (hakchi2 CE `GetStockKernel`, `FlashKernel`, `HandleHakchi(Uninstall)`, `DumpStockKernel`).

use std::path::PathBuf;

use async_trait::async_trait;
use cartload_console::Hakchi;

use crate::{
    Context, Error, Outcome, Result, Step, StockKernelImage, device_failure,
    steps::{commands, upload},
};

/// `hakchi getBackup2`, or an error with what the console said.
async fn read_backup(ctx: &Context) -> Result<Vec<u8>> {
    let output = ctx
        .exec_with(commands::GET_BACKUP2, None, ctx.options.flash_timeout)
        .await?;
    if !output.success() || output.stdout.is_empty() {
        return Err(device_failure(commands::GET_BACKUP2, &output));
    }
    Ok(output.stdout)
}

/// Reads the stock kernel backup from NAND into memory, or takes it from the file given with
/// [`FlowOptions::stock_kernel`](crate::FlowOptions::stock_kernel) when the console has none.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadStockKernel;

#[async_trait]
impl Step<Context> for ReadStockKernel {
    fn name(&self) -> &str {
        "read-stock-kernel"
    }

    fn description(&self) -> &str {
        "Read the stock kernel backup, or take the given file"
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.state.md_partitioning() {
            return Ok(Outcome::NotNeeded(
                "the Mega Drive Mini keeps its stock kernel in place".to_string(),
            ));
        }
        let image = match read_backup(ctx).await {
            Ok(bytes) => StockKernelImage::new(bytes, "the console's backup")?,
            Err(error) => {
                ctx.reporter
                    .warn(format!("no stock kernel backup on the console: {error}"));
                let Some(path) = ctx.options.stock_kernel.clone() else {
                    return Err(Error::Precondition(
                        "the console has no stock kernel backup; pass --stock-kernel \
                         <kernel.img> from an earlier dump-kernel run"
                            .to_string(),
                    ));
                };
                let bytes = tokio::fs::read(&path).await?;
                let image = StockKernelImage::new(bytes, path.display().to_string())?;
                if image.known.is_none() && !ctx.options.force_stock_kernel {
                    return Err(Error::Precondition(format!(
                        "{} is {}; pass --force-stock-kernel to flash it anyway",
                        path.display(),
                        image.describe()
                    )));
                }
                image
            }
        };
        ctx.reporter.info(format!(
            "stock kernel from {}: {}",
            image.origin,
            image.describe()
        ));
        ctx.stock_kernel = Some(image);
        Ok(Outcome::Done)
    }
}

/// Uploads the stock kernel to `/kernel.img`, validates it with `sntool check` and flashes it
/// with `hakchi flashBoot2` (which verifies the flash by MD5).
#[derive(Clone, Copy, Debug, Default)]
pub struct FlashStockKernel;

#[async_trait]
impl Step<Context> for FlashStockKernel {
    fn name(&self) -> &str {
        "flash-stock-kernel"
    }

    fn description(&self) -> &str {
        "Write the stock kernel back to NAND"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        if ctx.state.md_partitioning() {
            return Ok(None);
        }
        Ok(ctx
            .succeeds(commands::IS_ORIGINAL_BOOT2)
            .await?
            .then(|| "the kernel partition already holds the stock kernel".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.state.md_partitioning() {
            return Ok(Outcome::NotNeeded(
                "the Mega Drive Mini keeps its stock kernel in place".to_string(),
            ));
        }
        let Some(image) = ctx.stock_kernel.clone() else {
            return Err(Error::Precondition(
                "no stock kernel in memory; the read-stock-kernel step must run first".to_string(),
            ));
        };
        ctx.reporter
            .info(format!("uploading the stock kernel ({})", image.describe()));
        upload(ctx, commands::KERNEL_PATH, &image.bytes, "stock kernel").await?;
        let check = ctx.exec(commands::SNTOOL_CHECK_KERNEL).await?;
        if !check.success() {
            return Err(Error::Device(format!(
                "the stock kernel image failed `sntool check`: {}",
                check.stdout_trimmed()
            )));
        }
        let info = ctx
            .exec_checked(commands::SNTOOL_KERNEL_INFO, ctx.options.command_timeout)
            .await?;
        for line in info.stdout_lossy().lines().filter(|l| !l.trim().is_empty()) {
            ctx.reporter.debug(format!("sntool: {line}"));
        }
        ctx.reporter
            .info("flashing the stock kernel (hakchi flashBoot2, verified by MD5)");
        let flash = ctx
            .exec_with(commands::FLASH_BOOT2, None, ctx.options.flash_timeout)
            .await?;
        if !flash.success() {
            return Err(Error::Device(format!(
                "flashing the stock kernel failed verification: {}",
                flash.stderr_trimmed()
            )));
        }
        Ok(Outcome::Done)
    }
}

/// `hakchi mod_uninstall` between `mount_base` and `umount_base`: removes
/// `/var/lib/hakchi`, restores the stock u-boot and shrinks the data partition.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemoveInstall;

#[async_trait]
impl Step<Context> for RemoveInstall {
    fn name(&self) -> &str {
        "remove"
    }

    fn description(&self) -> &str {
        "Remove hakchi's files and restore the stock u-boot"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        let link = ctx.link()?;
        let hakchi = Hakchi::new(&link);
        hakchi.mount_base().await?;
        let exists = ctx.succeeds(commands::INSTALL_DIR_EXISTS).await;
        if let Err(error) = hakchi.umount_base().await {
            ctx.reporter.debug(format!("umount_base: {error}"));
        }
        Ok((!exists?).then(|| "no /var/lib/hakchi on the data partition".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        let link = ctx.link()?;
        let hakchi = Hakchi::new(&link);
        hakchi.mount_base().await?;
        let result = ctx
            .exec_with(commands::MOD_UNINSTALL, None, ctx.options.flash_timeout)
            .await;
        if let Err(error) = hakchi.umount_base().await {
            ctx.reporter.debug(format!("umount_base: {error}"));
        }
        let output = result?;
        for line in output
            .stdout_lossy()
            .lines()
            .filter(|l| !l.trim().is_empty())
        {
            ctx.reporter.info(format!("console: {line}"));
        }
        if !output.success() {
            return Err(device_failure(commands::MOD_UNINSTALL, &output));
        }
        Ok(Outcome::Done)
    }
}

/// Reads the stock kernel backup and writes it to a file.
#[derive(Clone, Debug)]
pub struct DumpStockKernel {
    path: PathBuf,
}

impl DumpStockKernel {
    /// Dumps to `path` (`kernel.img`).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl Step<Context> for DumpStockKernel {
    fn name(&self) -> &str {
        "dump-stock-kernel"
    }

    fn description(&self) -> &str {
        "Save the stock kernel backup to the file"
    }

    async fn already_done(&self, _ctx: &mut Context) -> Result<Option<String>> {
        let Ok(bytes) = tokio::fs::read(&self.path).await else {
            return Ok(None);
        };
        Ok(StockKernelImage::new(bytes, "file")
            .ok()
            .map(|image| format!("{} already holds {}", self.path.display(), image.describe())))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.state.md_partitioning() {
            return Err(Error::Precondition(
                "dumping the stock kernel is not supported on the Mega Drive Mini partition \
                 layout (hakchi2 CE refuses it too)"
                    .to_string(),
            ));
        }
        let bytes = read_backup(ctx).await.map_err(|error| {
            Error::Device(format!(
                "no stock kernel backup on the console ({error}); on a console that never had \
                 hakchi installed, install it first: the installer backs the stock kernel up"
            ))
        })?;
        let image = StockKernelImage::new(bytes, "the console's backup")?;
        tokio::fs::write(&self.path, &image.bytes).await?;
        ctx.reporter.info(format!(
            "wrote {}: {}",
            self.path.display(),
            image.describe()
        ));
        Ok(Outcome::Done)
    }
}
