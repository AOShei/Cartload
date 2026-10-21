//! Install hakchi2 CE's base mods on a console running its installed hakchi kernel, the way
//! CE's `ModTasks` does it: each mod is sent as a tar stream into a directory under `/tmp`
//! (`tar -xvC <dir>`), then `hakchi packs_install <dir>` runs the device's `pack_install` on
//! every `*.hmod` there (copies the files into the hakchi rootfs overlay, sources the mod's
//! `install` script, writes `<installpath>/hmod/uninstall-<name>`). The rootfs is an
//! overlayfs upper directory, so the console is rebooted afterwards for the new files to be
//! seen reliably (CE reboots too: `BootHakchi` ends its mod install).

use std::collections::BTreeMap;

use async_trait::async_trait;
use cartload_hmod::basemods::{LoadedMod, md5_hex};

use crate::{
    Context, Error, Outcome, Result, Step, device_failure,
    steps::{commands, remote_md5_list},
};

/// Default hakchi rootfs overlay when the console did not report one.
pub const DEFAULT_ROOTFS: &str = "/var/lib/hakchi/rootfs";

/// Paths of the hakchi install on the console, as the mod steps need them.
fn hakchi_paths(ctx: &Context) -> (String, String) {
    let rootfs = ctx
        .state
        .info()
        .and_then(|info| info.rootfs_path.clone())
        .unwrap_or_else(|| DEFAULT_ROOTFS.to_string());
    let installpath = rootfs
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_else(|| "/var/lib/hakchi".to_string());
    (rootfs, installpath)
}

/// Whether a mod's payload files are in the rootfs with the uploaded content's MD5s and the
/// mod is registered. `Ok(Some(reason))` when installed.
async fn installed(ctx: &Context, m: &LoadedMod) -> Result<Option<String>> {
    let (rootfs, installpath) = hakchi_paths(ctx);
    let expected: BTreeMap<String, String> = m
        .spec
        .payload()
        .filter_map(|file| {
            m.files
                .iter()
                .find(|(path, _)| path == file.path)
                .map(|(_, data)| (format!("{rootfs}/{}", file.path), md5_hex(data)))
        })
        .collect();
    let paths: Vec<String> = expected.keys().cloned().collect();
    let Some(actual) = remote_md5_list(ctx, &paths).await? else {
        return Ok(None);
    };
    for (path, md5) in &expected {
        if actual.get(path) != Some(md5) {
            return Ok(None);
        }
    }
    let registered = ctx
        .succeeds(&commands::mod_registered(&installpath, m.name()))
        .await?;
    if !registered {
        return Ok(None);
    }
    Ok(Some(format!(
        "{}: {} file(s) in {rootfs} with the expected MD5, registered in {installpath}/hmod",
        m.name(),
        expected.len()
    )))
}

/// Uploads the loaded base mods, runs `hakchi packs_install`, checks the files landed in the
/// rootfs and reboots the console.
#[derive(Clone, Copy, Debug, Default)]
pub struct InstallBaseMods;

#[async_trait]
impl Step<Context> for InstallBaseMods {
    fn name(&self) -> &str {
        "base-mods"
    }

    fn description(&self) -> &str {
        "Upload and install hakchi2 CE's base mods, then reboot"
    }

    async fn already_done(&self, ctx: &mut Context) -> Result<Option<String>> {
        if ctx.base_mods.is_empty() {
            return Ok(None);
        }
        let mut reasons = Vec::new();
        for m in &ctx.base_mods {
            match installed(ctx, m).await? {
                Some(reason) => reasons.push(reason),
                None => return Ok(None),
            }
        }
        Ok(Some(reasons.join("; ")))
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.base_mods.is_empty() {
            return Err(Error::Precondition(
                "no base mods were loaded; fetch them first (cartload-hmod base-mods --fetch) \
                 or pass --mods"
                    .to_string(),
            ));
        }
        if !ctx.state.is_installed() || ctx.state.is_memboot_shell() {
            return Err(Error::WrongState(format!(
                "base mods are installed on a console booted into its hakchi kernel, not \
                 {}",
                ctx.state.describe()
            )));
        }
        let mods = ctx.base_mods.clone();
        ctx.exec_checked(commands::MKDIR_MODS, ctx.options.command_timeout)
            .await?;
        for m in &mods {
            let mkdir = commands::mkdir_mod(m.name());
            ctx.exec_checked(&mkdir, ctx.options.command_timeout)
                .await?;
            let extract = commands::extract_mod(m.name());
            ctx.reporter.progress(m.name(), 0, m.tar.len() as u64);
            let output = ctx
                .exec_with(&extract, Some(&m.tar), ctx.options.flash_timeout)
                .await?;
            if !output.success() {
                return Err(device_failure(&extract, &output));
            }
            ctx.reporter
                .progress(m.name(), m.tar.len() as u64, m.tar.len() as u64);
            ctx.reporter.info(format!(
                "uploaded {}.hmod ({} files, {} bytes)",
                m.name(),
                m.files.len(),
                m.len()
            ));
        }
        let output = ctx
            .exec_with(commands::PACKS_INSTALL, None, ctx.options.flash_timeout)
            .await?;
        for line in output
            .stdout_lossy()
            .lines()
            .chain(output.stderr_lossy().lines())
        {
            let line = line.trim_end();
            if !line.is_empty() {
                ctx.reporter.info(format!("console: {line}"));
            }
        }
        if !output.success() {
            return Err(device_failure(commands::PACKS_INSTALL, &output));
        }
        for m in &mods {
            if installed(ctx, m).await?.is_none() {
                return Err(Error::Device(format!(
                    "{}.hmod was installed but its files are not in the hakchi rootfs with \
                     the expected MD5s (or it is not registered); see the installer output \
                     above",
                    m.name()
                )));
            }
        }
        ctx.reporter.info(format!(
            "installed {}; rebooting the console so the new files are seen",
            mods.iter()
                .map(LoadedMod::name)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if let Err(error) = ctx
            .exec_with(commands::REBOOT, None, std::time::Duration::from_secs(10))
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

/// After the reboot: the mods' files are in place and every binary they add is found on the
/// console's `PATH` (what the emulator wrappers rely on for `tiny7zx`).
#[derive(Clone, Copy, Debug, Default)]
pub struct VerifyBaseMods;

#[async_trait]
impl Step<Context> for VerifyBaseMods {
    fn name(&self) -> &str {
        "verify-mods"
    }

    fn description(&self) -> &str {
        "Check the base mods are installed"
    }

    async fn run(&self, ctx: &mut Context) -> Result<Outcome> {
        if ctx.base_mods.is_empty() {
            return Ok(Outcome::NotNeeded("no base mods were loaded".to_string()));
        }
        if ctx.console.is_none() {
            ctx.refresh_state().await?;
        }
        let mods = ctx.base_mods.clone();
        for m in &mods {
            let Some(reason) = installed(ctx, m).await? else {
                return Err(Error::Device(format!(
                    "{}.hmod is not installed on the console; rerun the command",
                    m.name()
                )));
            };
            ctx.reporter.info(reason);
            for file in m.spec.payload() {
                let Some(binary) = file.path.strip_prefix("bin/") else {
                    continue;
                };
                let which = commands::which(binary);
                let output = ctx.exec(&which).await?;
                if !output.success() {
                    return Err(Error::Device(format!(
                        "`{which}` found nothing although {}.hmod is installed; reboot the \
                         console and rerun the command",
                        m.name()
                    )));
                }
                ctx.reporter.info(format!(
                    "{binary} is on the console's PATH at {}",
                    output.stdout_trimmed()
                ));
            }
        }
        Ok(Outcome::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hakchi_paths_default_without_a_console() {
        let ctx = Context::new(
            Box::new(crate::UsbBackend::default()),
            crate::Reporter::silent(),
            crate::FlowOptions::default(),
        );
        assert_eq!(
            hakchi_paths(&ctx),
            (
                "/var/lib/hakchi/rootfs".to_string(),
                "/var/lib/hakchi".to_string()
            )
        );
    }
}
