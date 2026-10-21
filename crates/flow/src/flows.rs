//! The flows: which steps, in which order. The sequences are hakchi2 CE's
//! (`MembootTasks.cs`: `InstallHakchi`, `Memboot`, `UninstallHakchi`, `DumpStockKernel`).

use std::path::PathBuf;

use crate::{
    Context, Flow,
    steps::{
        CheckMegaDrive, DetectState, DumpStockKernel, FlashStockKernel, GrowPartition, Memboot,
        PrepareInstall, ReadStockKernel, Reboot, RebootAfterGrow, RemoveInstall, RunInstaller,
        UploadHmod, VerifyInstall, WaitForConsole, WaitOffline,
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
        for flow in [install_kernel(), memboot(), uninstall_kernel()] {
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
