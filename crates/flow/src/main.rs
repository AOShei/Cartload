//! `cartload-flow`: run the kernel install, memboot, uninstall and dump flows from the
//! command line, printing each step, its outcome and its timing.

use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use cartload_flow::{
    Context, Event, Flow, FlowOptions, HmodPackage, Level, Outcome, Reporter, UsbBackend, channel,
    flows,
};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "cartload-flow",
    version,
    about = "Install, memboot, uninstall or dump the hakchi kernel on a NES / SNES / Mega \
             Drive Classic Mini",
    after_help = "A failed flow can be rerun with the same command: finished steps are \
                  skipped.\nDo not leave a flow running unattended; it needs the console \
                  put in FEL mode (hold Reset while powering on) when nothing answers on \
                  the USB network."
)]
struct Cli {
    /// Show debug output (RUST_LOG overrides this).
    #[arg(short, long, global = true)]
    verbose: bool,

    /// List the steps and validate the inputs without touching USB.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Seconds to wait for a console in FEL mode.
    #[arg(long, global = true, value_name = "SECS", default_value_t = 120)]
    wait_fel: u64,

    /// Seconds to wait for the console on the USB network after a memboot.
    #[arg(long, global = true, value_name = "SECS", default_value_t = 120)]
    wait_console: u64,

    /// Seconds to wait for the console to come back after a reboot or kexec.
    #[arg(long, global = true, value_name = "SECS", default_value_t = 180)]
    wait_reboot: u64,

    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct HmodArg {
    /// hakchi.hmod (gzip tar with boot/boot.img, boot/uboot.bin and var/version).
    #[arg(long, value_name = "PATH")]
    hmod: PathBuf,
}

#[derive(Args)]
struct OptionalHmodArg {
    /// hakchi.hmod; needed only when the console is not running a hakchi kernel (FEL
    /// memboot).
    #[arg(long, value_name = "PATH")]
    hmod: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Install or repair the hakchi kernel: memboot, grow the data partition, upload the
    /// package, run the installer, reboot, verify.
    Install {
        #[command(flatten)]
        hmod: HmodArg,
    },
    /// Memboot only (recovery): leave the console in the memboot shell with SSH open.
    Memboot {
        #[command(flatten)]
        hmod: HmodArg,
    },
    /// Return the console to the stock firmware: flash the stock kernel back, remove the
    /// hakchi files, restore u-boot, shrink the data partition, reboot.
    Uninstall {
        #[command(flatten)]
        hmod: OptionalHmodArg,

        /// Stock kernel dump to flash when the console has no backup of its own.
        #[arg(long, value_name = "FILE")]
        stock_kernel: Option<PathBuf>,

        /// Flash --stock-kernel even when its MD5 is not a known stock image.
        #[arg(long)]
        force_stock_kernel: bool,
    },
    /// Write the stock kernel backup to a file, then reboot.
    DumpKernel {
        #[command(flatten)]
        hmod: OptionalHmodArg,

        /// Output file (kernel.img).
        out: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(if cli.verbose { "debug" } else { "warn" }));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let mut options = FlowOptions {
        fel_wait: Duration::from_secs(cli.wait_fel),
        console_wait: Duration::from_secs(cli.wait_console),
        reboot_wait: Duration::from_secs(cli.wait_reboot),
        ..FlowOptions::default()
    };
    let (flow, hmod) = match cli.command {
        Command::Install { hmod } => (flows::install_kernel(), Some(hmod.hmod)),
        Command::Memboot { hmod } => (flows::memboot(), Some(hmod.hmod)),
        Command::Uninstall {
            hmod,
            stock_kernel,
            force_stock_kernel,
        } => {
            options.stock_kernel = stock_kernel;
            options.force_stock_kernel = force_stock_kernel;
            (flows::uninstall_kernel(), hmod.hmod)
        }
        Command::DumpKernel { hmod, out } => (flows::dump_stock_kernel(out), hmod.hmod),
    };

    let package = match hmod {
        Some(path) => Some(
            HmodPackage::load(&path)
                .map_err(|error| anyhow::anyhow!("reading {}: {error}", path.display()))?,
        ),
        None => None,
    };
    if let Some(package) = &package {
        println!(
            "package:  {} ({} bytes, md5 {})",
            package.path.display(),
            package.bytes.len(),
            package.md5
        );
        println!("version:  {}", package.files.version);
    }
    let steps = flow.steps();
    println!("flow:     {} ({} steps)", flow.name(), steps.len());
    if cli.dry_run {
        for step in &steps {
            println!(
                "  [{:>2}/{}] {:<18} {}",
                step.index + 1,
                steps.len(),
                step.name,
                step.description
            );
        }
        println!("dry run: inputs are valid, not touching USB");
        return Ok(());
    }

    let (sender, receiver) = channel();
    let printer = tokio::spawn(print_events(receiver, steps.len()));
    let mut context = Context::new(
        Box::new(UsbBackend::default()),
        Reporter::new(sender.clone()),
        options,
    );
    if let Some(package) = package {
        context = context.with_hmod(package);
    }
    let mut flow: Flow<Context> = flow.with_events(sender);
    let result = flow.run(&mut context).await;
    context.disconnect().await;
    drop(flow);
    drop(context);
    let _ = printer.await;

    match result {
        Ok(report) => {
            println!(
                "{} finished: {} steps in {}",
                report.flow,
                report.results.len(),
                human_duration(report.elapsed)
            );
            Ok(())
        }
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!("rerun the same command to resume; finished steps are skipped");
            std::process::exit(1);
        }
    }
}

async fn print_events(mut receiver: cartload_flow::EventReceiver, total: usize) {
    let mut last_progress: Option<(String, u64)> = None;
    while let Some(event) = receiver.recv().await {
        match event {
            Event::FlowStarted { resume_from, .. } => {
                if resume_from > 0 {
                    println!("resuming from step {}", resume_from + 1);
                }
            }
            Event::StepStarted { step } => {
                println!(
                    "[{:>2}/{}] {}: {}",
                    step.index + 1,
                    total,
                    step.name,
                    step.description
                );
                last_progress = None;
            }
            Event::StepFinished {
                outcome, elapsed, ..
            } => {
                let text = match outcome {
                    Outcome::Done => "done".to_string(),
                    other => other.to_string(),
                };
                println!("        => {text} ({})", human_duration(elapsed));
            }
            Event::StepFailed { error, elapsed, .. } => {
                println!(
                    "        => FAILED after {}: {error}",
                    human_duration(elapsed)
                );
            }
            Event::Log { level, message } => {
                let prefix = match level {
                    Level::Debug => continue,
                    Level::Info => "",
                    Level::Warn => "warning: ",
                };
                println!("        {prefix}{message}");
            }
            Event::Progress { label, done, total } => {
                let percent = (done * 100).checked_div(total).unwrap_or(100);
                let bucket = percent / 25;
                let changed = last_progress
                    .as_ref()
                    .is_none_or(|(l, b)| *l != label || *b != bucket);
                if changed || done == total {
                    println!("        {label}: {done} / {total} bytes ({percent}%)");
                    last_progress = Some((label, bucket));
                }
            }
            Event::FlowFinished { .. } => {}
        }
    }
}

fn human_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs >= 60.0 {
        format!("{} min {:.0} s", (secs / 60.0).floor(), secs % 60.0)
    } else {
        format!("{secs:.1} s")
    }
}
