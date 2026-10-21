//! Reboot the console.

use std::time::Duration;

use async_trait::async_trait;

use crate::{Context, Outcome, Result, Step, steps::commands};

/// `sync; umount -ar; reboot -f` (hakchi2 CE `ShellTasks.Reboot`). The connection drops, so
/// the command's result is ignored.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reboot;

#[async_trait]
impl Step<Context> for Reboot {
    fn name(&self) -> &str {
        "reboot"
    }

    fn description(&self) -> &str {
        "Reboot the console (sync; umount -ar; reboot -f)"
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        ctx.reporter.info("rebooting the console");
        if let Err(error) = ctx
            .exec_with(commands::REBOOT, None, Duration::from_secs(10))
            .await
        {
            ctx.reporter
                .debug(format!("reboot command returned {error} (expected)"));
        }
        ctx.expect_cycle = true;
        ctx.disconnect().await;
        Ok(Outcome::Done)
    }
}
