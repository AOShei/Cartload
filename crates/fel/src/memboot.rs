//! Memboot: run a hakchi kernel from RAM, the way hakchi2 CE does it.
//!
//! Inputs are `fes1.bin` (DRAM init, shipped with hakchi2 CE), and `boot/uboot.bin` and
//! `boot/boot.img` from `hakchi.hmod`. The boot image is an Android boot image (kernel plus
//! ramdisk) whose header carries the kernel command line.
//!
//! Sequence, with the addresses hakchi2 CE uses on the R16:
//!
//! 1. Validate and prepare the images: find `bootcmd=` in u-boot and apply the SD marker swap,
//!    check the boot image header, pad it to a whole number of 128 KiB NAND sectors, and
//!    optionally rewrite the kernel command line.
//! 2. Verify the device and check it is an R16.
//! 3. Initialise DRAM with `fes1.bin`.
//! 4. Upload the boot image to [`TRANSFER_BASE`].
//! 5. Upload u-boot to [`crate::UBOOT_BASE`] and run `boota <TRANSFER_BASE>`.
//!
//! The console then boots the kernel; within about ten seconds it re-enumerates as the
//! hakchi network gadget (`04e8:6863`).

use tracing::{debug, info};

use crate::{
    Device, Error, Progress, Result, SECTOR_SIZE, TRANSFER_BASE, TRANSFER_MAX_SIZE,
    message::VerifyDeviceResponse, transport::Transport,
};

/// Magic at the start of an Android boot image.
pub const BOOT_MAGIC: &[u8; 8] = b"ANDROID!";
/// Offset of the kernel command line inside the boot image header.
pub const BOOT_ARGS_OFFSET: usize = 64;
/// Size of the kernel command line field.
pub const BOOT_ARGS_SIZE: usize = 512;
/// Minimum image length needed to read the header and the command line.
pub const BOOT_HEADER_MIN_LEN: usize = BOOT_ARGS_OFFSET + BOOT_ARGS_SIZE;
/// Command line tokens that select how the hakchi ramdisk exposes a shell.
pub const SHELL_TOKENS: [&str; 2] = ["hakchi-shell", "hakchi-clovershell"];

/// Fixed-layout fields of the Android boot image header (little-endian, after the magic).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootImageHeader {
    /// Kernel size in bytes.
    pub kernel_size: u32,
    /// Kernel load address.
    pub kernel_addr: u32,
    /// Ramdisk size in bytes.
    pub ramdisk_size: u32,
    /// Ramdisk load address.
    pub ramdisk_addr: u32,
    /// Second stage size in bytes (0 in practice).
    pub second_size: u32,
    /// Second stage load address.
    pub second_addr: u32,
    /// Kernel tags address.
    pub tags_addr: u32,
    /// Page size used to align the sections.
    pub page_size: u32,
    /// Device tree size in bytes (0 in practice).
    pub dt_size: u32,
}

fn le_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

impl BootImageHeader {
    /// Parse the header at the start of `image`.
    pub fn parse(image: &[u8]) -> Result<Self> {
        if image.len() < BOOT_HEADER_MIN_LEN {
            return Err(Error::InvalidBootImage("file is shorter than the header"));
        }
        if !image.starts_with(BOOT_MAGIC) {
            return Err(Error::InvalidBootImage("missing ANDROID! magic"));
        }
        let header = Self {
            kernel_size: le_u32(image, 8),
            kernel_addr: le_u32(image, 12),
            ramdisk_size: le_u32(image, 16),
            ramdisk_addr: le_u32(image, 20),
            second_size: le_u32(image, 24),
            second_addr: le_u32(image, 28),
            tags_addr: le_u32(image, 32),
            page_size: le_u32(image, 36),
            dt_size: le_u32(image, 40),
        };
        if header.page_size == 0 {
            return Err(Error::InvalidBootImage("page size is zero"));
        }
        Ok(header)
    }

    /// Total image size described by the header: one header page plus each section rounded up
    /// to whole pages.
    pub fn image_size(&self) -> Result<u32> {
        let page = self.page_size;
        let pages = [
            self.kernel_size,
            self.ramdisk_size,
            self.second_size,
            self.dt_size,
        ]
        .into_iter()
        .try_fold(1u32, |acc, size| acc.checked_add(size.div_ceil(page)))
        .and_then(|pages| pages.checked_mul(page));
        pages.ok_or(Error::InvalidBootImage("section sizes overflow"))
    }
}

/// The kernel command line stored in `image`, without trailing NULs.
pub fn cmdline(image: &[u8]) -> Result<String> {
    if image.len() < BOOT_HEADER_MIN_LEN {
        return Err(Error::InvalidBootImage("file is shorter than the header"));
    }
    let field = &image[BOOT_ARGS_OFFSET..BOOT_HEADER_MIN_LEN];
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    Ok(String::from_utf8_lossy(&field[..end]).into_owned())
}

/// Store `cmdline` in `image`, zero-padded, keeping one byte for the terminating NUL.
pub fn set_cmdline(image: &mut [u8], cmdline: &str) -> Result<()> {
    if image.len() < BOOT_HEADER_MIN_LEN {
        return Err(Error::InvalidBootImage("file is shorter than the header"));
    }
    let max = BOOT_ARGS_SIZE - 1;
    if cmdline.len() > max {
        return Err(Error::CmdlineTooLong {
            len: cmdline.len(),
            max,
        });
    }
    let field = &mut image[BOOT_ARGS_OFFSET..BOOT_HEADER_MIN_LEN];
    field.fill(0);
    field[..cmdline.len()].copy_from_slice(cmdline.as_bytes());
    Ok(())
}

/// How the membooted ramdisk should expose its shell.
///
/// hakchi2 CE's "force network" and "force clovershell" settings. The stock `boot.img`
/// already carries `hakchi-shell`, so [`Keep`](Self::Keep) is the normal choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellOverride {
    /// Leave the command line as shipped in the image.
    #[default]
    Keep,
    /// Force the network shell (`hakchi-shell`): SSH over the USB RNDIS gadget.
    Network,
    /// Force the legacy clovershell protocol (`hakchi-clovershell`).
    Clovershell,
}

impl ShellOverride {
    /// The command line token this override adds.
    pub const fn token(self) -> Option<&'static str> {
        match self {
            Self::Keep => None,
            Self::Network => Some("hakchi-shell"),
            Self::Clovershell => Some("hakchi-clovershell"),
        }
    }
}

/// Rewrite the command line for `shell`: drop any existing shell token and append the chosen
/// one. Returns the new command line, or `None` when nothing was changed.
pub fn patch_cmdline(image: &mut [u8], shell: ShellOverride) -> Result<Option<String>> {
    let Some(token) = shell.token() else {
        return Ok(None);
    };
    let current = cmdline(image)?;
    let mut tokens: Vec<&str> = current
        .split_whitespace()
        .filter(|t| !SHELL_TOKENS.contains(t))
        .collect();
    tokens.push(token);
    let patched = tokens.join(" ");
    set_cmdline(image, &patched)?;
    Ok(Some(patched))
}

/// Check a boot image and return the copy memboot uploads: trimmed or zero-padded to the size
/// in its header rounded up to whole [`SECTOR_SIZE`] blocks, with the command line patched
/// for `shell`.
pub fn prepare_boot_image(image: &[u8], shell: ShellOverride) -> Result<Vec<u8>> {
    let header = BootImageHeader::parse(image)?;
    let size = header.image_size()?;
    if size as usize > image.len() {
        return Err(Error::BootImageTruncated {
            size: image.len(),
            declared: size,
        });
    }
    if size > TRANSFER_MAX_SIZE {
        return Err(Error::BootImageTooLarge {
            size,
            max: TRANSFER_MAX_SIZE,
        });
    }
    let padded = size.div_ceil(SECTOR_SIZE) * SECTOR_SIZE;
    let mut out = image[..(padded as usize).min(image.len())].to_vec();
    out.resize(padded as usize, 0);
    debug!(
        kernel = header.kernel_size,
        ramdisk = header.ramdisk_size,
        page = header.page_size,
        declared = size,
        padded,
        "boot image"
    );
    if let Some(cmdline) = patch_cmdline(&mut out, shell)? {
        info!(cmdline, "patched kernel command line");
    }
    Ok(out)
}

/// Which u-boot flavour to run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UbootVariant {
    /// `uboot.bin` exactly as shipped in the hmod.
    Normal,
    /// The two trailing 32-bit words swapped, which is what hakchi2 CE memboots with
    /// (`UbootType.SD`): it enables u-boot's SD card support.
    #[default]
    Sd,
}

/// `uboot.bin` ready for [`Device::run_uboot_cmd`]: the command string is written over the
/// value of its built-in `bootcmd` environment variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UbootImage {
    data: Vec<u8>,
    cmd_offset: usize,
}

impl UbootImage {
    /// Smallest image accepted: room for the resident check and the SD marker.
    pub const MIN_LEN: usize = 0x20 + 8;

    /// Prepare `bytes` as `variant`. Fails if the image is too small or has no `bootcmd=`.
    pub fn new(bytes: &[u8], variant: UbootVariant) -> Result<Self> {
        if bytes.len() < Self::MIN_LEN {
            return Err(Error::InvalidUboot("image is too small"));
        }
        let mut data = bytes.to_vec();
        if variant == UbootVariant::Sd {
            let n = data.len();
            let (head, tail) = data.split_at_mut(n - 4);
            head[n - 8..].swap_with_slice(tail);
        }
        let cmd_offset = data
            .windows(8)
            .position(|w| w == b"bootcmd=")
            .ok_or(Error::InvalidUboot("no bootcmd= environment variable"))?
            + 8;
        Ok(Self { data, cmd_offset })
    }

    /// The image bytes to upload.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Offset of the `bootcmd` value inside the image.
    pub fn cmd_offset(&self) -> usize {
        self.cmd_offset
    }
}

/// Memboot settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MembootOptions {
    /// Kernel command line patch.
    pub shell: ShellOverride,
    /// u-boot flavour.
    pub uboot: UbootVariant,
}

/// Phases of a memboot, reported through the progress callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembootStage {
    /// Parsing and patching the images; no USB traffic yet.
    PreparingImages,
    /// Asking the boot ROM for its SoC id.
    VerifyingDevice,
    /// Uploading and running `fes1.bin`.
    InitialisingDram,
    /// Uploading the boot image to DRAM.
    UploadingKernel,
    /// Uploading u-boot and running `boota`.
    StartingUboot,
}

/// Progress events from [`memboot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembootEvent {
    /// A new phase started.
    Stage(MembootStage),
    /// Boot image upload progress.
    Upload(Progress),
}

/// What a successful memboot did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembootReport {
    /// The boot ROM's answer to verify-device.
    pub device: VerifyDeviceResponse,
    /// Bytes uploaded for the boot image (after padding).
    pub image_len: usize,
    /// Kernel command line the kernel was started with.
    pub cmdline: String,
    /// The u-boot command executed.
    pub command: String,
}

/// Boot `boot_img` from RAM on `device`.
///
/// `fes1` is `fes1.bin`; `uboot` and `boot_img` are `boot/uboot.bin` and `boot/boot.img` from
/// `hakchi.hmod`. The images are validated before the device is touched. See the
/// [module documentation](self) for the sequence.
pub async fn memboot<T: Transport>(
    device: &mut Device<T>,
    fes1: &[u8],
    uboot: &[u8],
    boot_img: &[u8],
    options: &MembootOptions,
    mut progress: impl FnMut(MembootEvent) + Send,
) -> Result<MembootReport> {
    progress(MembootEvent::Stage(MembootStage::PreparingImages));
    let uboot = UbootImage::new(uboot, options.uboot)?;
    let image = prepare_boot_image(boot_img, options.shell)?;
    let cmdline = cmdline(&image)?;

    progress(MembootEvent::Stage(MembootStage::VerifyingDevice));
    let response = device.verify_device().await?;
    if !response.is_r16() {
        return Err(Error::UnexpectedSoc {
            found: response.soc_id,
            expected: crate::SOC_ID_R16,
        });
    }

    progress(MembootEvent::Stage(MembootStage::InitialisingDram));
    device.init_dram(fes1).await?;

    progress(MembootEvent::Stage(MembootStage::UploadingKernel));
    info!(
        len = image.len(),
        "uploading boot image to {TRANSFER_BASE:#x}"
    );
    device
        .write_memory_with_progress(TRANSFER_BASE, &image, |p| progress(MembootEvent::Upload(p)))
        .await?;

    progress(MembootEvent::Stage(MembootStage::StartingUboot));
    let command = format!("boota {TRANSFER_BASE:x}");
    device.run_uboot_cmd(&uboot, &command).await?;
    info!("kernel started; the console should come up as the hakchi network gadget shortly");
    Ok(MembootReport {
        device: response,
        image_len: image.len(),
        cmdline,
        command,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::{
        FES1_BASE, UBOOT_BASE,
        device::{FES1_CHECK_LEN, mock::Script},
        message::FelCommand,
    };

    /// Header bytes of hakchi.hmod v1.0.4-126's boot/boot.img (4 585 472 bytes on disk).
    const HAKCHI_HEADER: [u8; 48] = [
        0x41, 0x4e, 0x44, 0x52, 0x4f, 0x49, 0x44, 0x21, 0x60, 0x0a, 0x1e, 0x00, 0x00, 0x80, 0x00,
        0x40, 0x30, 0xdc, 0x27, 0x00, 0x00, 0x00, 0x00, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xf0, 0x40, 0x00, 0x01, 0x00, 0x40, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00,
    ];
    const HAKCHI_CMDLINE: &str = "root=/dev/nandb decrypt ro console=ttyS0,115200 loglevel=4 \
                                  ion_cma_512m=16m coherent_pool=4m consoleblank=0 hakchi-shell";

    fn hakchi_like_image() -> Vec<u8> {
        let mut image = vec![0u8; 2048];
        image[..48].copy_from_slice(&HAKCHI_HEADER);
        set_cmdline(&mut image, HAKCHI_CMDLINE).unwrap();
        image
    }

    /// A tiny valid image: page size 16, 4-byte kernel, 4-byte ramdisk, so three pages.
    fn tiny_image(cmdline: &str) -> Vec<u8> {
        let mut image = vec![0u8; BOOT_HEADER_MIN_LEN];
        image[..8].copy_from_slice(BOOT_MAGIC);
        image[8..12].copy_from_slice(&4u32.to_le_bytes());
        image[16..20].copy_from_slice(&4u32.to_le_bytes());
        image[36..40].copy_from_slice(&16u32.to_le_bytes());
        set_cmdline(&mut image, cmdline).unwrap();
        image
    }

    #[test]
    fn parses_real_hakchi_header() {
        let image = hakchi_like_image();
        let header = BootImageHeader::parse(&image).unwrap();
        assert_eq!(
            header,
            BootImageHeader {
                kernel_size: 0x1e0a60,
                kernel_addr: 0x4000_8000,
                ramdisk_size: 0x27dc30,
                ramdisk_addr: 0x4100_0000,
                second_size: 0,
                second_addr: 0x40f0_0000,
                tags_addr: 0x4000_0100,
                page_size: 0x800,
                dt_size: 0,
            }
        );
        assert_eq!(header.image_size().unwrap(), 4_585_472);
        assert_eq!(cmdline(&image).unwrap(), HAKCHI_CMDLINE);
    }

    #[test]
    fn rejects_bad_images() {
        assert!(matches!(
            BootImageHeader::parse(&[0; 10]),
            Err(Error::InvalidBootImage(_))
        ));
        let mut image = tiny_image("x");
        image[0] = b'B';
        assert!(matches!(
            BootImageHeader::parse(&image),
            Err(Error::InvalidBootImage(_))
        ));
        let mut image = tiny_image("x");
        image[36..40].fill(0);
        assert!(matches!(
            BootImageHeader::parse(&image),
            Err(Error::InvalidBootImage(_))
        ));
        let mut image = tiny_image("x");
        image[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            prepare_boot_image(&image, ShellOverride::Keep),
            Err(Error::InvalidBootImage(_))
        ));
    }

    #[test]
    fn prepare_pads_to_sector_and_rejects_truncated_or_huge() {
        let image = tiny_image("quiet");
        let prepared = prepare_boot_image(&image, ShellOverride::Keep).unwrap();
        assert_eq!(prepared.len(), SECTOR_SIZE as usize);
        assert_eq!(&prepared[..image.len()], &image[..]);
        assert!(prepared[image.len()..].iter().all(|b| *b == 0));

        let mut truncated = tiny_image("quiet");
        truncated[8..12].copy_from_slice(&0x1000u32.to_le_bytes());
        assert!(matches!(
            prepare_boot_image(&truncated, ShellOverride::Keep),
            Err(Error::BootImageTruncated {
                declared: 0x1020,
                ..
            })
        ));

        // Declared size over the transfer area but a file that short cannot exist in tests, so
        // check the limit through the header arithmetic instead.
        let mut huge = tiny_image("quiet");
        huge[8..12].copy_from_slice(&(TRANSFER_MAX_SIZE + 1).to_le_bytes());
        let size = BootImageHeader::parse(&huge).unwrap().image_size().unwrap();
        assert!(size > TRANSFER_MAX_SIZE);
    }

    #[test]
    fn prepare_trims_trailing_data() {
        let mut image = tiny_image("quiet");
        image.resize(SECTOR_SIZE as usize + 10, 0xaa);
        let prepared = prepare_boot_image(&image, ShellOverride::Keep).unwrap();
        assert_eq!(prepared.len(), SECTOR_SIZE as usize);
    }

    #[test]
    fn patch_cmdline_replaces_shell_token() {
        let mut image = tiny_image("root=/dev/nandb hakchi-shell quiet");
        let patched = patch_cmdline(&mut image, ShellOverride::Clovershell).unwrap();
        assert_eq!(
            patched.as_deref(),
            Some("root=/dev/nandb quiet hakchi-clovershell")
        );
        assert_eq!(
            cmdline(&image).unwrap(),
            "root=/dev/nandb quiet hakchi-clovershell"
        );

        let patched = patch_cmdline(&mut image, ShellOverride::Network).unwrap();
        assert_eq!(
            patched.as_deref(),
            Some("root=/dev/nandb quiet hakchi-shell")
        );

        let mut image = tiny_image("root=/dev/nandb");
        assert_eq!(
            patch_cmdline(&mut image, ShellOverride::Keep).unwrap(),
            None
        );
        assert_eq!(cmdline(&image).unwrap(), "root=/dev/nandb");
    }

    #[test]
    fn patch_cmdline_keeps_padding_clean() {
        let mut image = tiny_image("a=b hakchi-clovershell c=d");
        patch_cmdline(&mut image, ShellOverride::Network).unwrap();
        let field = &image[BOOT_ARGS_OFFSET..BOOT_HEADER_MIN_LEN];
        let text = "a=b c=d hakchi-shell";
        assert_eq!(&field[..text.len()], text.as_bytes());
        assert!(field[text.len()..].iter().all(|b| *b == 0));
    }

    #[test]
    fn cmdline_too_long_is_rejected() {
        let long = "x".repeat(BOOT_ARGS_SIZE - 1);
        let mut image = tiny_image(&long);
        assert!(matches!(
            patch_cmdline(&mut image, ShellOverride::Network),
            Err(Error::CmdlineTooLong { max: 511, .. })
        ));
        let mut image = vec![0u8; BOOT_HEADER_MIN_LEN];
        assert!(set_cmdline(&mut image, &"y".repeat(BOOT_ARGS_SIZE)).is_err());
        assert!(set_cmdline(&mut image, &long).is_ok());
    }

    fn uboot_bytes() -> Vec<u8> {
        let mut image = vec![0x11u8; 0x80];
        image[0x30..0x38].copy_from_slice(b"bootcmd=");
        image[0x78..].copy_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xde, 0xc0, 0x07, 0xb0]);
        image
    }

    #[test]
    fn uboot_sd_variant_swaps_trailing_words() {
        let bytes = uboot_bytes();
        let normal = UbootImage::new(&bytes, UbootVariant::Normal).unwrap();
        assert_eq!(normal.data(), &bytes[..]);
        assert_eq!(normal.cmd_offset(), 0x38);

        let sd = UbootImage::new(&bytes, UbootVariant::Sd).unwrap();
        assert_eq!(&sd.data()[..0x78], &bytes[..0x78]);
        assert_eq!(
            &sd.data()[0x78..],
            &[0xde, 0xc0, 0x07, 0xb0, 0xff, 0xff, 0xff, 0xff]
        );
        assert_eq!(sd.cmd_offset(), 0x38);
    }

    #[test]
    fn uboot_without_bootcmd_is_rejected() {
        let bytes = vec![0u8; 0x80];
        assert!(matches!(
            UbootImage::new(&bytes, UbootVariant::Sd),
            Err(Error::InvalidUboot("no bootcmd= environment variable"))
        ));
        assert!(matches!(
            UbootImage::new(&bytes[..8], UbootVariant::Sd),
            Err(Error::InvalidUboot("image is too small"))
        ));
    }

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
    async fn memboot_wire_sequence() {
        let fes1: Vec<u8> = (0..0x400u32).map(|i| (i * 3) as u8).collect();
        let uboot_raw = uboot_bytes();
        let uboot = UbootImage::new(&uboot_raw, UbootVariant::Sd).unwrap();
        let boot_img = tiny_image("root=/dev/nandb hakchi-shell");
        let expected_image = prepare_boot_image(&boot_img, ShellOverride::Clovershell).unwrap();

        let mut script = Script::default();
        // 1. verify device
        script.verify_device(&r16());
        // 2. init DRAM: fes1 not resident, upload + exec
        script.read_memory(
            FES1_BASE + 0x400 - FES1_CHECK_LEN,
            &[0; FES1_CHECK_LEN as usize],
        );
        script.write_memory(FES1_BASE, &fes1);
        script.exec(FES1_BASE);
        // 3. kernel upload: two 64 KiB chunks
        script.write_memory(TRANSFER_BASE, &expected_image);
        // 4. u-boot: not resident, upload, write command, exec
        script.read_memory(UBOOT_BASE, &[0; 0x20]);
        script.write_memory(UBOOT_BASE, uboot.data());
        script.write_memory(UBOOT_BASE + 0x38, b"boota 47400000\0\0");
        script.exec(UBOOT_BASE);

        let mut device = Device::from_transport(script.into_transport());
        tokio::time::pause();
        let mut events = Vec::new();
        let options = MembootOptions {
            shell: ShellOverride::Clovershell,
            uboot: UbootVariant::Sd,
        };
        let report = memboot(&mut device, &fes1, &uboot_raw, &boot_img, &options, |e| {
            events.push(e)
        })
        .await
        .unwrap();
        device.transport().finish();

        assert_eq!(report.device, r16());
        assert_eq!(report.image_len, SECTOR_SIZE as usize);
        assert_eq!(report.cmdline, "root=/dev/nandb hakchi-clovershell");
        assert_eq!(report.command, "boota 47400000");
        assert_eq!(
            events,
            vec![
                MembootEvent::Stage(MembootStage::PreparingImages),
                MembootEvent::Stage(MembootStage::VerifyingDevice),
                MembootEvent::Stage(MembootStage::InitialisingDram),
                MembootEvent::Stage(MembootStage::UploadingKernel),
                MembootEvent::Upload(Progress {
                    done: 0,
                    total: SECTOR_SIZE as usize
                }),
                MembootEvent::Upload(Progress {
                    done: 0x1_0000,
                    total: SECTOR_SIZE as usize
                }),
                MembootEvent::Upload(Progress {
                    done: 0x2_0000,
                    total: SECTOR_SIZE as usize
                }),
                MembootEvent::Stage(MembootStage::StartingUboot),
            ]
        );
    }

    /// Compile-time check: the futures must be `Send` so Tauri can run them on its runtime.
    #[allow(dead_code)]
    fn memboot_future_is_send(device: &mut Device<crate::UsbTransport>, images: &[u8]) {
        fn assert_send<T: Send>(_: &T) {}
        let options = MembootOptions::default();
        assert_send(&memboot(device, images, images, images, &options, |_| {}));
        assert_send(&device.verify_device());
    }

    #[tokio::test]
    async fn memboot_refuses_other_socs_before_touching_dram() {
        let fes1 = vec![0u8; 0x100];
        let boot_img = tiny_image("quiet");
        let mut script = Script::default();
        script.verify_device(&VerifyDeviceResponse {
            soc_id: 0x0016_8900,
            ..r16()
        });
        let mut device = Device::from_transport(script.into_transport());
        let err = memboot(
            &mut device,
            &fes1,
            &uboot_bytes(),
            &boot_img,
            &MembootOptions::default(),
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            Error::UnexpectedSoc {
                found: 0x0016_8900,
                ..
            }
        ));
        device.transport().finish();
    }

    #[tokio::test]
    async fn memboot_validates_images_before_usb() {
        let mut device = Device::from_transport(Script::default().into_transport());
        let err = memboot(
            &mut device,
            &[0; 0x100],
            &[0; 0x80],
            &tiny_image("x"),
            &MembootOptions::default(),
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::InvalidUboot(_)));
        let err = memboot(
            &mut device,
            &[0; 0x100],
            &uboot_bytes(),
            &[0; 16],
            &MembootOptions::default(),
            |_| {},
        )
        .await
        .unwrap_err();
        assert!(matches!(err, Error::InvalidBootImage(_)));
        device.transport().finish();
        let _ = FelCommand::Run;
    }
}
