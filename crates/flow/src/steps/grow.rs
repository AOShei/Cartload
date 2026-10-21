//! Grow the data partition (hakchi2 CE `GrowTask`).

use async_trait::async_trait;

use crate::{
    Context, Outcome, Result, Step,
    partitions::{Partition, data_partition, has_udisk, human_size, parse_sunxi_part},
    steps::{commands, kexec_memboot},
};

/// Reads the NAND partition table.
async fn partitions(ctx: &Context) -> Result<Vec<Partition>> {
    let output = ctx.exec(commands::SUNXI_PART).await?;
    Ok(parse_sunxi_part(&output.stdout_lossy()))
}

/// Merges the unused `UDISK` partition into the data partition with `sunxi-part grow`, as
/// hakchi2 CE always does on install. A console without `UDISK` was grown already.
#[derive(Clone, Copy, Debug, Default)]
pub struct GrowPartition;

#[async_trait]
impl Step<Context> for GrowPartition {
    fn name(&self) -> &str {
        "grow"
    }

    fn description(&self) -> &str {
        "Merge the unused UDISK partition into the data partition"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        let partitions = partitions(ctx).await?;
        if has_udisk(&partitions) {
            return Ok(None);
        }
        Ok(Some(if partitions.is_empty() {
            "sunxi-part listed no partitions, nothing to grow".to_string()
        } else {
            "no UDISK partition left; the data partition is already grown".to_string()
        }))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        let before = partitions(ctx).await?;
        let dry_run = ctx.exec(commands::SUNXI_PART_GROW_DRY_RUN).await?;
        let after = parse_sunxi_part(&dry_run.stdout_lossy());
        match (data_partition(&before), data_partition(&after)) {
            (Some(old), Some(new)) if new.bytes() > old.bytes() => ctx.reporter.info(format!(
                "growing the data partition ({}) from {} to {}",
                old.device,
                human_size(old.bytes()),
                human_size(new.bytes())
            )),
            _ => ctx
                .reporter
                .info("growing the data partition (sunxi-part grow)"),
        }
        let output = ctx
            .exec_with(commands::SUNXI_PART_GROW, None, ctx.options.flash_timeout)
            .await?;
        for line in output
            .stdout_lossy()
            .lines()
            .chain(output.stderr_lossy().lines())
        {
            if !line.trim().is_empty() {
                ctx.reporter.debug(format!("sunxi-part: {line}"));
            }
        }
        if !output.success() {
            ctx.reporter.warn(format!(
                "sunxi-part grow exited with {:?}; continuing as hakchi2 CE does",
                output.exit_code
            ));
        }
        let now = partitions(ctx).await?;
        if has_udisk(&now) {
            ctx.reporter
                .warn("UDISK is still listed: the partition table did not change");
            return Ok(Outcome::NotNeeded(
                "sunxi-part grow did not change the partition table".to_string(),
            ));
        }
        if let Some(data) = data_partition(&now) {
            ctx.reporter.info(format!(
                "data partition {} is now {}",
                data.device,
                human_size(data.bytes())
            ));
        }
        ctx.grew = true;
        Ok(Outcome::Done)
    }
}

/// After a grow the kernel still has the old partition table, so hakchi2 CE memboots again
/// before installing: `GrowTask` is built with `rebootToRecovery`, so its `Reboot` task runs
/// `MembootTasks.Memboot`, the same `detached-fallback` kexec of the package's `boot.img` as
/// a memboot from an installed kernel, not `ShellTasks.Reboot`. Skipped when nothing was
/// grown in this run, and not needed on the Mega Drive Mini (`sntool ismd`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RebootAfterGrow;

#[async_trait]
impl Step<Context> for RebootAfterGrow {
    fn name(&self) -> &str {
        "reboot-grown"
    }

    fn description(&self) -> &str {
        "Reboot into the memboot shell with the new partition table"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        Ok((!ctx.grew).then(|| "the partition table did not change in this run".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.succeeds(commands::SNTOOL_ISMD).await? {
            ctx.grew = false;
            return Ok(Outcome::NotNeeded(
                "Mega Drive Mini: hakchi2 CE skips this reboot".to_string(),
            ));
        }
        let image = ctx.hmod()?.files.boot_image.clone();
        kexec_memboot(ctx, Some(&image)).await?;
        ctx.grew = false;
        Ok(Outcome::Done)
    }
}
