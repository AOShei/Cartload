//! Allwinner FEL USB boot protocol for the Classic Mini consoles.
//!
//! The NES, SNES and Mega Drive Classic Mini all use the Allwinner R16 SoC. Holding Reset while
//! powering the console on starts the SoC's boot ROM in FEL mode: it enumerates as USB
//! `1f3a:efe8` and accepts memory reads, writes and code execution over a pair of bulk
//! endpoints. Memboot uses that to run a hakchi kernel from RAM without touching the NAND:
//!
//! 1. [`Device::verify_device`] checks the SoC id.
//! 2. [`Device::init_dram`] uploads and runs `fes1.bin` so DRAM becomes usable.
//! 3. [`memboot()`] uploads `boot.img` (kernel plus ramdisk, Android boot format) to DRAM,
//!    uploads `uboot.bin` and runs `boota <address>` through u-boot, which starts the kernel.
//!
//! The protocol knowledge comes from hakchi2 CE (`FelLib`, `MembootTasks.cs`) and sunxi-tools.
//! Nothing here needs hardware to compile or test: [`Transport`] abstracts the USB pipe and
//! [`Device`] is generic over it.
//!
//! Wire formats live in [`message`]; the memboot recipe and image handling in [`mod@memboot`].

mod device;
mod error;
pub mod memboot;
pub mod message;
mod transport;

pub use device::{DRAM_INIT_DELAY, Device, FES1_CHECK_LEN, Progress, UBOOT_CHECK_LEN};
pub use error::{Error, Result};
pub use memboot::{
    MembootEvent, MembootOptions, MembootReport, MembootStage, ShellOverride, UbootImage,
    UbootVariant, memboot,
};
pub use message::VerifyDeviceResponse;
pub use nusb::DeviceInfo;
pub use transport::{
    DEFAULT_TIMEOUT, EXPECTED_ENDPOINTS, Transport, UsbTransport, find_devices, wait_for_device,
};

/// USB vendor id of the Allwinner boot ROM in FEL mode.
pub const USB_VID: u16 = 0x1f3a;
/// USB product id of the Allwinner boot ROM in FEL mode.
pub const USB_PID: u16 = 0xefe8;
/// SoC id the boot ROM reports on the Allwinner R16 (A33 family, "sun8iw5").
pub const SOC_ID_R16: u32 = 0x0016_6700;

/// SRAM address `fes1.bin` is loaded to and run from.
pub const FES1_BASE: u32 = 0x2000;
/// Start of DRAM once `fes1` has initialised it.
pub const DRAM_BASE: u32 = 0x4000_0000;
/// DRAM address u-boot is loaded to and run from.
pub const UBOOT_BASE: u32 = DRAM_BASE + 0x0700_0000;
/// DRAM staging area for boot images and flash transfers.
pub const TRANSFER_BASE: u32 = DRAM_BASE + 0x0740_0000;
/// NAND sector size; uploads are padded to whole sectors.
pub const SECTOR_SIZE: u32 = 0x2_0000;
/// Largest payload the staging area at [`TRANSFER_BASE`] takes (32 MiB).
pub const TRANSFER_MAX_SIZE: u32 = SECTOR_SIZE * 0x100;
/// hakchi2 CE's kernel size limit for FEL flashing (4 MiB). Not the limit on the console:
/// hakchi's u-boot loads up to 5 MiB (v1.0.4-126's image is 4.4 MiB); see
/// `cartload_hmod::BOOT_IMAGE_MAX_LEN`. Memboot images may be larger still.
pub const KERNEL_MAX_SIZE: u32 = SECTOR_SIZE * 0x20;
/// NAND offset of the first u-boot copy.
pub const UBOOT_FLASH_BASE: u32 = 0x10_0000;
/// Size of the u-boot area in NAND.
pub const UBOOT_FLASH_MAX_SIZE: u32 = SECTOR_SIZE * 0x10;
/// NAND offset of the kernel partition.
pub const KERNEL_FLASH_BASE: u32 = SECTOR_SIZE * 0x30;
/// Largest single FEL download or upload the boot ROM accepts.
pub const MAX_CHUNK: usize = 0x1_0000;

/// Crate name and version, for CLI banners and diagnostics.
pub const fn version() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn version_mentions_crate_name() {
        assert!(version().starts_with(env!("CARGO_PKG_NAME")));
    }

    #[test]
    fn addresses_match_hakchi2_ce() {
        assert_eq!(UBOOT_BASE, 0x4700_0000);
        assert_eq!(TRANSFER_BASE, 0x4740_0000);
        assert_eq!(TRANSFER_MAX_SIZE, 32 * 1024 * 1024);
        assert_eq!(KERNEL_MAX_SIZE, 4 * 1024 * 1024);
        assert_eq!(KERNEL_FLASH_BASE, 0x60_0000);
        assert_eq!(UBOOT_FLASH_MAX_SIZE, 0x20_0000);
    }

    #[test]
    fn not_found_error_tells_the_user_what_to_do() {
        let message = Error::DeviceNotFound.to_string();
        assert!(message.contains("1f3a:efe8"));
        assert!(message.contains("Hold Reset"));
    }
}
