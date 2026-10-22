//! Mega Drive Mini: refuse to install over Project Lunar.

use async_trait::async_trait;
use cartload_console::{Hakchi, parse::check_device_path};

use crate::{Context, Error, Outcome, Result, Step, steps::commands};

/// hakchi2 CE's `CheckMdRootfs`, reduced to its hard refusal: a Mega Drive Mini whose system
/// partition carries Project Lunar init scripts cannot take a hakchi install. CE also compares
/// the system files against known hashes and asks the user about mismatches; Cartload does
/// not (no prompts in a flow), and says so.
#[derive(Clone, Copy, Debug, Default)]
pub struct CheckMegaDrive;

#[async_trait]
impl Step<Context> for CheckMegaDrive {
    fn name(&self) -> &str {
        "check-md"
    }

    fn description(&self) -> &str {
        "Refuse to install on a Mega Drive Mini, which Cartload does not support"
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if !ctx.state.md_partitioning() {
            return Ok(Outcome::NotNeeded(
                "not a Mega Drive partition layout".to_string(),
            ));
        }
        let link = ctx.link()?;
        let hakchi = Hakchi::new(&link);
        hakchi.mount_base().await?;
        let clean = async {
            let squashfs = hakchi.get("squashfs").await?;
            check_device_path(&squashfs)?;
            ctx.succeeds(&commands::project_lunar_check(&squashfs))
                .await
        }
        .await;
        if let Err(error) = hakchi.umount_base().await {
            ctx.reporter.debug(format!("umount_base: {error}"));
        }
        if clean? {
            ctx.reporter.warn(
                "Mega Drive Mini: system file hashes are not checked (hakchi2 CE would ask \
                 about modified system files)",
            );
            Ok(Outcome::Done)
        } else {
            Err(Error::Precondition(
                "Project Lunar is installed on this Mega Drive Mini; hakchi cannot be installed \
                 over it (uninstall Project Lunar first)"
                    .to_string(),
            ))
        }
    }
}
