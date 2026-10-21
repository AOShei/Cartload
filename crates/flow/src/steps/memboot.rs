//! Memboot: run the hakchi kernel from RAM.

use std::time::Duration;

use async_trait::async_trait;

use crate::{ConsoleState, Context, Error, Outcome, Result, Step, steps::kexec_memboot};

/// Boots the hakchi kernel from RAM, the first step of every flow.
///
/// Like hakchi2 CE's `WaitForFelOrMembootableShell`: when a hakchi kernel is already running
/// (installed, booted normally) the kernel is kexec'd from it; otherwise the console must be
/// in FEL mode (hold Reset while powering on) and the kernel goes in through USB. Already in
/// the memboot shell: nothing to do.
#[derive(Clone, Copy, Debug, Default)]
pub struct Memboot {
    allow_nand_kernel: bool,
}

impl Memboot {
    /// Memboot that needs the package's `boot/boot.img` in every case.
    pub fn new() -> Self {
        Self::default()
    }

    /// Memboot that, from a running hakchi kernel without a package, kexecs the kernel that is
    /// already in NAND (enough for uninstalling or dumping; FEL still needs the package).
    pub fn allowing_nand_kernel() -> Self {
        Self {
            allow_nand_kernel: true,
        }
    }
}

#[async_trait]
impl Step<Context> for Memboot {
    fn name(&self) -> &str {
        "memboot"
    }

    fn description(&self) -> &str {
        "Boot the hakchi kernel from RAM: through FEL, or with kexec from a running hakchi kernel"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        ctx.refresh_state().await?;
        Ok(ctx
            .state
            .is_memboot_shell()
            .then(|| "the console is already in the memboot shell".to_string()))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.state.is_online() {
            ctx.reporter.info(format!("found {}", ctx.state.describe()));
            let image = ctx.hmod.as_ref().map(|hmod| hmod.files.boot_image.clone());
            match (image, self.allow_nand_kernel) {
                (Some(image), _) => {
                    ctx.reporter.info(
                        "membooting the package's boot.img with kexec from the running kernel",
                    );
                    kexec_memboot(ctx, Some(&image)).await?;
                }
                (None, true) => {
                    ctx.reporter
                        .info("membooting the kernel from NAND with kexec from the running kernel");
                    kexec_memboot(ctx, None).await?;
                }
                (None, false) => {
                    return Err(Error::Precondition(
                        "the console runs a hakchi kernel; membooting the package from it needs \
                         hakchi.hmod: pass --hmod <path>"
                            .to_string(),
                    ));
                }
            }
            return Ok(Outcome::Done);
        }
        fel_memboot(ctx).await?;
        Ok(Outcome::Done)
    }
}

async fn fel_memboot(ctx: &mut Context) -> Result<()> {
    ctx.hmod()?;
    let wait = ctx.options.fel_wait;
    ctx.reporter.info(format!(
        "waiting up to {} s for a console in FEL mode: hold Reset, switch Power on, release \
         Reset after two seconds",
        wait.as_secs()
    ));
    let started = tokio::time::Instant::now();
    loop {
        if ctx.backend.fel_device_present().await? {
            break;
        }
        if started.elapsed() >= wait {
            return Err(Error::Timeout {
                what: "a console in FEL mode (USB 1f3a:efe8) appearing".to_string(),
                timeout: wait,
                hint: "power the console off, hold Reset, switch Power on, keep Reset held for \
                       two seconds and release it; check the USB cable and, on Linux, the udev \
                       rule for 1f3a:efe8"
                    .to_string(),
            });
        }
        tokio::time::sleep(Duration::from_millis(500).min(ctx.options.poll_interval)).await;
    }
    let hmod = ctx.hmod()?;
    ctx.reporter.info(format!(
        "membooting hakchi {} through FEL",
        hmod.files.version
    ));
    let report = ctx
        .backend
        .fel_memboot(&hmod.files, &ctx.options.memboot, &ctx.reporter)
        .await?;
    ctx.reporter.info(format!(
        "uploaded {} bytes and ran `{}`; the console should enumerate as the hakchi network \
         gadget (04e8:6863) within about 10 s",
        report.image_len, report.command
    ));
    ctx.expect_cycle = false;
    ctx.state = ConsoleState::Offline;
    Ok(())
}
