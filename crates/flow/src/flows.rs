//! The flows: which steps, in which order. The sequences are hakchi2 CE's
//! (`MembootTasks.cs`: `InstallHakchi`, `Memboot`, `UninstallHakchi`, `DumpStockKernel`).

use std::path::PathBuf;

use crate::{
    Context, Flow,
    steps::{
        CheckMegaDrive, DetectState, DumpStockKernel, FlashStockKernel, GrowPartition,
        InstallBaseMods, InstallMods, Memboot, PrepareInstall, ReadStockKernel, Reboot,
        RebootAfterGrow, RemoveInstall, RunInstaller, UploadHmod, VerifyBaseMods, VerifyInstall,
        VerifyMods, WaitForConsole, WaitOffline,
    },
};

/// Name of the install flow.
pub const INSTALL: &str = "install";
/// Name of the memboot flow.
pub const MEMBOOT: &str = "memboot";
/// Name of the uninstall flow.
pub const UNINSTALL: &str = "uninstall";
/// Name of the dump flow.
pub const DUMP_KERNEL: &str = "dump-kernel";
/// Name of the base mods flow.
pub const BASE_MODS: &str = "base-mods";
/// Name of [`install_mods`].
pub const MODS: &str = "mods";

/// Install (or repair) the hakchi kernel. Needs the package in the context.
///
/// memboot, wait, detect, Mega Drive check, grow, reboot if grown, wait, detect, prepare,
/// upload, install, wait for the reboot, verify.
pub fn install_kernel() -> Flow<Context> {
    Flow::new(INSTALL)
        .step(Memboot::new())
        .step(WaitForConsole)
        .step(DetectState::expect_memboot_shell())
        .step(CheckMegaDrive)
        .step(GrowPartition)
        .step(RebootAfterGrow)
        .step(WaitForConsole)
        .step(DetectState::expect_memboot_shell())
        .step(PrepareInstall)
        .step(UploadHmod)
        .step(RunInstaller)
        .step(WaitForConsole)
        .step(VerifyInstall)
}

/// [`install_kernel`] followed by the base mods: once the installed kernel is verified, upload
/// and install the mods in the context (`tiny7zx` for compressed games), reboot, and check
/// them. Needs the mods in the context ([`Context::with_base_mods`]).
pub fn install_kernel_with_base_mods() -> Flow<Context> {
    install_kernel()
        .step(InstallBaseMods)
        .step(WaitForConsole)
        .step(DetectState::any())
        .step(VerifyBaseMods)
}

/// Install hakchi2 CE's base mods on a console already running its hakchi kernel: identify
/// it, upload and `hakchi packs_install` the mods in the context, reboot, verify. Nothing
/// touches the kernel partitions.
pub fn install_base_mods() -> Flow<Context> {
    Flow::new(BASE_MODS)
        .step(WaitForConsole)
        .step(DetectState::expect_installed())
        .step(InstallBaseMods)
        .step(WaitForConsole)
        .step(DetectState::any())
        .step(VerifyBaseMods)
}

/// Install emulator modules from the Mod Hub (RetroArch and cores) on a console running its
/// hakchi kernel: identify it, upload and `hakchi packs_install` the modules in the context
/// ([`Context::with_mods`]), reboot, verify. Nothing touches the kernel partitions.
pub fn install_mods() -> Flow<Context> {
    Flow::new(MODS)
        .step(WaitForConsole)
        .step(DetectState::expect_installed())
        .step(InstallMods)
        .step(WaitForConsole)
        .step(DetectState::any())
        .step(VerifyMods)
}

/// Memboot only, for recovery: the console ends up in the memboot shell with SSH open.
pub fn memboot() -> Flow<Context> {
    Flow::new(MEMBOOT)
        .step(Memboot::new())
        .step(WaitForConsole)
        .step(DetectState::any())
}

/// Return the console to the stock firmware. The package is needed only when the console is
/// not running a hakchi kernel (FEL memboot).
pub fn uninstall_kernel() -> Flow<Context> {
    Flow::new(UNINSTALL)
        .step(Memboot::allowing_nand_kernel())
        .step(WaitForConsole)
        .step(DetectState::expect_memboot_shell())
        .step(ReadStockKernel)
        .step(FlashStockKernel)
        .step(RemoveInstall)
        .step(Reboot)
        .step(WaitOffline)
}

/// Write the stock kernel backup to `path` and reboot. Same package rule as
/// [`uninstall_kernel`].
pub fn dump_stock_kernel(path: impl Into<PathBuf>) -> Flow<Context> {
    Flow::new(DUMP_KERNEL)
        .step(Memboot::allowing_nand_kernel())
        .step(WaitForConsole)
        .step(DetectState::expect_memboot_shell())
        .step(DumpStockKernel::new(path))
        .step(Reboot)
        .step(WaitOffline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flows_have_the_ce_sequences() {
        let names =
            |flow: Flow<Context>| flow.steps().into_iter().map(|s| s.name).collect::<Vec<_>>();
        assert_eq!(
            names(install_kernel()),
            [
                "memboot",
                "wait-console",
                "detect",
                "check-md",
                "grow",
                "reboot-grown",
                "wait-console",
                "detect",
                "prepare",
                "upload",
                "install",
                "wait-console",
                "verify",
            ]
        );
        assert_eq!(names(memboot()), ["memboot", "wait-console", "detect"]);
        let with_mods = names(install_kernel_with_base_mods());
        assert_eq!(with_mods.len(), 17);
        assert_eq!(
            &with_mods[13..],
            ["base-mods", "wait-console", "detect", "verify-mods"]
        );
        assert_eq!(
            names(install_base_mods()),
            [
                "wait-console",
                "detect",
                "base-mods",
                "wait-console",
                "detect",
                "verify-mods",
            ]
        );
        assert_eq!(
            names(install_mods()),
            [
                "wait-console",
                "detect",
                "mods",
                "wait-console",
                "detect",
                "verify-emulators",
            ]
        );
        assert_eq!(
            names(uninstall_kernel()),
            [
                "memboot",
                "wait-console",
                "detect",
                "read-stock-kernel",
                "flash-stock-kernel",
                "remove",
                "reboot",
                "wait-offline",
            ]
        );
        assert_eq!(
            names(dump_stock_kernel("kernel.img")),
            [
                "memboot",
                "wait-console",
                "detect",
                "dump-stock-kernel",
                "reboot",
                "wait-offline",
            ]
        );
        for flow in [
            install_kernel(),
            memboot(),
            uninstall_kernel(),
            install_base_mods(),
            install_mods(),
        ] {
            for step in flow.steps() {
                assert!(
                    !step.description.is_empty(),
                    "{} has a description",
                    step.name
                );
            }
        }
    }
}
