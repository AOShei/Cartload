//! The FEL protocol: memory access and code execution on a connected boot ROM.

use std::time::Duration;

use tracing::{debug, info, warn};

use crate::{
    DRAM_BASE, Error, FES1_BASE, MAX_CHUNK, Result, UBOOT_BASE,
    memboot::UbootImage,
    message::{
        FelCommand, FelMessage, FelRequest, FelStatus, UsbRequest, UsbResponse,
        VerifyDeviceResponse,
    },
    transport::{Transport, UsbTransport, find_devices},
};

/// Number of trailing `fes1.bin` bytes compared with SRAM to detect an earlier DRAM init.
pub const FES1_CHECK_LEN: u32 = 0x80;
/// Number of leading `uboot.bin` bytes compared with DRAM to detect a resident u-boot.
pub const UBOOT_CHECK_LEN: u32 = 0x20;
/// How long `fes1` needs to bring DRAM up after being started.
pub const DRAM_INIT_DELAY: Duration = Duration::from_secs(2);

/// Upload progress, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Bytes transferred so far.
    pub done: usize,
    /// Total bytes to transfer.
    pub total: usize,
}

/// A console in FEL mode.
///
/// Generic over the [`Transport`] so the protocol can be tested without hardware; the default
/// is the nusb-backed [`UsbTransport`].
pub struct Device<T: Transport = UsbTransport> {
    transport: T,
    dram_ready: bool,
}

impl Device<UsbTransport> {
    /// Open the first connected FEL device.
    pub async fn open() -> Result<Self> {
        let devices = find_devices().await?;
        let info = devices.first().ok_or(Error::DeviceNotFound)?;
        if devices.len() > 1 {
            warn!(
                count = devices.len(),
                "more than one FEL device connected; using the first"
            );
        }
        Self::open_info(info).await
    }

    /// Open a specific device found by [`find_devices`] or
    /// [`wait_for_device`](crate::wait_for_device).
    pub async fn open_info(info: &nusb::DeviceInfo) -> Result<Self> {
        Ok(Self::from_transport(UsbTransport::open(info).await?))
    }
}

impl<T: Transport> Device<T> {
    /// Wrap an already-open transport.
    pub fn from_transport(transport: T) -> Self {
        Self {
            transport,
            dram_ready: false,
        }
    }

    /// The underlying transport.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// The underlying transport, mutably (to adjust timeouts, for example).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Whether [`init_dram`](Self::init_dram) has completed on this handle.
    pub fn dram_ready(&self) -> bool {
        self.dram_ready
    }

    /// Give the transport back.
    pub fn into_transport(self) -> T {
        self.transport
    }

    // USB-level framing: every FEL message or data block is wrapped in AWUC / AWUS.

    async fn usb_write(&mut self, data: &[u8]) -> Result<()> {
        let len = u32::try_from(data.len()).map_err(|_| Error::TransferTooLarge(data.len()))?;
        self.transport
            .write(&UsbRequest::write(len).to_bytes())
            .await?;
        self.transport.write(data).await?;
        self.usb_response("write").await
    }

    async fn usb_read(&mut self, len: usize) -> Result<Vec<u8>> {
        let len32 = u32::try_from(len).map_err(|_| Error::TransferTooLarge(len))?;
        self.transport
            .write(&UsbRequest::read(len32).to_bytes())
            .await?;
        let data = self.transport.read(len).await?;
        self.usb_response("read").await?;
        Ok(data)
    }

    async fn usb_response(&mut self, op: &'static str) -> Result<()> {
        let raw = self.transport.read(UsbResponse::LEN).await?;
        let response = UsbResponse::from_bytes(&raw)?;
        if response.status != 0 {
            return Err(Error::UsbStatus {
                op,
                status: response.status,
            });
        }
        Ok(())
    }

    // FEL-level messages.

    async fn fel_request(&mut self, cmd: FelCommand) -> Result<()> {
        self.usb_write(&FelRequest::new(cmd).to_bytes()).await
    }

    async fn fel_message(&mut self, cmd: FelCommand, address: u32, len: u32) -> Result<()> {
        self.usb_write(&FelMessage::new(cmd, address, len).to_bytes())
            .await
    }

    async fn fel_status(&mut self) -> Result<FelStatus> {
        let raw = self.usb_read(FelStatus::LEN).await?;
        FelStatus::from_bytes(&raw)
    }

    async fn expect_ok(&mut self, op: &'static str) -> Result<()> {
        let status = self.fel_status().await?;
        if status.state != 0 {
            return Err(Error::FelStatus {
                op,
                state: status.state,
            });
        }
        Ok(())
    }

    fn check_range(&self, address: u32, len: usize) -> Result<()> {
        let end = (address as usize)
            .checked_add(len)
            .filter(|end| *end <= 1 << 32);
        if end.is_none() {
            return Err(Error::AddressOverflow { address, len });
        }
        if address >= DRAM_BASE && !self.dram_ready {
            return Err(Error::DramNotInitialised);
        }
        Ok(())
    }

    /// Ask the boot ROM to identify itself.
    ///
    /// A console in normal boot (stock firmware) has the same USB id but never answers; that
    /// surfaces as [`Error::Timeout`].
    pub async fn verify_device(&mut self) -> Result<VerifyDeviceResponse> {
        self.fel_request(FelCommand::VerifyDevice).await?;
        let raw = self.usb_read(VerifyDeviceResponse::LEN).await?;
        let status = self.fel_status().await?;
        if status.state != 0 {
            debug!(
                state = status.state,
                "verify device returned a non-zero state; ignoring"
            );
        }
        let response = VerifyDeviceResponse::from_bytes(&raw)?;
        info!("{response}");
        Ok(response)
    }

    /// Read `len` bytes from `address`.
    ///
    /// Reads are done in 64 KiB chunks and rounded up to whole words on the wire; the result
    /// is trimmed to `len`. Addresses in DRAM need [`init_dram`](Self::init_dram) first.
    pub async fn read_memory(&mut self, address: u32, len: u32) -> Result<Vec<u8>> {
        let padded = (len as usize).div_ceil(4) * 4;
        self.check_range(address, padded)?;
        let mut out = Vec::with_capacity(padded);
        let mut remaining = u32::try_from(padded).map_err(|_| Error::TransferTooLarge(padded))?;
        let mut cursor = address;
        while remaining > 0 {
            let chunk = remaining.min(MAX_CHUNK as u32);
            self.fel_message(FelCommand::Upload, cursor, chunk).await?;
            let data = self.usb_read(chunk as usize).await?;
            self.expect_ok("read memory").await?;
            out.extend_from_slice(&data);
            cursor = cursor.wrapping_add(chunk);
            remaining -= chunk;
        }
        out.truncate(len as usize);
        debug!(
            address = format_args!("{address:#010x}"),
            len, "read memory"
        );
        Ok(out)
    }

    /// Write `data` to `address`.
    ///
    /// Writes are done in 64 KiB chunks; the last chunk is zero-padded to a whole word.
    /// Addresses in DRAM need [`init_dram`](Self::init_dram) first.
    pub async fn write_memory(&mut self, address: u32, data: &[u8]) -> Result<()> {
        self.write_memory_with_progress(address, data, |_| {}).await
    }

    /// [`write_memory`](Self::write_memory) reporting progress after every chunk.
    pub async fn write_memory_with_progress(
        &mut self,
        address: u32,
        data: &[u8],
        mut progress: impl FnMut(Progress) + Send,
    ) -> Result<()> {
        let total = data.len();
        self.check_range(address, total.div_ceil(4) * 4)?;
        progress(Progress { done: 0, total });
        let mut pos = 0;
        while pos < total {
            let end = (pos + MAX_CHUNK).min(total);
            let mut chunk = data[pos..end].to_vec();
            chunk.resize(chunk.len().div_ceil(4) * 4, 0);
            let chunk_len =
                u32::try_from(chunk.len()).map_err(|_| Error::TransferTooLarge(chunk.len()))?;
            let cursor = address.wrapping_add(pos as u32);
            self.fel_message(FelCommand::Download, cursor, chunk_len)
                .await?;
            self.usb_write(&chunk).await?;
            self.expect_ok("write memory").await?;
            pos = end;
            progress(Progress { done: pos, total });
        }
        debug!(
            address = format_args!("{address:#010x}"),
            len = total,
            "wrote memory"
        );
        Ok(())
    }

    /// Jump to `address`.
    pub async fn exec(&mut self, address: u32) -> Result<()> {
        info!(address = format_args!("{address:#010x}"), "exec");
        self.fel_message(FelCommand::Run, address, 0).await?;
        self.expect_ok("exec").await
    }

    /// Initialise DRAM by uploading and running `fes1.bin` (the first-stage DRAM init blob).
    ///
    /// If the tail of `fes1` is already resident in SRAM at its load address, a previous run
    /// initialised DRAM and nothing is uploaded. Otherwise `fes1` is written to
    /// [`FES1_BASE`], executed, and the call waits [`DRAM_INIT_DELAY`].
    pub async fn init_dram(&mut self, fes1: &[u8]) -> Result<()> {
        if self.dram_ready {
            return Ok(());
        }
        let fes1_len = u32::try_from(fes1.len()).map_err(|_| Error::InvalidFes1(fes1.len()))?;
        if fes1_len < FES1_CHECK_LEN {
            return Err(Error::InvalidFes1(fes1.len()));
        }
        let tail_address = FES1_BASE + fes1_len - FES1_CHECK_LEN;
        let resident = self.read_memory(tail_address, FES1_CHECK_LEN).await?;
        if resident == fes1[fes1.len() - FES1_CHECK_LEN as usize..] {
            info!("fes1 already resident in SRAM; DRAM is initialised");
            self.dram_ready = true;
            return Ok(());
        }
        info!(
            len = fes1.len(),
            "uploading fes1 to {FES1_BASE:#x} and running it"
        );
        self.write_memory(FES1_BASE, fes1).await?;
        self.exec(FES1_BASE).await?;
        tokio::time::sleep(DRAM_INIT_DELAY).await;
        self.dram_ready = true;
        Ok(())
    }

    /// Run a u-boot command.
    ///
    /// Uploads `uboot` to [`UBOOT_BASE`] unless its first bytes are already there, writes
    /// `command` (NUL-terminated) over the `bootcmd` value inside the image, and jumps to
    /// u-boot. Control does not return to FEL for commands like `boota` that start a kernel;
    /// for commands that do return, re-open the device after a few seconds.
    pub async fn run_uboot_cmd(&mut self, uboot: &UbootImage, command: &str) -> Result<()> {
        if !self.dram_ready {
            return Err(Error::DramNotInitialised);
        }
        let image = uboot.data();
        let resident = self.read_memory(UBOOT_BASE, UBOOT_CHECK_LEN).await?;
        if resident == image[..UBOOT_CHECK_LEN as usize] {
            debug!("u-boot already resident at {UBOOT_BASE:#x}");
        } else {
            info!(len = image.len(), "uploading u-boot to {UBOOT_BASE:#x}");
            self.write_memory(UBOOT_BASE, image).await?;
        }
        let mut bytes = command.as_bytes().to_vec();
        bytes.push(0);
        if uboot.cmd_offset() + bytes.len() > image.len() {
            return Err(Error::CommandTooLong(command.len()));
        }
        let cmd_offset =
            u32::try_from(uboot.cmd_offset()).map_err(|_| Error::CommandTooLong(command.len()))?;
        self.write_memory(UBOOT_BASE + cmd_offset, &bytes).await?;
        info!(command, "running u-boot command");
        self.exec(UBOOT_BASE).await
    }
}

/// Scripted transport for tests: a queue of expected writes and canned reads.
#[cfg(test)]
pub(crate) mod mock {
    use std::collections::VecDeque;

    use super::*;

    /// One expected interaction.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Step {
        /// The device must receive exactly these bytes.
        Write(Vec<u8>),
        /// The device answers with these bytes (the read length must match).
        Read(Vec<u8>),
    }

    /// Builder for the expected wire sequence, mirroring the protocol framing.
    #[derive(Default, Debug)]
    pub struct Script {
        pub steps: VecDeque<Step>,
    }

    impl Script {
        pub fn write(&mut self, data: &[u8]) {
            self.steps.push_back(Step::Write(data.to_vec()));
        }

        pub fn read(&mut self, data: &[u8]) {
            self.steps.push_back(Step::Read(data.to_vec()));
        }

        /// A framed write: AWUC header, payload, AWUS ok.
        pub fn usb_write(&mut self, payload: &[u8]) {
            self.write(&UsbRequest::write(payload.len() as u32).to_bytes());
            self.write(payload);
            self.read(&UsbResponse::ok().to_bytes());
        }

        /// A framed read: AWUC header, payload, AWUS ok.
        pub fn usb_read(&mut self, payload: &[u8]) {
            self.write(&UsbRequest::read(payload.len() as u32).to_bytes());
            self.read(payload);
            self.read(&UsbResponse::ok().to_bytes());
        }

        pub fn fel_request(&mut self, cmd: FelCommand) {
            self.usb_write(&FelRequest::new(cmd).to_bytes());
        }

        pub fn fel_message(&mut self, cmd: FelCommand, address: u32, len: u32) {
            self.usb_write(&FelMessage::new(cmd, address, len).to_bytes());
        }

        pub fn fel_status(&mut self, state: u8) {
            self.usb_read(
                &FelStatus {
                    state,
                    ..FelStatus::ok()
                }
                .to_bytes(),
            );
        }

        pub fn verify_device(&mut self, response: &VerifyDeviceResponse) {
            self.fel_request(FelCommand::VerifyDevice);
            self.usb_read(&response.to_bytes());
            self.fel_status(0);
        }

        /// The device returns `data` for a read of `address` (already word-padded).
        pub fn read_memory(&mut self, address: u32, data: &[u8]) {
            let mut pos = 0;
            while pos < data.len() {
                let end = (pos + MAX_CHUNK).min(data.len());
                self.fel_message(FelCommand::Upload, address + pos as u32, (end - pos) as u32);
                self.usb_read(&data[pos..end]);
                self.fel_status(0);
                pos = end;
            }
        }

        /// The device must receive `data` at `address` (already word-padded).
        pub fn write_memory(&mut self, address: u32, data: &[u8]) {
            let mut pos = 0;
            while pos < data.len() {
                let end = (pos + MAX_CHUNK).min(data.len());
                self.fel_message(
                    FelCommand::Download,
                    address + pos as u32,
                    (end - pos) as u32,
                );
                self.usb_write(&data[pos..end]);
                self.fel_status(0);
                pos = end;
            }
        }

        pub fn exec(&mut self, address: u32) {
            self.fel_message(FelCommand::Run, address, 0);
            self.fel_status(0);
        }

        pub fn into_transport(self) -> MockTransport {
            MockTransport {
                steps: self.steps,
                consumed: 0,
            }
        }
    }

    /// Transport that replays a [`Script`] and panics on any deviation.
    #[derive(Debug)]
    pub struct MockTransport {
        steps: VecDeque<Step>,
        consumed: usize,
    }

    impl MockTransport {
        /// Assert that every scripted step was consumed.
        pub fn finish(&self) {
            assert!(
                self.steps.is_empty(),
                "script has {} unconsumed steps after {} consumed; next: {:?}",
                self.steps.len(),
                self.consumed,
                self.steps.front().map(describe)
            );
        }
    }

    fn describe(step: &Step) -> String {
        match step {
            Step::Write(d) => format!("write of {} bytes {:02x?}", d.len(), &d[..d.len().min(16)]),
            Step::Read(d) => format!("read of {} bytes {:02x?}", d.len(), &d[..d.len().min(16)]),
        }
    }

    fn assert_bytes(actual: &[u8], expected: &[u8], step: usize) {
        if actual != expected {
            let first = actual.iter().zip(expected).position(|(a, b)| a != b);
            panic!(
                "step {step}: wrote {} bytes, expected {} bytes; first difference at {first:?}: \
                 got {:02x?}, expected {:02x?}",
                actual.len(),
                expected.len(),
                &actual[..actual.len().min(20)],
                &expected[..expected.len().min(20)],
            );
        }
    }

    impl Transport for MockTransport {
        async fn write(&mut self, data: &[u8]) -> Result<()> {
            let step = self.consumed;
            self.consumed += 1;
            match self.steps.pop_front() {
                Some(Step::Write(expected)) => assert_bytes(data, &expected, step),
                other => panic!(
                    "step {step}: unexpected write of {} bytes; script expected {:?}",
                    data.len(),
                    other.as_ref().map(describe)
                ),
            }
            Ok(())
        }

        async fn read(&mut self, len: usize) -> Result<Vec<u8>> {
            let step = self.consumed;
            self.consumed += 1;
            match self.steps.pop_front() {
                Some(Step::Read(data)) => {
                    assert_eq!(len, data.len(), "step {step}: read length mismatch");
                    Ok(data)
                }
                other => panic!(
                    "step {step}: unexpected read of {len} bytes; script expected {:?}",
                    other.as_ref().map(describe)
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::mock::Script;
    use super::*;
    use crate::memboot::UbootVariant;

    fn r16() -> VerifyDeviceResponse {
        VerifyDeviceResponse {
            soc_id: crate::SOC_ID_R16,
            firmware: 1,
            protocol: 1,
            data_flag: 0x44,
            data_length: 8,
            scratchpad: 0x7e00,
        }
    }

    #[tokio::test]
    async fn verify_device_round_trip() {
        let mut script = Script::default();
        script.verify_device(&r16());
        let mut device = Device::from_transport(script.into_transport());
        let response = device.verify_device().await.unwrap();
        assert_eq!(response, r16());
        device.transport().finish();
    }

    #[tokio::test]
    async fn verify_device_rejects_garbage() {
        let mut script = Script::default();
        script.fel_request(FelCommand::VerifyDevice);
        script.usb_read(&[0u8; 32]);
        script.fel_status(0);
        let mut device = Device::from_transport(script.into_transport());
        assert!(matches!(
            device.verify_device().await,
            Err(Error::BadMagic { .. })
        ));
    }

    #[tokio::test]
    async fn usb_status_failure_is_reported() {
        let mut script = Script::default();
        script.write(&UsbRequest::write(16).to_bytes());
        script.write(&FelRequest::new(FelCommand::VerifyDevice).to_bytes());
        script.read(
            &UsbResponse {
                status: 1,
                ..UsbResponse::ok()
            }
            .to_bytes(),
        );
        let mut device = Device::from_transport(script.into_transport());
        assert!(matches!(
            device.verify_device().await,
            Err(Error::UsbStatus {
                op: "write",
                status: 1
            })
        ));
    }

    #[tokio::test]
    async fn read_memory_rounds_to_words_and_trims() {
        let mut script = Script::default();
        script.read_memory(0x2000, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let mut device = Device::from_transport(script.into_transport());
        assert_eq!(
            device.read_memory(0x2000, 5).await.unwrap(),
            vec![1, 2, 3, 4, 5]
        );
        device.transport().finish();
    }

    #[tokio::test]
    async fn read_memory_chunks_at_64k() {
        let data: Vec<u8> = (0..(MAX_CHUNK + 4)).map(|i| i as u8).collect();
        let mut script = Script::default();
        script.read_memory(0x1000, &data);
        let mut device = Device::from_transport(script.into_transport());
        assert_eq!(
            device.read_memory(0x1000, data.len() as u32).await.unwrap(),
            data
        );
        device.transport().finish();
    }

    #[tokio::test]
    async fn read_memory_reports_fel_failure() {
        let mut script = Script::default();
        script.fel_message(FelCommand::Upload, 0x2000, 4);
        script.usb_read(&[0; 4]);
        script.fel_status(0x7f);
        let mut device = Device::from_transport(script.into_transport());
        assert!(matches!(
            device.read_memory(0x2000, 4).await,
            Err(Error::FelStatus {
                op: "read memory",
                state: 0x7f
            })
        ));
    }

    #[tokio::test]
    async fn write_memory_pads_last_chunk_and_reports_progress() {
        let data: Vec<u8> = (0..(MAX_CHUNK + 5)).map(|i| (i * 7) as u8).collect();
        let mut padded = data.clone();
        padded.resize(MAX_CHUNK + 8, 0);
        let mut script = Script::default();
        script.write_memory(0x3000, &padded);
        let mut device = Device::from_transport(script.into_transport());
        let mut seen = Vec::new();
        device
            .write_memory_with_progress(0x3000, &data, |p| seen.push(p))
            .await
            .unwrap();
        assert_eq!(
            seen,
            vec![
                Progress {
                    done: 0,
                    total: data.len()
                },
                Progress {
                    done: MAX_CHUNK,
                    total: data.len()
                },
                Progress {
                    done: data.len(),
                    total: data.len()
                },
            ]
        );
        device.transport().finish();
    }

    #[tokio::test]
    async fn write_memory_to_dram_needs_init() {
        let mut device = Device::from_transport(Script::default().into_transport());
        assert!(matches!(
            device.write_memory(DRAM_BASE, &[0; 4]).await,
            Err(Error::DramNotInitialised)
        ));
        assert!(matches!(
            device.read_memory(0xffff_fffc, 8).await,
            Err(Error::AddressOverflow { .. })
        ));
    }

    #[tokio::test]
    async fn exec_sends_run() {
        let mut script = Script::default();
        script.exec(0x2000);
        let mut device = Device::from_transport(script.into_transport());
        device.exec(0x2000).await.unwrap();
        device.transport().finish();
    }

    fn fes1() -> Vec<u8> {
        (0..0x200u32).map(|i| (i ^ 0x5a) as u8).collect()
    }

    #[tokio::test]
    async fn init_dram_uploads_when_not_resident() {
        let fes1 = fes1();
        let mut script = Script::default();
        script.read_memory(
            FES1_BASE + 0x200 - FES1_CHECK_LEN,
            &[0; FES1_CHECK_LEN as usize],
        );
        script.write_memory(FES1_BASE, &fes1);
        script.exec(FES1_BASE);
        let mut device = Device::from_transport(script.into_transport());
        tokio::time::pause();
        device.init_dram(&fes1).await.unwrap();
        assert!(device.dram_ready());
        device.transport().finish();
        // Second call is a no-op.
        device.init_dram(&fes1).await.unwrap();
    }

    #[tokio::test]
    async fn init_dram_skips_upload_when_resident() {
        let fes1 = fes1();
        let mut script = Script::default();
        script.read_memory(FES1_BASE + 0x200 - FES1_CHECK_LEN, &fes1[0x200 - 0x80..]);
        let mut device = Device::from_transport(script.into_transport());
        device.init_dram(&fes1).await.unwrap();
        assert!(device.dram_ready());
        device.transport().finish();
    }

    #[tokio::test]
    async fn init_dram_rejects_tiny_fes1() {
        let mut device = Device::from_transport(Script::default().into_transport());
        assert!(matches!(
            device.init_dram(&[0; 16]).await,
            Err(Error::InvalidFes1(16))
        ));
    }

    fn uboot_bytes() -> Vec<u8> {
        let mut image = vec![0xeeu8; 0x100];
        image[0x40..0x48].copy_from_slice(b"bootcmd=");
        image[0x48..0x50].copy_from_slice(b"run x\0y=");
        image[0xf8..].copy_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xde, 0xc0, 0x07, 0xb0]);
        image
    }

    #[tokio::test]
    async fn run_uboot_cmd_uploads_and_executes() {
        let uboot = UbootImage::new(&uboot_bytes(), UbootVariant::Normal).unwrap();
        let fes1 = fes1();
        let mut script = Script::default();
        script.read_memory(FES1_BASE + 0x200 - FES1_CHECK_LEN, &fes1[0x200 - 0x80..]);
        script.read_memory(UBOOT_BASE, &[0; 0x20]);
        script.write_memory(UBOOT_BASE, uboot.data());
        script.write_memory(UBOOT_BASE + 0x48, b"boota 47400000\0\0");
        script.exec(UBOOT_BASE);
        let mut device = Device::from_transport(script.into_transport());
        device.init_dram(&fes1).await.unwrap();
        device
            .run_uboot_cmd(&uboot, "boota 47400000")
            .await
            .unwrap();
        device.transport().finish();
    }

    #[tokio::test]
    async fn run_uboot_cmd_skips_upload_when_resident() {
        let uboot = UbootImage::new(&uboot_bytes(), UbootVariant::Normal).unwrap();
        let fes1 = fes1();
        let mut script = Script::default();
        script.read_memory(FES1_BASE + 0x200 - FES1_CHECK_LEN, &fes1[0x200 - 0x80..]);
        script.read_memory(UBOOT_BASE, &uboot.data()[..0x20]);
        script.write_memory(UBOOT_BASE + 0x48, b"abc\0");
        script.exec(UBOOT_BASE);
        let mut device = Device::from_transport(script.into_transport());
        device.init_dram(&fes1).await.unwrap();
        device.run_uboot_cmd(&uboot, "abc").await.unwrap();
        device.transport().finish();
    }

    #[tokio::test]
    async fn run_uboot_cmd_requires_dram() {
        let uboot = UbootImage::new(&uboot_bytes(), UbootVariant::Normal).unwrap();
        let mut device = Device::from_transport(Script::default().into_transport());
        assert!(matches!(
            device.run_uboot_cmd(&uboot, "x").await,
            Err(Error::DramNotInitialised)
        ));
    }
}
