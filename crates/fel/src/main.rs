//! `cartload-fel`: probe a console in FEL mode or memboot a hakchi kernel, without the UI.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use cartload_fel::{
    Device, DeviceInfo, Error, MembootEvent, MembootOptions, MembootStage, ShellOverride,
    UbootImage, UbootVariant, find_devices, memboot, wait_for_device,
};
use cartload_hmod::Package;
use clap::{Parser, Subcommand, ValueEnum};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "cartload-fel",
    version,
    about = "Allwinner FEL tool for the NES / SNES / Mega Drive Classic Mini"
)]
struct Cli {
    /// Show debug output (RUST_LOG overrides this).
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Look for a console in FEL mode and print its boot ROM version information.
    Probe {
        /// Keep polling for up to this many seconds for a FEL device to appear.
        #[arg(long, value_name = "SECS")]
        wait: Option<u64>,
    },
    /// Boot the hakchi kernel from RAM. Nothing is written to the console's NAND. The DRAM init
    /// binary (fes1.bin) is built in.
    Memboot {
        /// hakchi.hmod (gzip tar) providing boot/boot.img and boot/uboot.bin.
        #[arg(long, value_name = "PATH")]
        hmod: PathBuf,

        /// Keep polling for up to this many seconds for a FEL device to appear.
        #[arg(long, value_name = "SECS")]
        wait: Option<u64>,

        /// Kernel command line patch for the shell type.
        #[arg(long, value_enum, default_value_t = ShellArg::Keep)]
        shell: ShellArg,

        /// u-boot flavour: hakchi2 CE memboots with the SD variant.
        #[arg(long, value_enum, default_value_t = UbootArg::Sd)]
        uboot: UbootArg,

        /// Validate and describe the images, then stop without touching USB.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ShellArg {
    /// Leave the command line as shipped in boot.img.
    Keep,
    /// Force the network shell (hakchi-shell).
    Network,
    /// Force the legacy clovershell protocol (hakchi-clovershell).
    Clovershell,
}

impl From<ShellArg> for ShellOverride {
    fn from(value: ShellArg) -> Self {
        match value {
            ShellArg::Keep => Self::Keep,
            ShellArg::Network => Self::Network,
            ShellArg::Clovershell => Self::Clovershell,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum UbootArg {
    /// uboot.bin exactly as shipped.
    Normal,
    /// Trailing marker words swapped (SD card support), what hakchi2 CE uses.
    Sd,
}

impl From<UbootArg> for UbootVariant {
    fn from(value: UbootArg) -> Self {
        match value {
            UbootArg::Normal => Self::Normal,
            UbootArg::Sd => Self::Sd,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(if cli.verbose { "debug" } else { "info" }));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Command::Probe { wait } => probe(wait).await,
        Command::Memboot {
            hmod,
            wait,
            shell,
            uboot,
            dry_run,
        } => {
            let options = MembootOptions {
                shell: shell.into(),
                uboot: uboot.into(),
            };
            run_memboot(&hmod, wait, options, dry_run).await
        }
    }
}

async fn locate(wait: Option<u64>) -> Result<DeviceInfo> {
    match wait {
        Some(secs) => {
            info!("waiting up to {secs} s for a FEL device (hold Reset while powering on)");
            Ok(wait_for_device(Duration::from_secs(secs)).await?)
        }
        None => Ok(find_devices()
            .await?
            .into_iter()
            .next()
            .ok_or(Error::DeviceNotFound)?),
    }
}

fn describe(info: &DeviceInfo) -> String {
    format!(
        "bus {} device {:03}, {} speed, manufacturer {:?}, product {:?}, serial {:?}",
        info.bus_id(),
        info.device_address(),
        info.speed()
            .map_or("unknown".to_string(), |s| format!("{s:?}").to_lowercase()),
        info.manufacturer_string().unwrap_or(""),
        info.product_string().unwrap_or(""),
        info.serial_number().unwrap_or(""),
    )
}

async fn probe(wait: Option<u64>) -> Result<()> {
    let info = locate(wait).await?;
    println!(
        "FEL device {:04x}:{:04x}: {}",
        info.vendor_id(),
        info.product_id(),
        describe(&info)
    );
    let mut device = Device::open_info(&info).await?;
    let (ep_in, ep_out) = device.transport().endpoints();
    println!("  bulk endpoints: IN {ep_in:#04x}, OUT {ep_out:#04x}");
    let response = device.verify_device().await?;
    println!("  {response}");
    if response.is_r16() {
        println!("  SoC is the Allwinner R16 used by the Classic Mini consoles: OK");
    } else {
        println!("  SoC is NOT the Allwinner R16; this is not a Classic Mini console");
    }
    Ok(())
}

async fn run_memboot(
    hmod: &Path,
    wait: Option<u64>,
    options: MembootOptions,
    dry_run: bool,
) -> Result<()> {
    let fes1_bytes = cartload_hmod::fes1();
    let files = Package::open(hmod)
        .and_then(|package| package.boot_files())
        .with_context(|| format!("reading {}", hmod.display()))?;
    info!(
        boot_img = files.boot_image.len(),
        uboot = files.uboot.len(),
        version = %files.version,
        "read boot files from hmod"
    );

    // Validate everything before looking for the device, and describe what will happen.
    let uboot = UbootImage::new(&files.uboot, options.uboot)?;
    let header = cartload_fel::memboot::BootImageHeader::parse(&files.boot_image)?;
    let prepared = cartload_fel::memboot::prepare_boot_image(&files.boot_image, options.shell)?;
    println!("hmod:      {}", hmod.display());
    println!("version:   {}", files.version);
    println!("fes1.bin:  {} bytes, built in", fes1_bytes.len());
    println!(
        "uboot.bin: {} bytes, {:?} variant, bootcmd value at offset {:#x}",
        uboot.data().len(),
        options.uboot,
        uboot.cmd_offset()
    );
    println!(
        "boot.img:  {} bytes on disk, {} declared (kernel {} + ramdisk {}, page {}), {} uploaded",
        files.boot_image.len(),
        header.image_size()?,
        header.kernel_size,
        header.ramdisk_size,
        header.page_size,
        prepared.len()
    );
    println!("cmdline:   {}", cartload_fel::memboot::cmdline(&prepared)?);
    if dry_run {
        println!("dry run: images are valid, not touching USB");
        return Ok(());
    }

    let info = locate(wait).await?;
    println!(
        "FEL device {:04x}:{:04x}: {}",
        info.vendor_id(),
        info.product_id(),
        describe(&info)
    );
    let mut device = Device::open_info(&info).await?;
    let mut last_percent = 0;
    let report = memboot(
        &mut device,
        fes1_bytes,
        &files.uboot,
        &files.boot_image,
        &options,
        |event| match event {
            MembootEvent::Stage(stage) => info!(
                "{}",
                match stage {
                    MembootStage::PreparingImages => "preparing images",
                    MembootStage::VerifyingDevice => "verifying device",
                    MembootStage::InitialisingDram => "initialising DRAM",
                    MembootStage::UploadingKernel => "uploading boot image",
                    MembootStage::StartingUboot => "starting u-boot",
                }
            ),
            MembootEvent::Upload(p) => {
                let percent = (p.done * 100).checked_div(p.total).unwrap_or(100);
                if percent / 10 > last_percent / 10 || p.done == p.total {
                    last_percent = percent;
                    info!("uploaded {} / {} bytes ({percent}%)", p.done, p.total);
                }
            }
        },
    )
    .await?;
    println!(
        "memboot done: {} bytes uploaded, ran `{}`",
        report.image_len, report.command
    );
    println!(
        "the console should enumerate as 04e8:6863 within about 10 s and answer SSH at \
         169.254.13.37 (root, empty password)"
    );
    if !report.cmdline.contains("hakchi-shell") {
        warn!("command line has no hakchi-shell token; the network shell may not start");
    }
    Ok(())
}
