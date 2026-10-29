//! Verify the install after the reboot.

use async_trait::async_trait;

use crate::{Context, Error, Outcome, Result, Step};

/// Reconnects, runs the identification sequence and checks that the console booted the
/// installed system with the package's versions.
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifyInstall;

#[async_trait]
impl Step<Context> for VerifyInstall {
    fn name(&self) -> &str {
        "verify"
    }

    fn description(&self) -> &str {
        "Check the installed kernel reports the package's versions"
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        ctx.refresh_state().await?;
        let description = ctx.state.describe();
        let Some(info) = ctx.state.info() else {
            return Err(Error::WrongState(format!(
                "{description}; the console did not come back after the install"
            )));
        };
        if info.minimal_memboot {
            return Err(Error::Device(format!(
                "the console came back in the memboot shell instead of booting the installed \
                 system ({description}); the installer did not complete, see its output above"
            )));
        }
        let Some(versions) = &info.versions else {
            return Err(Error::Device(format!(
                "the installed system has no /var/version ({description})"
            )));
        };
        if let Some(hmod) = &ctx.hmod {
            let expected = &hmod.files.version;
            let mismatches: Vec<String> = [
                (
                    "script",
                    versions.script.as_str(),
                    Some(expected.hakchi.as_str()),
                ),
                // Compared whenever either side has it: hakchi's package over Cartload's
                // system leaves no cartloadVersion, and the other way round.
                (
                    "Cartload system",
                    versions.system.as_deref().unwrap_or("none"),
                    (expected.cartload.is_some() || versions.system.is_some())
                        .then(|| expected.cartload.as_deref().unwrap_or("none")),
                ),
                ("boot", versions.boot.as_str(), expected.boot.as_deref()),
                (
                    "kernel",
                    versions.kernel.as_str(),
                    expected.kernel.as_deref(),
                ),
            ]
            .into_iter()
            .filter(|(_, got, want)| want.is_some_and(|want| want != *got))
            .map(|(what, got, want)| format!("{what} {got} (package {})", want.unwrap_or_default()))
            .collect();
            if !mismatches.is_empty() {
                return Err(Error::Device(format!(
                    "the installed versions differ from the package: {}",
                    mismatches.join(", ")
                )));
            }
        }
        if !info.can_interact {
            return Err(Error::Device(format!(
                "the installed system is too old to use: {description}"
            )));
        }
        ctx.reporter.info(format!("installed: {description}"));
        if let Some(storage) = &info.storage {
            ctx.reporter
                .info(format!("games storage {}: {}", storage.path, storage.usage));
        }
        Ok(Outcome::Done)
    }
}
