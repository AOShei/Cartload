//! FEL protocol messages with explicit byte layouts.
//!
//! Every multi-byte field is little-endian. The layouts follow the `AW*` classes of hakchi2 CE's
//! `FelLib` and agree with `fel_lib.c` in sunxi-tools.
//!
//! | Message | Bytes | Direction | Layout |
//! | --- | --- | --- | --- |
//! | [`UsbRequest`] | 32 | host to device | `AWUC`, tag u32, len u32, u16 0, u8 0, cmd_len u8 = 0x0c, cmd u8, u8 0, len u32, 10 zero bytes |
//! | [`UsbResponse`] | 13 | device to host | `AWUS`, tag u32, residue u32, status u8 |
//! | [`FelRequest`] | 16 | host to device | cmd u16, tag u16, 12 zero bytes |
//! | [`FelMessage`] | 16 | host to device | cmd u16, tag u16, address u32, len u32, flags u32 |
//! | [`FelStatus`] | 8 | device to host | mark u16, tag u16, state u8, 3 zero bytes |
//! | [`VerifyDeviceResponse`] | 32 | device to host | `AWUSBFEX`, soc_id u32, firmware u32, protocol u16, data_flag u8, data_length u8, scratchpad u32, 8 zero bytes |
//!
//! Every FEL request and every data block travels inside a [`UsbRequest`] / [`UsbResponse`]
//! pair: the host writes the 32-byte request, then the payload (for writes), then reads the
//! payload (for reads), then reads the 13-byte response.

use std::fmt;

use crate::{Error, Result};

fn le_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn le_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn put_u16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn check_len(what: &'static str, bytes: &[u8], expected: usize) -> Result<()> {
    if bytes.len() < expected {
        return Err(Error::ShortResponse {
            what,
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn check_magic(what: &'static str, bytes: &[u8], magic: &[u8]) -> Result<()> {
    if !bytes.starts_with(magic) {
        return Err(Error::BadMagic { what });
    }
    Ok(())
}

/// Direction of the USB-level wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbCommand {
    /// The host will read `len` bytes from the device.
    Read = 0x11,
    /// The host will write `len` bytes to the device.
    Write = 0x12,
}

impl UsbCommand {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x11 => Some(Self::Read),
            0x12 => Some(Self::Write),
            _ => None,
        }
    }
}

/// USB-level request (`AWUC`) that frames every FEL message and data block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbRequest {
    /// Request tag, always 0 in practice.
    pub tag: u32,
    /// Number of payload bytes that follow (write) or are expected back (read).
    pub len: u32,
    /// Transfer direction.
    pub cmd: UsbCommand,
}

impl UsbRequest {
    /// Encoded size in bytes.
    pub const LEN: usize = 32;
    /// Leading magic.
    pub const MAGIC: &[u8; 4] = b"AWUC";
    /// Value of the `cmd_len` byte.
    pub const CMD_LEN: u8 = 0x0c;

    /// Request announcing that the host will read `len` bytes.
    pub const fn read(len: u32) -> Self {
        Self {
            tag: 0,
            len,
            cmd: UsbCommand::Read,
        }
    }

    /// Request announcing that the host will write `len` bytes.
    pub const fn write(len: u32) -> Self {
        Self {
            tag: 0,
            len,
            cmd: UsbCommand::Write,
        }
    }

    /// Encode to the wire layout.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        out[0..4].copy_from_slice(Self::MAGIC);
        put_u32(&mut out, 4, self.tag);
        put_u32(&mut out, 8, self.len);
        out[15] = Self::CMD_LEN;
        out[16] = self.cmd as u8;
        put_u32(&mut out, 18, self.len);
        out
    }

    /// Decode from the wire layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        const WHAT: &str = "USB request";
        check_len(WHAT, bytes, Self::LEN)?;
        check_magic(WHAT, bytes, Self::MAGIC)?;
        let cmd = UsbCommand::from_u8(bytes[16]).ok_or(Error::BadMagic { what: WHAT })?;
        Ok(Self {
            tag: le_u32(bytes, 4),
            len: le_u32(bytes, 8),
            cmd,
        })
    }
}

/// USB-level response (`AWUS`) that closes every framed transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsbResponse {
    /// Echo of the request tag.
    pub tag: u32,
    /// Bytes not transferred.
    pub residue: u32,
    /// 0 on success.
    pub status: u8,
}

impl UsbResponse {
    /// Encoded size in bytes.
    pub const LEN: usize = 13;
    /// Leading magic.
    pub const MAGIC: &[u8; 4] = b"AWUS";

    /// A successful response with tag 0.
    pub const fn ok() -> Self {
        Self {
            tag: 0,
            residue: 0,
            status: 0,
        }
    }

    /// Encode to the wire layout.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        out[0..4].copy_from_slice(Self::MAGIC);
        put_u32(&mut out, 4, self.tag);
        put_u32(&mut out, 8, self.residue);
        out[12] = self.status;
        out
    }

    /// Decode from the wire layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        const WHAT: &str = "USB response";
        check_len(WHAT, bytes, Self::LEN)?;
        check_magic(WHAT, bytes, Self::MAGIC)?;
        Ok(Self {
            tag: le_u32(bytes, 4),
            residue: le_u32(bytes, 8),
            status: bytes[12],
        })
    }
}

/// FEL command codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum FelCommand {
    /// Ask for the 32-byte [`VerifyDeviceResponse`].
    VerifyDevice = 0x1,
    /// Switch role (unused here).
    SwitchRole = 0x2,
    /// Ask whether the device is ready (8-byte answer, unused here).
    IsReady = 0x3,
    /// Ask for the command set version (unused here).
    GetCmdSetVer = 0x4,
    /// Disconnect (unused here).
    Disconnect = 0x10,
    /// Write memory: the host sends `len` bytes to `address`.
    Download = 0x101,
    /// Execute code at `address`.
    Run = 0x102,
    /// Read memory: the device sends `len` bytes from `address`.
    Upload = 0x103,
}

impl FelCommand {
    fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x1 => Some(Self::VerifyDevice),
            0x2 => Some(Self::SwitchRole),
            0x3 => Some(Self::IsReady),
            0x4 => Some(Self::GetCmdSetVer),
            0x10 => Some(Self::Disconnect),
            0x101 => Some(Self::Download),
            0x102 => Some(Self::Run),
            0x103 => Some(Self::Upload),
            _ => None,
        }
    }
}

/// FEL request without parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FelRequest {
    /// Command code.
    pub cmd: FelCommand,
    /// Request tag, always 0 in practice.
    pub tag: u16,
}

impl FelRequest {
    /// Encoded size in bytes.
    pub const LEN: usize = 16;

    /// Request for `cmd` with tag 0.
    pub const fn new(cmd: FelCommand) -> Self {
        Self { cmd, tag: 0 }
    }

    /// Encode to the wire layout.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        put_u16(&mut out, 0, self.cmd as u16);
        put_u16(&mut out, 2, self.tag);
        out
    }

    /// Decode from the wire layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        const WHAT: &str = "FEL request";
        check_len(WHAT, bytes, Self::LEN)?;
        let cmd = FelCommand::from_u16(le_u16(bytes, 0)).ok_or(Error::BadMagic { what: WHAT })?;
        Ok(Self {
            cmd,
            tag: le_u16(bytes, 2),
        })
    }
}

/// FEL request carrying an address and a length (download, upload, run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FelMessage {
    /// Command code.
    pub cmd: FelCommand,
    /// Request tag, always 0 in practice.
    pub tag: u16,
    /// Target address on the device.
    pub address: u32,
    /// Length in bytes (0 for run).
    pub len: u32,
    /// Flags, always 0 in practice.
    pub flags: u32,
}

impl FelMessage {
    /// Encoded size in bytes.
    pub const LEN: usize = 16;

    /// Message for `cmd` at `address` with `len` bytes, tag and flags 0.
    pub const fn new(cmd: FelCommand, address: u32, len: u32) -> Self {
        Self {
            cmd,
            tag: 0,
            address,
            len,
            flags: 0,
        }
    }

    /// Encode to the wire layout.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        put_u16(&mut out, 0, self.cmd as u16);
        put_u16(&mut out, 2, self.tag);
        put_u32(&mut out, 4, self.address);
        put_u32(&mut out, 8, self.len);
        put_u32(&mut out, 12, self.flags);
        out
    }

    /// Decode from the wire layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        const WHAT: &str = "FEL message";
        check_len(WHAT, bytes, Self::LEN)?;
        let cmd = FelCommand::from_u16(le_u16(bytes, 0)).ok_or(Error::BadMagic { what: WHAT })?;
        Ok(Self {
            cmd,
            tag: le_u16(bytes, 2),
            address: le_u32(bytes, 4),
            len: le_u32(bytes, 8),
            flags: le_u32(bytes, 12),
        })
    }
}

/// Status the device returns after every FEL command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FelStatus {
    /// Marker, 0xffff.
    pub mark: u16,
    /// Echo of the request tag.
    pub tag: u16,
    /// 0 on success.
    pub state: u8,
}

impl FelStatus {
    /// Encoded size in bytes.
    pub const LEN: usize = 8;
    /// Expected marker value.
    pub const MARK: u16 = 0xffff;

    /// A successful status with tag 0.
    pub const fn ok() -> Self {
        Self {
            mark: Self::MARK,
            tag: 0,
            state: 0,
        }
    }

    /// Encode to the wire layout.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        put_u16(&mut out, 0, self.mark);
        put_u16(&mut out, 2, self.tag);
        out[4] = self.state;
        out
    }

    /// Decode from the wire layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        check_len("FEL status", bytes, Self::LEN)?;
        Ok(Self {
            mark: le_u16(bytes, 0),
            tag: le_u16(bytes, 2),
            state: bytes[4],
        })
    }
}

/// Answer to [`FelCommand::VerifyDevice`]: identifies the SoC and the boot ROM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifyDeviceResponse {
    /// SoC id, `0x00166700` on the R16 (the Classic Mini consoles).
    pub soc_id: u32,
    /// Boot ROM firmware field, 1 in practice.
    pub firmware: u32,
    /// Protocol version, 1 in practice.
    pub protocol: u16,
    /// Data flag, 0x44 in practice.
    pub data_flag: u8,
    /// Data length, 0x08 in practice.
    pub data_length: u8,
    /// Scratchpad (data start) address in SRAM.
    pub scratchpad: u32,
}

impl VerifyDeviceResponse {
    /// Encoded size in bytes.
    pub const LEN: usize = 32;
    /// Leading magic.
    pub const MAGIC: &[u8; 8] = b"AWUSBFEX";

    /// Encode to the wire layout.
    pub fn to_bytes(&self) -> [u8; Self::LEN] {
        let mut out = [0u8; Self::LEN];
        out[0..8].copy_from_slice(Self::MAGIC);
        put_u32(&mut out, 8, self.soc_id);
        put_u32(&mut out, 12, self.firmware);
        put_u16(&mut out, 16, self.protocol);
        out[18] = self.data_flag;
        out[19] = self.data_length;
        put_u32(&mut out, 20, self.scratchpad);
        out
    }

    /// Decode from the wire layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        const WHAT: &str = "verify device response";
        check_len(WHAT, bytes, Self::LEN)?;
        check_magic(WHAT, bytes, Self::MAGIC)?;
        Ok(Self {
            soc_id: le_u32(bytes, 8),
            firmware: le_u32(bytes, 12),
            protocol: le_u16(bytes, 16),
            data_flag: bytes[18],
            data_length: bytes[19],
            scratchpad: le_u32(bytes, 20),
        })
    }

    /// The 16-bit SoC number as sunxi-tools prints it (`0x1667` for the R16).
    pub const fn soc_number(&self) -> u16 {
        ((self.soc_id >> 8) & 0xffff) as u16
    }

    /// Whether this is the Allwinner R16 used by all three Classic Mini consoles.
    pub const fn is_r16(&self) -> bool {
        self.soc_id == crate::SOC_ID_R16
    }

    /// Marketing name of the SoC, when known.
    pub const fn soc_name(&self) -> Option<&'static str> {
        Some(match self.soc_number() {
            0x1623 => "A10",
            0x1625 => "A13 / R8",
            0x1633 => "A31",
            0x1639 => "A80",
            0x1650 => "A23",
            0x1651 => "A20",
            0x1663 => "F1C100s",
            0x1667 => "A33 / R16",
            0x1673 => "A83T",
            0x1680 => "H3",
            0x1681 => "V3s",
            0x1689 => "A64",
            0x1701 => "R40",
            0x1718 => "H6",
            0x1728 => "H616",
            _ => return None,
        })
    }
}

impl fmt::Display for VerifyDeviceResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AWUSBFEX soc={:#010x} ({}) firmware={} protocol={} data_flag={:#04x} \
             data_length={:#04x} scratchpad={:#010x}",
            self.soc_id,
            self.soc_name().unwrap_or("unknown SoC"),
            self.firmware,
            self.protocol,
            self.data_flag,
            self.data_length,
            self.scratchpad,
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    // Byte vectors derived by hand from the hakchi2 CE AW* class layouts.

    #[test]
    fn usb_request_write_layout() {
        let req = UsbRequest::write(0x10000);
        let bytes = req.to_bytes();
        let expected: [u8; 32] = [
            b'A', b'W', b'U', b'C', // magic
            0, 0, 0, 0, // tag
            0x00, 0x00, 0x01, 0x00, // len
            0, 0, 0, 0x0c, // reserved, cmd_len
            0x12, 0, // cmd, reserved
            0x00, 0x00, 0x01, 0x00, // len again
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(bytes, expected);
        assert_eq!(UsbRequest::from_bytes(&bytes).unwrap(), req);
    }

    #[test]
    fn usb_request_read_layout() {
        let req = UsbRequest::read(0x20);
        let bytes = req.to_bytes();
        let expected: [u8; 32] = [
            b'A', b'W', b'U', b'C', 0, 0, 0, 0, 0x20, 0, 0, 0, 0, 0, 0, 0x0c, 0x11, 0, 0x20, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(bytes, expected);
        assert_eq!(UsbRequest::from_bytes(&bytes).unwrap(), req);
    }

    #[test]
    fn usb_request_rejects_bad_magic_and_short_input() {
        let mut bytes = UsbRequest::read(1).to_bytes();
        bytes[0] = b'X';
        assert!(matches!(
            UsbRequest::from_bytes(&bytes),
            Err(Error::BadMagic { .. })
        ));
        assert!(matches!(
            UsbRequest::from_bytes(&bytes[..10]),
            Err(Error::ShortResponse {
                expected: 32,
                actual: 10,
                ..
            })
        ));
    }

    #[test]
    fn usb_response_layout() {
        let bytes: [u8; 13] = [
            b'A', b'W', b'U', b'S', 0x78, 0x56, 0x34, 0x12, 0x04, 0x00, 0x00, 0x00, 0x02,
        ];
        let resp = UsbResponse::from_bytes(&bytes).unwrap();
        assert_eq!(
            resp,
            UsbResponse {
                tag: 0x12345678,
                residue: 4,
                status: 2
            }
        );
        assert_eq!(resp.to_bytes(), bytes);
        assert_eq!(
            UsbResponse::ok().to_bytes(),
            [b'A', b'W', b'U', b'S', 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn fel_request_layout() {
        let bytes = FelRequest::new(FelCommand::VerifyDevice).to_bytes();
        assert_eq!(bytes, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let parsed = FelRequest::from_bytes(&bytes).unwrap();
        assert_eq!(
            parsed,
            FelRequest {
                cmd: FelCommand::VerifyDevice,
                tag: 0
            }
        );
        assert!(matches!(
            FelRequest::from_bytes(&[0x99, 0x99, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(Error::BadMagic { .. })
        ));
    }

    #[test]
    fn fel_message_layout() {
        let msg = FelMessage::new(FelCommand::Download, 0x4740_0000, 0x1_0000);
        let bytes = msg.to_bytes();
        let expected: [u8; 16] = [
            0x01, 0x01, // FEL_DOWNLOAD
            0, 0, // tag
            0x00, 0x00, 0x40, 0x47, // address
            0x00, 0x00, 0x01, 0x00, // len
            0, 0, 0, 0, // flags
        ];
        assert_eq!(bytes, expected);
        assert_eq!(FelMessage::from_bytes(&bytes).unwrap(), msg);

        let run = FelMessage::new(FelCommand::Run, 0x2000, 0).to_bytes();
        assert_eq!(
            run,
            [0x02, 0x01, 0, 0, 0x00, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        let upload = FelMessage::new(FelCommand::Upload, 0x4700_0000, 0x20).to_bytes();
        assert_eq!(
            upload,
            [0x03, 0x01, 0, 0, 0, 0, 0, 0x47, 0x20, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn fel_status_layout() {
        let bytes = [0xff, 0xff, 0, 0, 0, 0, 0, 0];
        let status = FelStatus::from_bytes(&bytes).unwrap();
        assert_eq!(status, FelStatus::ok());
        assert_eq!(status.to_bytes(), bytes);
        let failed = FelStatus::from_bytes(&[0xff, 0xff, 0x01, 0x00, 0x05, 0, 0, 0]).unwrap();
        assert_eq!(
            failed,
            FelStatus {
                mark: 0xffff,
                tag: 1,
                state: 5
            }
        );
        assert!(matches!(
            FelStatus::from_bytes(&[0xff; 4]),
            Err(Error::ShortResponse { .. })
        ));
    }

    #[test]
    fn verify_device_response_layout() {
        let bytes: [u8; 32] = [
            b'A', b'W', b'U', b'S', b'B', b'F', b'E', b'X', // magic
            0x00, 0x67, 0x16, 0x00, // soc id 0x00166700 (R16)
            0x01, 0x00, 0x00, 0x00, // firmware
            0x01, 0x00, // protocol
            0x44, 0x08, // data flag, data length
            0x00, 0x7e, 0x00, 0x00, // scratchpad 0x7e00
            0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let resp = VerifyDeviceResponse::from_bytes(&bytes).unwrap();
        assert_eq!(
            resp,
            VerifyDeviceResponse {
                soc_id: 0x0016_6700,
                firmware: 1,
                protocol: 1,
                data_flag: 0x44,
                data_length: 0x08,
                scratchpad: 0x7e00,
            }
        );
        assert_eq!(resp.to_bytes(), bytes);
        assert!(resp.is_r16());
        assert_eq!(resp.soc_number(), 0x1667);
        assert_eq!(resp.soc_name(), Some("A33 / R16"));
        assert_eq!(
            resp.to_string(),
            "AWUSBFEX soc=0x00166700 (A33 / R16) firmware=1 protocol=1 data_flag=0x44 \
             data_length=0x08 scratchpad=0x00007e00"
        );
    }

    #[test]
    fn verify_device_response_rejects_bad_magic() {
        let zeros = [0u8; 32];
        assert!(matches!(
            VerifyDeviceResponse::from_bytes(&zeros),
            Err(Error::BadMagic { .. })
        ));
        let other = VerifyDeviceResponse {
            soc_id: 0x0016_8900,
            ..VerifyDeviceResponse::from_bytes(
                &VerifyDeviceResponse {
                    soc_id: 0,
                    firmware: 0,
                    protocol: 0,
                    data_flag: 0,
                    data_length: 0,
                    scratchpad: 0,
                }
                .to_bytes(),
            )
            .unwrap()
        };
        assert!(!other.is_r16());
        assert_eq!(other.soc_name(), Some("A64"));
    }
}
