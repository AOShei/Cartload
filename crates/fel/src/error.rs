//! Error type for the crate.

use std::time::Duration;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No USB device with the FEL vendor and product id is connected.
    #[error(
        "no FEL device found (USB 1f3a:efe8). Hold Reset while powering the console on, keep it \
         held for two seconds, then retry"
    )]
    DeviceNotFound,

    /// The device exists but the OS refused to open it.
    #[error(
        "permission denied opening the FEL device (USB 1f3a:efe8): {0}. On Linux install a udev \
         rule granting access, see docs/hardware-tests.md"
    )]
    PermissionDenied(#[source] nusb::Error),

    /// Any other USB error from the operating system.
    #[error("USB error: {0}")]
    Usb(#[from] nusb::Error),

    /// A bulk transfer failed.
    #[error("USB transfer failed: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),

    /// The device did not answer in time. A console that enumerates as 1f3a:efe8 but is in
    /// normal boot (stock firmware) behaves like this.
    #[error(
        "the device did not answer within {timeout:?} ({op}). It is probably in normal boot, not \
         FEL mode: power it off, hold Reset, power on while still holding Reset, then retry"
    )]
    Timeout {
        /// What was being waited for.
        op: &'static str,
        /// How long was waited.
        timeout: Duration,
    },

    /// A bulk transfer moved fewer bytes than requested.
    #[error("short USB transfer during {op}: {actual} of {expected} bytes")]
    ShortTransfer {
        /// The operation in progress.
        op: &'static str,
        /// Bytes requested.
        expected: usize,
        /// Bytes transferred.
        actual: usize,
    },

    /// Interface 0 has no bulk IN and OUT endpoint pair, so this is not the FEL boot ROM.
    #[error(
        "the device has no bulk IN/OUT endpoint pair on interface 0, so it is not in FEL mode. \
         Hold Reset while powering the console on"
    )]
    NoBulkEndpoints,

    /// A transfer larger than the protocol allows was requested.
    #[error("transfer of {0} bytes is larger than the FEL protocol allows")]
    TransferTooLarge(usize),

    /// A response was shorter than its fixed layout.
    #[error("malformed {what}: expected {expected} bytes, got {actual}")]
    ShortResponse {
        /// The message type.
        what: &'static str,
        /// Expected length.
        expected: usize,
        /// Actual length.
        actual: usize,
    },

    /// A response did not start with the expected magic bytes.
    #[error("malformed {what}: bad magic")]
    BadMagic {
        /// The message type.
        what: &'static str,
    },

    /// The USB-level wrapper reported a failure status.
    #[error("USB layer reported failure (status {status:#04x}) during {op}")]
    UsbStatus {
        /// The operation in progress.
        op: &'static str,
        /// Status byte from the device.
        status: u8,
    },

    /// A FEL command reported a non-zero state.
    #[error("FEL command failed on the device during {op} (state {state:#04x})")]
    FelStatus {
        /// The operation in progress.
        op: &'static str,
        /// State byte from the device.
        state: u8,
    },

    /// The device is not an Allwinner R16.
    #[error(
        "unexpected SoC id {found:#010x}; the Classic Mini consoles use the Allwinner R16 \
         ({expected:#010x})"
    )]
    UnexpectedSoc {
        /// SoC id reported by the device.
        found: u32,
        /// SoC id of the R16.
        expected: u32,
    },

    /// A memory range does not fit in the 32-bit address space.
    #[error("memory range {address:#010x}+{len:#x} does not fit in 32-bit address space")]
    AddressOverflow {
        /// Start address.
        address: u32,
        /// Length in bytes.
        len: usize,
    },

    /// DRAM was not initialised before a DRAM address was accessed.
    #[error(
        "DRAM is not initialised; run init_dram with fes1.bin before accessing addresses at or \
         above 0x40000000"
    )]
    DramNotInitialised,

    /// `fes1.bin` is too small to be the DRAM init binary.
    #[error("fes1.bin is too small ({0} bytes)")]
    InvalidFes1(usize),

    /// The u-boot image cannot be used.
    #[error("u-boot image is invalid: {0}")]
    InvalidUboot(&'static str),

    /// A u-boot command does not fit in the image's environment area.
    #[error("u-boot command is too long ({0} bytes)")]
    CommandTooLong(usize),

    /// The boot image cannot be used.
    #[error("boot image is invalid: {0}")]
    InvalidBootImage(&'static str),

    /// The boot image file is shorter than its header declares.
    #[error("boot image is {size} bytes but its header describes {declared} bytes")]
    BootImageTruncated {
        /// File size.
        size: usize,
        /// Size computed from the header.
        declared: u32,
    },

    /// The boot image exceeds the memboot transfer area.
    #[error("boot image is too large for memboot ({size} bytes, maximum {max})")]
    BootImageTooLarge {
        /// Size computed from the header.
        size: u32,
        /// Maximum allowed.
        max: u32,
    },

    /// The patched kernel command line does not fit the header field.
    #[error("kernel command line would be {len} bytes after patching, maximum is {max}")]
    CmdlineTooLong {
        /// Resulting length.
        len: usize,
        /// Maximum length (leaving room for the terminating NUL).
        max: usize,
    },
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
