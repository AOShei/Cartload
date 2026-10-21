//! The hardware steps the flows are built from. Each one is small, describes itself, and
//! checks whether its work is already done so that a flow can be resumed.
//!
//! The exact command strings sent to the console live in [`commands`]; the tests replay them
//! against a simulated console.

mod detect;
mod grow;
mod install;
mod kernel;
mod md;
mod memboot;
mod mods;
mod reboot;
mod verify;
mod wait;

use std::time::Duration;

use cartload_console::parse::sh_quote;

pub use detect::{DetectState, Expect};
pub use grow::{GrowPartition, RebootAfterGrow};
pub use install::{PrepareInstall, RunInstaller, UploadHmod};
pub use kernel::{DumpStockKernel, FlashStockKernel, ReadStockKernel, RemoveInstall};
pub use md::CheckMegaDrive;
pub use memboot::Memboot;
pub use mods::{InstallBaseMods, VerifyBaseMods};
pub use reboot::Reboot;
pub use verify::VerifyInstall;
pub use wait::{WaitForConsole, WaitOffline};

use crate::{Context, Error, Result, device_failure};

/// The command strings the steps send, beyond the ones in
/// [`cartload_console::commands`]. They are what hakchi2 CE sends in `MembootTasks.cs`,
/// `GrowTask.cs` and `ModTasks.cs`, except for the kexec launcher, which replaces CE's
/// `detached-fallback` helper with the device's own `recovery` function.
pub mod commands {
    use cartload_console::parse::sh_quote;

    /// Whether the running kernel can kexec another one (hakchi2 CE's "membootable" test).
    pub const HAS_ATAGS: &str = "[ -f /proc/atags ]";
    /// Scratch directory for uploads on the console's tmpfs.
    pub const SCRATCH_DIR: &str = "/tmp/cartload";
    /// Creates the scratch directory.
    pub const MKDIR_SCRATCH: &str = "mkdir -p /tmp/cartload";
    /// Where a boot image for kexec is uploaded.
    pub const KEXEC_IMAGE: &str = "/tmp/cartload/boot.img";
    /// Where the kexec launcher script is uploaded.
    pub const KEXEC_SCRIPT: &str = "/tmp/cartload/kexec.sh";
    /// The kexec launcher: ignores the signals the dying SSH session sends, detaches from its
    /// pipes (a write to a closed pipe would kill the script before `kexec -e`), and runs the
    /// device's `recovery` function, which kexecs the image with `hakchi-shell` on the
    /// command line so the console comes back in the memboot shell.
    pub const KEXEC_SCRIPT_BODY: &str = "#!/bin/sh\n\
        # Written by Cartload: run the hakchi recovery kexec detached from the SSH session.\n\
        trap '' HUP TERM PIPE\n\
        exec </dev/null >/dev/null 2>&1\n\
        exec hakchi recovery \"$@\"\n";

    /// Starts the launcher in the background and returns at once.
    pub fn launch_kexec(image: Option<&str>) -> String {
        let image = image
            .map(|path| format!(" {}", sh_quote(path)))
            .unwrap_or_default();
        format!("sh {KEXEC_SCRIPT}{image} </dev/null >/dev/null 2>&1 &")
    }

    /// Whether the install flags and the transfer directory are in place.
    pub const INSTALL_FLAGGED: &str = r#"source /hakchi/config; [ "$cf_install" = "y" ] && [ "$cf_update" = "y" ] && [ -d /hakchi/transfer ]"#;
    /// hakchi2 CE `HandleHakchi(Install)`, first command.
    pub const FLAG_INSTALL: &str = r#"echo "cf_install=y" >> /hakchi/config"#;
    /// hakchi2 CE `HandleHakchi(Install)`, second command.
    pub const FLAG_UPDATE: &str = r#"echo "cf_update=y" >> /hakchi/config"#;
    /// hakchi2 CE `HandleHakchi(Install)`, third command.
    pub const MKDIR_TRANSFER: &str = "mkdir -p /hakchi/transfer/";
    /// Where the kernel package goes for the device-side installer.
    pub const HMOD_PATH: &str = "/hakchi/transfer/hakchi.hmod";
    /// The device-side installer (`/hakchi/shell/boot`): continues the boot with the
    /// transfer directory present, which installs hakchi, then reboots.
    pub const BOOT: &str = "boot";
    /// Lines in the installer output that mean it failed (`flashBoot` / `flashBoot0` in
    /// hakchi's `b0050_boot`). hakchi2 CE checks only the first. `invalid image on nand at
    /// sector: 68` is NOT a failure: `haveBackup2` prints it on stderr while deciding whether
    /// the stock kernel still needs backing up (seen on a real install, 2026-10-21).
    pub const FAILURE_MARKERS: [&str; 2] = ["flash md5 mismatch!", "flash boot0 failed!"];

    /// Lists the NAND partition table.
    pub const SUNXI_PART: &str = "sunxi-part";
    /// Shows what `sunxi-part grow` would do.
    pub const SUNXI_PART_GROW_DRY_RUN: &str = "sunxi-part grow --dry-run";
    /// Merges the unused UDISK partition into the data partition.
    pub const SUNXI_PART_GROW: &str = "sunxi-part grow";
    /// Exit code 0 on a Mega Drive Mini.
    pub const SNTOOL_ISMD: &str = "sntool ismd";

    /// Writes the stock kernel backup to stdout.
    pub const GET_BACKUP2: &str = "hakchi getBackup2";
    /// Where the stock kernel is uploaded for flashing.
    pub const KERNEL_PATH: &str = "/kernel.img";
    /// Validates the uploaded kernel image.
    pub const SNTOOL_CHECK_KERNEL: &str = "sntool check /kernel.img";
    /// Prints information about the uploaded kernel image.
    pub const SNTOOL_KERNEL_INFO: &str = "sntool kernel /kernel.img";
    /// Flashes the uploaded kernel image to the kernel partition with an MD5 verify.
    pub const FLASH_BOOT2: &str = "hakchi flashBoot2 /kernel.img";
    /// Exit code 0 when the kernel partition holds an untouched stock kernel.
    pub const IS_ORIGINAL_BOOT2: &str = "hakchi isOriginalBoot2";
    /// Removes the hakchi files, restores the stock u-boot and shrinks the data partition.
    pub const MOD_UNINSTALL: &str = "hakchi mod_uninstall";
    /// Whether hakchi's files exist on the mounted data partition (memboot shell paths).
    pub const INSTALL_DIR_EXISTS: &str = "[ -d /newroot/var/lib/hakchi ]";
    /// hakchi2 CE `ShellTasks.Reboot`.
    pub const REBOOT: &str = "sync; umount -ar; reboot -f";

    /// `md5sum <path>`.
    pub fn md5sum(path: &str) -> String {
        format!("md5sum {}", sh_quote(path))
    }

    /// `md5sum` of several files at once.
    pub fn md5sum_all(paths: &[String]) -> String {
        let quoted: Vec<String> = paths.iter().map(|p| sh_quote(p)).collect();
        format!("md5sum {}", quoted.join(" "))
    }

    /// Where base mods are unpacked before `hakchi packs_install` (hakchi2 CE uses
    /// `/tmp/hmods`).
    pub const MODS_DIR: &str = "/tmp/cartload/hmods";
    /// Creates the mods directory.
    pub const MKDIR_MODS: &str = "mkdir -p '/tmp/cartload/hmods'";
    /// Creates the directory of one mod (`<name>.hmod`, the directory form `pack_install`
    /// takes).
    pub fn mkdir_mod(name: &str) -> String {
        format!("mkdir -p {}", sh_quote(&format!("{MODS_DIR}/{name}.hmod")))
    }
    /// Unpacks the mod's tar stream (on stdin) into its directory, as hakchi2 CE's
    /// `ModTasks.TransferHmod` does.
    pub fn extract_mod(name: &str) -> String {
        format!("tar -xvC {}", sh_quote(&format!("{MODS_DIR}/{name}.hmod")))
    }
    /// Installs every `*.hmod` in the mods directory (hakchi2 CE `ModTasks.InstallHmods`).
    pub const PACKS_INSTALL: &str = "hakchi packs_install '/tmp/cartload/hmods'";
    /// Exit code 0 when the device registered the mod (`pack_install` wrote its uninstall
    /// script).
    pub fn mod_registered(installpath: &str, name: &str) -> String {
        format!(
            "[ -f {} ]",
            sh_quote(&format!("{installpath}/hmod/uninstall-{name}"))
        )
    }
    /// Exit code 0 (and the path on stdout) when `binary` is on the console's `PATH`.
    pub fn which(binary: &str) -> String {
        format!("which {}", sh_quote(binary))
    }

    /// hakchi2 CE `CheckMdRootfs`: succeeds when the Mega Drive Mini's system partition has
    /// no Project Lunar init scripts.
    pub fn project_lunar_check(squashfs: &str) -> String {
        format!(
            r#"[ "$(find {} -name "*_PL_*" | wc -l)" = "0" ]"#,
            sh_quote(&format!("{squashfs}/etc/init.d"))
        )
    }
}

/// Upload chunk size: one SSH exec (`cat >` then `cat >>`) per chunk, so that progress can
/// be reported and no single command runs for minutes.
pub const UPLOAD_CHUNK: usize = 1024 * 1024;

/// Writes `data` to `path` on the console through `cat`, reporting progress under `label`.
pub(crate) async fn upload(ctx: &Context, path: &str, data: &[u8], label: &str) -> Result<()> {
    let quoted = sh_quote(path);
    let total = data.len() as u64;
    ctx.reporter.progress(label, 0, total);
    let mut done = 0usize;
    loop {
        let end = (done + UPLOAD_CHUNK).min(data.len());
        let command = if done == 0 {
            format!("cat > {quoted}")
        } else {
            format!("cat >> {quoted}")
        };
        let output = ctx
            .exec_with(&command, Some(&data[done..end]), ctx.options.flash_timeout)
            .await?;
        if !output.success() {
            return Err(device_failure(&command, &output));
        }
        done = end;
        ctx.reporter.progress(label, done as u64, total);
        if done >= data.len() {
            return Ok(());
        }
    }
}

/// MD5 of a file on the console, or `None` when it does not exist.
pub(crate) async fn remote_md5(ctx: &Context, path: &str) -> Result<Option<String>> {
    let command = commands::md5sum(path);
    let output = ctx
        .exec_with(&command, None, ctx.options.flash_timeout)
        .await?;
    if !output.success() {
        return Ok(None);
    }
    Ok(output
        .stdout_trimmed()
        .split_whitespace()
        .next()
        .map(str::to_string))
}

/// MD5s of several files on the console, keyed by path, or `None` when any is missing.
pub(crate) async fn remote_md5_list(
    ctx: &Context,
    paths: &[String],
) -> Result<Option<std::collections::BTreeMap<String, String>>> {
    if paths.is_empty() {
        return Ok(Some(std::collections::BTreeMap::new()));
    }
    let command = commands::md5sum_all(paths);
    let output = ctx
        .exec_with(&command, None, ctx.options.flash_timeout)
        .await?;
    if !output.success() {
        return Ok(None);
    }
    let mut sums = std::collections::BTreeMap::new();
    for line in output.stdout_lossy().lines() {
        if let Some((md5, path)) = line.trim_end().split_once("  ") {
            sums.insert(path.to_string(), md5.to_string());
        }
    }
    Ok(Some(sums))
}

/// Memboots `image` (or, with `None`, the kernel in NAND) from a running hakchi kernel.
///
/// hakchi2 CE uploads its own `kexec` and a `detached-fallback` script for this; the device's
/// ramdisk and rootfs carry the same `kexec` and a `recovery` shell function, so Cartload
/// uploads only a four-line launcher (see [`commands::KEXEC_SCRIPT_BODY`]). The console drops
/// off the network and comes back in the memboot shell.
pub(crate) async fn kexec_memboot(ctx: &mut Context, image: Option<&[u8]>) -> Result<()> {
    if !ctx.succeeds(commands::HAS_ATAGS).await? {
        return Err(Error::WrongState(
            "the running kernel cannot kexec another one (no /proc/atags); power the console \
             off and use FEL mode instead: hold Reset while powering on"
                .to_string(),
        ));
    }
    ctx.exec_checked(commands::MKDIR_SCRATCH, ctx.options.command_timeout)
        .await?;
    let image_path = match image {
        Some(image) => {
            ctx.reporter.info(format!(
                "uploading the boot image for kexec ({} bytes)",
                image.len()
            ));
            upload(ctx, commands::KEXEC_IMAGE, image, "boot image").await?;
            Some(commands::KEXEC_IMAGE)
        }
        None => None,
    };
    upload(
        ctx,
        commands::KEXEC_SCRIPT,
        commands::KEXEC_SCRIPT_BODY.as_bytes(),
        "kexec launcher",
    )
    .await?;
    ctx.reporter
        .info("kexec: the console drops off the network now and comes back in the memboot shell");
    let launch = commands::launch_kexec(image_path);
    if let Err(error) = ctx.exec_with(&launch, None, Duration::from_secs(10)).await {
        ctx.reporter.debug(format!(
            "kexec launch returned {error} (expected if the link dropped)"
        ));
    }
    ctx.expect_cycle = true;
    ctx.disconnect().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kexec_launcher_quotes_the_image_and_backgrounds_the_script() {
        assert_eq!(
            commands::launch_kexec(Some("/tmp/cartload/boot.img")),
            "sh /tmp/cartload/kexec.sh '/tmp/cartload/boot.img' </dev/null >/dev/null 2>&1 &"
        );
        assert_eq!(
            commands::launch_kexec(None),
            "sh /tmp/cartload/kexec.sh </dev/null >/dev/null 2>&1 &"
        );
        assert!(commands::KEXEC_SCRIPT_BODY.contains("trap '' HUP TERM PIPE"));
        assert!(commands::KEXEC_SCRIPT_BODY.ends_with("exec hakchi recovery \"$@\"\n"));
    }

    #[test]
    fn mod_commands_quote_their_paths() {
        assert_eq!(
            commands::mkdir_mod("tiny7zx"),
            "mkdir -p '/tmp/cartload/hmods/tiny7zx.hmod'"
        );
        assert_eq!(
            commands::extract_mod("tiny7zx"),
            "tar -xvC '/tmp/cartload/hmods/tiny7zx.hmod'"
        );
        assert_eq!(
            commands::mod_registered("/var/lib/hakchi", "tiny7zx"),
            "[ -f '/var/lib/hakchi/hmod/uninstall-tiny7zx' ]"
        );
        assert_eq!(commands::which("tiny7zx"), "which 'tiny7zx'");
        assert_eq!(
            commands::md5sum_all(&["/a b".to_string(), "/c".to_string()]),
            "md5sum '/a b' '/c'"
        );
    }

    #[test]
    fn project_lunar_check_matches_ce() {
        assert_eq!(
            commands::project_lunar_check("/var/squashfs"),
            r#"[ "$(find '/var/squashfs/etc/init.d' -name "*_PL_*" | wc -l)" = "0" ]"#
        );
    }
}
