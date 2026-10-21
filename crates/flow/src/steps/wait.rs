//! Waiting for the console to appear on, or leave, the USB network.

use std::time::Duration;

use async_trait::async_trait;

use crate::{ConsoleState, Context, Error, Outcome, Result, Step};

/// Waits for the console to answer SSH on the USB network link.
///
/// After a reboot or kexec ([`Context::expect_cycle`]) it first waits for the console to
/// drop off the network, so that a still-running old kernel is not mistaken for the new one
/// (hakchi2 CE's `WaitForShellCycle`).
#[derive(Clone, Copy, Debug, Default)]
pub struct WaitForConsole;

#[async_trait]
impl Step<Context> for WaitForConsole {
    fn name(&self) -> &str {
        "wait-console"
    }

    fn description(&self) -> &str {
        "Wait for the console to come up on the USB network (169.254.13.37) and accept SSH"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        if ctx.expect_cycle {
            return Ok(None);
        }
        Ok(matches!(ctx.connect().await, Ok(true))
            .then(|| "the console is already reachable".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        let after_reboot = ctx.expect_cycle;
        if after_reboot {
            wait_offline(ctx, ctx.options.offline_wait).await?;
        }
        let (timeout, hint) = if after_reboot {
            (
                ctx.options.reboot_wait,
                "it did not come back after rebooting; check the console's screen and power, \
                 then rerun the command to resume",
            )
        } else {
            (
                ctx.options.console_wait,
                "check the USB cable and that FEL mode was entered (hold Reset while powering \
                 on); on Linux the hakchi0 interface needs a link-local address",
            )
        };
        ctx.reporter.info(format!(
            "waiting up to {} s for the console on the USB network",
            timeout.as_secs()
        ));
        wait_online(ctx, timeout, hint).await?;
        Ok(Outcome::Done)
    }
}

/// Waits for the console to leave the USB network, as it does when it reboots into the stock
/// firmware (which has no network gadget).
#[derive(Clone, Copy, Debug, Default)]
pub struct WaitOffline;

#[async_trait]
impl Step<Context> for WaitOffline {
    fn name(&self) -> &str {
        "wait-offline"
    }

    fn description(&self) -> &str {
        "Wait for the console to leave the USB network after the reboot"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        if ctx.expect_cycle {
            return Ok(None);
        }
        ctx.disconnect().await;
        Ok(ctx
            .probe()
            .await?
            .is_none()
            .then(|| "the console is already offline".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        wait_offline(ctx, ctx.options.offline_wait).await?;
        Ok(Outcome::Done)
    }
}

/// Polls the link until nothing answers.
pub(crate) async fn wait_offline(ctx: &mut Context, timeout: Duration) -> Result<()> {
    ctx.disconnect().await;
    let started = tokio::time::Instant::now();
    loop {
        if ctx.probe().await?.is_none() {
            ctx.reporter.info("the console left the USB network");
            ctx.expect_cycle = false;
            ctx.state = ConsoleState::Offline;
            return Ok(());
        }
        if started.elapsed() >= timeout {
            return Err(Error::Timeout {
                what: "the console leaving the USB network after the reboot command".to_string(),
                timeout,
                hint: "it probably did not reboot; check its screen, power-cycle it if needed, \
                       then rerun the command"
                    .to_string(),
            });
        }
        tokio::time::sleep(ctx.options.poll_interval).await;
    }
}

/// Polls the link until an SSH session opens. SSH failures while the console is still
/// booting are retried until the deadline.
pub(crate) async fn wait_online(ctx: &mut Context, timeout: Duration, hint: &str) -> Result<()> {
    let started = tokio::time::Instant::now();
    let mut last_error: Option<String> = None;
    loop {
        match ctx.connect().await {
            Ok(true) => {
                let address = ctx.console()?.address();
                ctx.reporter
                    .info(format!("the console answers SSH at {address}"));
                return Ok(());
            }
            Ok(false) => {}
            Err(error) => {
                ctx.reporter
                    .debug(format!("console not ready yet: {error}"));
                last_error = Some(error.to_string());
            }
        }
        if started.elapsed() >= timeout {
            let mut hint = hint.to_string();
            if let Some(error) = last_error {
                hint.push_str(&format!(" (last error: {error})"));
            }
            return Err(Error::Timeout {
                what: "the console reappearing on the USB network".to_string(),
                timeout,
                hint,
            });
        }
        tokio::time::sleep(ctx.options.poll_interval).await;
    }
}
