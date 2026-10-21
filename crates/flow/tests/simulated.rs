//! The flows against a simulated console.
//!
//! `Sim` is a small state machine that answers the exact commands the steps send the way the
//! device does (see `docs/device-protocol.md` and hakchi2 CE's `MembootTasks.cs`), and
//! changes state on FEL memboot, kexec, the device-side installer (`boot`) and `reboot`.
//! Time is paused, so every wait is instant.

#![allow(clippy::unwrap_used)]

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use cartload_console::{ExecOutput, Versions, commands as console_commands, parse::sh_quote};
use cartload_fel::{MembootOptions, MembootReport, VerifyDeviceResponse};
use cartload_flow::{
    Backend, Console, Context, Error, Event, Flow, FlowOptions, HmodPackage, Outcome, Reporter,
    channel, flows, steps::commands, tools,
};
use cartload_hmod::{
    BootFiles, Version,
    basemods::{self, LoadedMod},
};

const ADDRESS: &str = "169.254.13.37:22";
const ROOTFS: &str = "/var/lib/hakchi/rootfs";
const HWID: &str = "0461872a 87280207 9d2818c7 6c118000";
const STOCK_KERNEL_LEN: usize = 4096;
const OFFLINE_POLLS: usize = 3;

/// What is in the kernel partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NandKernel {
    Stock,
    Hakchi,
}

/// How the console comes back after it drops off the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComeBack {
    MembootShell,
    FullBoot,
    Stays,
}

#[derive(Debug)]
struct Sim {
    online: bool,
    memboot_shell: bool,
    installed: bool,
    fel_present: bool,
    md: bool,
    udisk: bool,
    nand_kernel: NandKernel,
    backup: Option<Vec<u8>>,
    install_dir: bool,
    cf_install: bool,
    cf_update: bool,
    transfer_dir: bool,
    mounted: bool,
    files: BTreeMap<String, Vec<u8>>,
    /// Uploaded files that were `chmod +x`'d.
    executable: BTreeSet<String>,
    offline_polls_left: usize,
    come_back: ComeBack,
    generation: u64,
    log: Vec<String>,
    /// Fault injection: the next hmod upload chunk fails.
    fail_hmod_upload: bool,
    /// Fault injection: the next `chmod +x` of the kexec tools fails.
    fail_kexec_chmod: bool,
    /// Fault injection: after a memboot (FEL or kexec) the console comes back like this
    /// instead of in the memboot shell.
    memboot_comes_back: Option<ComeBack>,
    fel_memboots: usize,
    kexecs: usize,
    boots: usize,
    installed_versions: Versions,
    /// Files in the hakchi rootfs overlay (absolute paths), as `pack_install` copies them.
    rootfs_files: BTreeMap<String, Vec<u8>>,
    /// Mods with an `uninstall-<name>` file in `/var/lib/hakchi/hmod`.
    mods_registered: BTreeSet<String>,
    /// Whether a reboot happened since the last mod install (new files are visible to
    /// `which` only then).
    mods_live: bool,
}

impl Sim {
    fn stock() -> Self {
        Self {
            online: false,
            memboot_shell: false,
            installed: false,
            fel_present: false,
            md: false,
            udisk: true,
            nand_kernel: NandKernel::Stock,
            backup: None,
            install_dir: false,
            cf_install: false,
            cf_update: false,
            transfer_dir: false,
            mounted: false,
            files: BTreeMap::new(),
            executable: BTreeSet::new(),
            offline_polls_left: 0,
            come_back: ComeBack::Stays,
            generation: 0,
            log: Vec::new(),
            fail_hmod_upload: false,
            fail_kexec_chmod: false,
            memboot_comes_back: None,
            fel_memboots: 0,
            kexecs: 0,
            boots: 0,
            installed_versions: package_versions(),
            rootfs_files: BTreeMap::new(),
            mods_registered: BTreeSet::new(),
            mods_live: true,
        }
    }

    fn installed() -> Self {
        Self {
            online: true,
            installed: true,
            udisk: false,
            nand_kernel: NandKernel::Hakchi,
            backup: Some(stock_kernel()),
            install_dir: true,
            ..Self::stock()
        }
    }

    fn go_offline(&mut self, come_back: ComeBack) {
        self.online = false;
        self.memboot_shell = false;
        self.mounted = false;
        self.offline_polls_left = OFFLINE_POLLS;
        self.come_back = come_back;
        self.generation += 1;
    }

    fn memboot(&mut self) {
        let come_back = self.memboot_comes_back.unwrap_or(ComeBack::MembootShell);
        self.go_offline(come_back);
    }

    fn poll(&mut self) -> bool {
        if self.online {
            return true;
        }
        if self.offline_polls_left > 0 {
            self.offline_polls_left -= 1;
            if self.offline_polls_left == 0 {
                match self.come_back {
                    ComeBack::MembootShell => {
                        self.online = true;
                        self.memboot_shell = true;
                        self.cf_install = false;
                        self.cf_update = false;
                        self.transfer_dir = false;
                        self.files.clear();
                        self.executable.clear();
                    }
                    ComeBack::FullBoot => {
                        self.online = true;
                        self.memboot_shell = false;
                    }
                    ComeBack::Stays => {}
                }
                self.generation += 1;
            }
        }
        self.online
    }

    /// What `detached-fallback recovery` needs when the launcher starts it: hakchi2 CE's
    /// `kexec` and the script itself uploaded unchanged and executable, and a launcher that
    /// runs the uploaded script from the scratch directory, never `hakchi recovery`.
    fn check_kexec_tools(&self) {
        for (path, expected) in [
            (commands::KEXEC_BINARY, tools::kexec_static()),
            (commands::DETACHED_FALLBACK, tools::detached_fallback()),
        ] {
            assert_eq!(
                self.files.get(path).map(Vec::as_slice),
                Some(expected),
                "{path} uploaded unchanged"
            );
            assert!(self.executable.contains(path), "{path} executable");
        }
        let launcher = self
            .files
            .get(commands::KEXEC_SCRIPT)
            .expect("launcher uploaded");
        let launcher = std::str::from_utf8(launcher).unwrap();
        assert!(launcher.starts_with("#!/bin/sh\n"), "{launcher}");
        assert!(launcher.contains("cd /tmp/cartload"), "{launcher}");
        assert!(
            launcher.contains("/bin/sh /tmp/cartload/detached-fallback recovery \"$@\""),
            "{launcher}"
        );
        assert!(!launcher.contains("hakchi recovery"), "{launcher}");
    }

    fn exec(&mut self, command: &str, stdin: Option<&[u8]>) -> ExecOutput {
        self.log.push(command.to_string());
        let hmod_path = sh_quote(commands::HMOD_PATH);
        let games = "/var/lib/hakchi/games";

        // Uploads: `cat > path` and `cat >> path`.
        if let Some(rest) = command.strip_prefix("cat >") {
            let (append, path) = match rest.strip_prefix('>') {
                Some(path) => (true, path.trim()),
                None => (false, rest.trim()),
            };
            if path == hmod_path && self.fail_hmod_upload {
                self.fail_hmod_upload = false;
                return code(1, "", "cat: write error: No space left on device\n");
            }
            let key = path.trim_matches('\'').to_string();
            let data = stdin.unwrap_or_default().to_vec();
            if append {
                self.files.entry(key).or_default().extend_from_slice(&data);
            } else {
                self.files.insert(key, data);
            }
            return ok("");
        }

        // Base mods: `tar -xvC <dir>` with the hmod's tar on stdin.
        if let Some(name) = ["tiny7zx", "font_remount"]
            .into_iter()
            .find(|name| command == commands::extract_mod(name))
        {
            let mut archive = tar::Archive::new(stdin.unwrap_or_default());
            for entry in archive.entries().unwrap() {
                let mut entry = entry.unwrap();
                let path = entry.path().unwrap().to_string_lossy().into_owned();
                let mut data = Vec::new();
                entry.read_to_end(&mut data).unwrap();
                self.files
                    .insert(format!("{}/{name}.hmod/{path}", commands::MODS_DIR), data);
            }
            return ok("");
        }
        if command == commands::MKDIR_MODS
            || ["tiny7zx", "font_remount"]
                .iter()
                .any(|name| command == commands::mkdir_mod(name))
        {
            return ok("");
        }
        if let Some(name) = ["tiny7zx", "font_remount"]
            .into_iter()
            .find(|name| command == commands::mod_registered("/var/lib/hakchi", name))
        {
            return exit(self.mods_registered.contains(name));
        }
        if command == commands::which("tiny7zx") {
            let present = self
                .rootfs_files
                .contains_key(&format!("{ROOTFS}/bin/tiny7zx"));
            return if present && self.mods_live {
                ok("/bin/tiny7zx\n")
            } else {
                code(1, "", "")
            };
        }
        if command == commands::PACKS_INSTALL {
            assert!(
                self.installed && !self.memboot_shell,
                "mods install on the booted kernel"
            );
            let prefix = format!("{}/", commands::MODS_DIR);
            let uploaded: Vec<(String, Vec<u8>)> = self
                .files
                .iter()
                .filter(|(k, _)| k.starts_with(&prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut out = String::new();
            let mut names = BTreeSet::new();
            for (key, data) in uploaded {
                let rest = &key[prefix.len()..];
                let (dir, rel) = rest.split_once('/').unwrap();
                let name = dir.strip_suffix(".hmod").unwrap().to_string();
                let base = rel.rsplit('/').next().unwrap();
                if !(rel == "install" || rel == "uninstall" || base.starts_with("readme")) {
                    self.rootfs_files.insert(format!("{ROOTFS}/{rel}"), data);
                }
                names.insert(name);
                self.files.remove(&key);
            }
            for name in names {
                out.push_str(&format!(
                    "installing {name}...\ncreating uninstall for {name}...\npackage {name} installed\n"
                ));
                self.mods_registered.insert(name);
            }
            self.mods_live = false;
            return ok(&out);
        }

        match command {
            // Identification (cartload-console's connect sequence).
            console_commands::IS_MD_PARTITIONING => exit(self.md),
            console_commands::IS_MINIMAL_MEMBOOT => {
                if self.memboot_shell {
                    ok("")
                } else if self.installed {
                    code(2, "", "sh: /hakchi/config: No such file or directory\n")
                } else {
                    code(1, "", "")
                }
            }
            console_commands::HWID => ok(&format!("{HWID}\n")),
            console_commands::VERSION_EXISTS => ok(if self.installed { "yes\n" } else { "" }),
            console_commands::VERSIONS => {
                let v = &self.installed_versions;
                ok(&format!("{} {} {}\n", v.boot, v.kernel, v.script))
            }
            console_commands::SYSTEM_CODE => ok("snes-eur\n"),
            console_commands::CURRENT_FIRMWARE => ok("_nand_\n"),
            console_commands::FIND_GAME_SYNC_STORAGE => ok(&format!("{games}\n")),
            console_commands::CONFIG => ok("cfg_cloverapp_home='no'\n"),
            console_commands::STORAGE => ok("361060 | 28860 | 321060\n"),
            console_commands::EXTERNAL_SAVES => code(1, "", ""),
            console_commands::SAVES_SIZE => ok("104\n"),
            console_commands::ORIGINAL_GAMES_PATH => ok("/var/squashfs/usr/share/games\n"),
            c if c == console_commands::get("gamepath") => ok("/usr/share/games\n"),
            c if c == console_commands::get("rootfs") => ok("/var/lib/hakchi/rootfs\n"),
            c if c == console_commands::get("squashfs") => ok("/var/squashfs\n"),
            c if c == console_commands::games_sizes(games) => ok(""),
            c if c == console_commands::dir_exists("/var/squashfs/usr/share/games") => ok("YES\n"),
            c if c == console_commands::list_dirs("/var/squashfs/usr/share/games") => {
                ok("CLV-P-SAAAE/\nCLV-P-SAABE/\n")
            }
            c if c == commands::project_lunar_check("/var/squashfs") => {
                assert!(self.mounted, "the squashfs is reachable after mount_base");
                ok("")
            }
            console_commands::MOUNT_BASE => {
                self.mounted = true;
                ok("")
            }
            console_commands::UMOUNT_BASE => {
                self.mounted = false;
                ok("")
            }

            // kexec from a running kernel: hakchi2 CE's `Memboot` with its own tools.
            commands::HAS_ATAGS => ok(""),
            // No UI to stop in the memboot shell; the flow ignores the failure as CE does.
            commands::UISTOP => exit(!self.memboot_shell),
            commands::UISTART => ok(""),
            commands::MKDIR_SCRATCH => ok(""),
            commands::CHMOD_KEXEC_TOOLS => {
                if self.fail_kexec_chmod {
                    self.fail_kexec_chmod = false;
                    return code(
                        1,
                        "",
                        "chmod: /tmp/cartload/kexec: Operation not permitted\n",
                    );
                }
                for path in [commands::KEXEC_BINARY, commands::DETACHED_FALLBACK] {
                    assert!(
                        self.files.contains_key(path),
                        "{path} uploaded before chmod"
                    );
                    self.executable.insert(path.to_string());
                }
                ok("")
            }
            c if c == commands::launch_kexec(Some(commands::KEXEC_IMAGE)) => {
                self.check_kexec_tools();
                assert!(
                    self.files.contains_key(commands::KEXEC_IMAGE),
                    "image uploaded"
                );
                self.kexecs += 1;
                self.memboot();
                ok("")
            }
            c if c == commands::launch_kexec(None) => {
                self.check_kexec_tools();
                assert!(
                    !self.files.contains_key(commands::KEXEC_IMAGE),
                    "no image uploaded: the kernel in NAND is kexec'd"
                );
                assert_eq!(
                    self.nand_kernel,
                    NandKernel::Hakchi,
                    "NAND kernel can kexec"
                );
                self.kexecs += 1;
                self.memboot();
                ok("")
            }

            // Partition table.
            commands::SUNXI_PART => ok(&partition_table(self.udisk)),
            commands::SUNXI_PART_GROW_DRY_RUN => ok(&partition_table(false)),
            commands::SUNXI_PART_GROW => {
                assert!(self.memboot_shell, "grow runs in the memboot shell");
                self.udisk = false;
                ok("partition table updated\n")
            }
            commands::SNTOOL_ISMD => exit(self.md),

            // Install preparation and the installer.
            commands::INSTALL_FLAGGED => {
                exit(self.cf_install && self.cf_update && self.transfer_dir)
            }
            commands::FLAG_INSTALL => {
                self.cf_install = true;
                ok("")
            }
            commands::FLAG_UPDATE => {
                self.cf_update = true;
                ok("")
            }
            commands::MKDIR_TRANSFER => {
                self.transfer_dir = true;
                ok("")
            }
            c if c == commands::md5sum(commands::HMOD_PATH) => {
                match self.files.get(commands::HMOD_PATH) {
                    Some(data) => ok(&format!(
                        "{:x}  {}\n",
                        md5::compute(data),
                        commands::HMOD_PATH
                    )),
                    None => code(1, "", "md5sum: can't open '/hakchi/transfer/hakchi.hmod'\n"),
                }
            }
            c if c.starts_with("md5sum '") => {
                let mut out = String::new();
                for path in c["md5sum ".len()..]
                    .split(' ')
                    .map(|p| p.trim_matches('\''))
                {
                    match self.rootfs_files.get(path) {
                        Some(data) => out.push_str(&format!("{:x}  {path}\n", md5::compute(data))),
                        None => return code(1, &out, &format!("md5sum: can't open '{path}'\n")),
                    }
                }
                ok(&out)
            }
            commands::BOOT => {
                assert!(self.memboot_shell, "boot runs in the memboot shell");
                self.boots += 1;
                let hmod = self.files.get(commands::HMOD_PATH).cloned();
                if self.cf_install && self.cf_update && hmod.is_some() {
                    if self.nand_kernel == NandKernel::Stock && !self.md {
                        self.backup = Some(stock_kernel());
                    }
                    self.nand_kernel = NandKernel::Hakchi;
                    self.install_dir = true;
                    self.installed = true;
                    self.go_offline(ComeBack::FullBoot);
                    ExecOutput {
                        exit_code: None,
                        stdout: b"installing hakchi...\nbackup boot2\npackage hakchi installed\n"
                            .to_vec(),
                        stderr: Vec::new(),
                    }
                } else {
                    self.go_offline(ComeBack::Stays);
                    ExecOutput {
                        exit_code: None,
                        stdout: b"nothing to install\n".to_vec(),
                        stderr: Vec::new(),
                    }
                }
            }

            // Stock kernel handling.
            commands::GET_BACKUP2 => match (&self.backup, self.md) {
                (Some(backup), false) => ExecOutput {
                    exit_code: Some(0),
                    stdout: backup.clone(),
                    stderr: Vec::new(),
                },
                _ => code(1, "", "whoopsie\n"),
            },
            commands::SNTOOL_CHECK_KERNEL => exit(
                self.files
                    .get(commands::KERNEL_PATH)
                    .is_some_and(|f| f.starts_with(b"ANDROID!")),
            ),
            commands::SNTOOL_KERNEL_INFO => ok("kernel: zImage, ramdisk: cpio\n"),
            commands::FLASH_BOOT2 => {
                let file = self
                    .files
                    .get(commands::KERNEL_PATH)
                    .cloned()
                    .unwrap_or_default();
                assert_eq!(
                    file,
                    stock_kernel(),
                    "the stock kernel is what gets flashed"
                );
                assert!(!self.md);
                self.nand_kernel = NandKernel::Stock;
                ok("")
            }
            commands::IS_ORIGINAL_BOOT2 => exit(self.nand_kernel == NandKernel::Stock),
            commands::INSTALL_DIR_EXISTS => {
                assert!(self.mounted, "data partition mounted first");
                exit(self.install_dir)
            }
            commands::MOD_UNINSTALL => {
                assert!(self.mounted, "mod_uninstall needs mount_base");
                if !self.md {
                    assert_eq!(self.nand_kernel, NandKernel::Stock, "kernel restored first");
                }
                self.install_dir = false;
                self.installed = false;
                self.udisk = true;
                ok("uninstalling hakchi...\ndone.\n")
            }
            commands::REBOOT => {
                let come_back = if self.nand_kernel == NandKernel::Hakchi && self.install_dir {
                    ComeBack::FullBoot
                } else {
                    ComeBack::Stays
                };
                self.mods_live = true;
                self.go_offline(come_back);
                ExecOutput::default()
            }
            other => panic!("the simulated console does not know the command {other:?}"),
        }
    }
}

fn partition_table(udisk: bool) -> String {
    let mut table = String::from("mbr:mbr:32768\nnanda:bootloader:65536\nnandb:rootfs:262144\n");
    if udisk {
        table.push_str("nandc:data:524288\nnandd:UDISK:3145728\n");
    } else {
        table.push_str("nandc:data:3670016\n");
    }
    table
}

fn ok(stdout: &str) -> ExecOutput {
    code(0, stdout, "")
}

fn code(exit_code: u32, stdout: &str, stderr: &str) -> ExecOutput {
    ExecOutput {
        exit_code: Some(exit_code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

fn exit(success: bool) -> ExecOutput {
    code(if success { 0 } else { 1 }, "", "")
}

fn boot_image(fill: u8) -> Vec<u8> {
    let mut image = vec![fill; STOCK_KERNEL_LEN];
    image[..8].copy_from_slice(b"ANDROID!");
    image[8..12].copy_from_slice(&4u32.to_le_bytes());
    image[16..20].copy_from_slice(&4u32.to_le_bytes());
    image[36..40].copy_from_slice(&16u32.to_le_bytes());
    image[64..576].fill(0);
    image
}

fn stock_kernel() -> Vec<u8> {
    boot_image(0x55)
}

fn package_versions() -> Versions {
    Versions {
        boot: "1.0.3".into(),
        kernel: "3.4.113.29-madmonkey".into(),
        script: "v1.0.4-126".into(),
    }
}

fn package() -> HmodPackage {
    let version = Version::parse(
        "bootVersion='1.0.3'\nhakchiVersion='v1.0.4-126'\nkernelVersion='3.4.113.29-madmonkey'\n",
    )
    .unwrap();
    let files = BootFiles {
        boot_image: boot_image(0xaa),
        uboot: vec![0x11; 64],
        version,
    };
    // Larger than two upload chunks so the chunked upload is exercised.
    let bytes = (0..2_500_000u32).map(|i| (i % 251) as u8).collect();
    HmodPackage::from_parts("/tmp/hakchi.hmod", files, bytes)
}

#[derive(Clone)]
struct SimBackend {
    sim: Arc<Mutex<Sim>>,
}

impl SimBackend {
    fn new(sim: Sim) -> Self {
        Self {
            sim: Arc::new(Mutex::new(sim)),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Sim) -> T) -> T {
        f(&mut self.sim.lock().unwrap())
    }

    fn log(&self) -> Vec<String> {
        self.with(|sim| sim.log.clone())
    }

    /// Lets a pending reboot finish, as a user would wait before running the next command.
    fn settle(&self) {
        self.with(|sim| {
            for _ in 0..OFFLINE_POLLS {
                sim.poll();
            }
        });
    }
}

struct SimConsole {
    sim: Arc<Mutex<Sim>>,
    generation: u64,
}

#[async_trait]
impl Console for SimConsole {
    fn address(&self) -> SocketAddr {
        ADDRESS.parse().unwrap()
    }

    async fn exec(
        &self,
        command: &str,
        stdin: Option<&[u8]>,
        _timeout: Duration,
    ) -> cartload_console::Result<ExecOutput> {
        let mut sim = self.sim.lock().unwrap();
        if sim.generation != self.generation || !sim.online {
            return Err(cartload_console::Error::Closed);
        }
        Ok(sim.exec(command, stdin))
    }

    async fn close(self: Box<Self>) {}
}

#[async_trait]
impl Backend for SimBackend {
    async fn discover(&self) -> cartload_flow::Result<Option<SocketAddr>> {
        Ok(self.with(Sim::poll).then(|| ADDRESS.parse().unwrap()))
    }

    async fn connect(&self, _address: SocketAddr) -> cartload_flow::Result<Box<dyn Console>> {
        let generation = self.with(|sim| {
            assert!(sim.online, "connect only after discover");
            sim.generation
        });
        Ok(Box::new(SimConsole {
            sim: Arc::clone(&self.sim),
            generation,
        }))
    }

    async fn fel_device_present(&self) -> cartload_flow::Result<bool> {
        Ok(self.with(|sim| sim.fel_present))
    }

    async fn fel_memboot(
        &self,
        files: &BootFiles,
        _options: &MembootOptions,
        reporter: &Reporter,
    ) -> cartload_flow::Result<MembootReport> {
        reporter.info("simulated FEL memboot");
        self.with(|sim| {
            assert!(sim.fel_present, "FEL device present");
            assert!(!sim.online);
            sim.log.push("<fel memboot>".into());
            sim.fel_memboots += 1;
            sim.fel_present = false;
            sim.memboot();
        });
        Ok(MembootReport {
            device: VerifyDeviceResponse {
                soc_id: cartload_fel::SOC_ID_R16,
                firmware: 1,
                protocol: 1,
                data_flag: 0x44,
                data_length: 8,
                scratchpad: 0x7e00,
            },
            image_len: files.boot_image.len(),
            cmdline: "root=/dev/nandb hakchi-shell".into(),
            command: "boota 47400000".into(),
        })
    }
}

fn options() -> FlowOptions {
    FlowOptions {
        fel_wait: Duration::from_secs(20),
        console_wait: Duration::from_secs(60),
        reboot_wait: Duration::from_secs(90),
        offline_wait: Duration::from_secs(30),
        poll_interval: Duration::from_secs(1),
        ..FlowOptions::default()
    }
}

fn context(
    backend: &SimBackend,
    hmod: Option<HmodPackage>,
) -> (Context, cartload_flow::EventReceiver) {
    let (sender, receiver) = channel();
    let mut ctx = Context::new(Box::new(backend.clone()), Reporter::new(sender), options());
    if let Some(hmod) = hmod {
        ctx = ctx.with_hmod(hmod);
    }
    (ctx, receiver)
}

fn outcomes(flow: &Flow<Context>) -> Vec<(String, String)> {
    flow.results()
        .iter()
        .map(|r| {
            let outcome = match &r.outcome {
                Outcome::Done => "done",
                Outcome::AlreadyDone(_) => "already",
                Outcome::NotNeeded(_) => "not-needed",
            };
            (r.step.name.clone(), outcome.to_string())
        })
        .collect()
}

fn expected(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, o)| (n.to_string(), o.to_string()))
        .collect()
}

/// Asserts that `expected` appears in `log` in order (not necessarily adjacent).
fn assert_in_order(log: &[String], expected: &[&str]) {
    let mut position = 0;
    for want in expected {
        match log[position..].iter().position(|c| c == want) {
            Some(offset) => position += offset + 1,
            None => panic!("{want:?} not found after position {position} in {log:#?}"),
        }
    }
}

fn count(log: &[String], command: &str) -> usize {
    log.iter().filter(|c| c.as_str() == command).count()
}

#[tokio::test(start_paused = true)]
async fn install_from_stock_runs_the_ce_sequence() {
    let backend = SimBackend::new(Sim {
        fel_present: true,
        ..Sim::stock()
    });
    let (mut ctx, mut events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();

    let report = flow.run(&mut ctx).await.expect("install succeeds");

    assert_eq!(
        outcomes(&flow),
        expected(&[
            ("memboot", "done"),
            ("wait-console", "done"),
            ("detect", "done"),
            ("check-md", "not-needed"),
            ("grow", "done"),
            ("reboot-grown", "done"),
            ("wait-console", "done"),
            ("detect", "done"),
            ("prepare", "done"),
            ("upload", "done"),
            ("install", "done"),
            ("wait-console", "done"),
            ("verify", "done"),
        ])
    );
    assert_eq!(report.results.len(), 13);

    let log = backend.log();
    let hmod_path = sh_quote(commands::HMOD_PATH);
    assert_in_order(
        &log,
        &[
            "<fel memboot>",
            console_commands::IS_MD_PARTITIONING,
            console_commands::IS_MINIMAL_MEMBOOT,
            console_commands::HWID,
            commands::SUNXI_PART,
            commands::SUNXI_PART_GROW_DRY_RUN,
            commands::SUNXI_PART_GROW,
            commands::SUNXI_PART,
            commands::SNTOOL_ISMD,
            commands::HAS_ATAGS,
            commands::UISTOP,
            commands::MKDIR_SCRATCH,
            &format!("cat > {}", sh_quote(commands::KEXEC_BINARY)),
            &format!("cat > {}", sh_quote(commands::DETACHED_FALLBACK)),
            commands::CHMOD_KEXEC_TOOLS,
            &format!("cat > {}", sh_quote(commands::KEXEC_IMAGE)),
            &format!("cat > {}", sh_quote(commands::KEXEC_SCRIPT)),
            &commands::launch_kexec(Some(commands::KEXEC_IMAGE)),
            console_commands::HWID,
            commands::INSTALL_FLAGGED,
            commands::FLAG_INSTALL,
            commands::FLAG_UPDATE,
            commands::MKDIR_TRANSFER,
            &commands::md5sum(commands::HMOD_PATH),
            &format!("cat > {hmod_path}"),
            &format!("cat >> {hmod_path}"),
            &format!("cat >> {hmod_path}"),
            &commands::md5sum(commands::HMOD_PATH),
            commands::BOOT,
            console_commands::VERSIONS,
            console_commands::SYSTEM_CODE,
        ],
    );
    assert_eq!(count(&log, commands::SUNXI_PART_GROW), 1);
    assert_eq!(count(&log, commands::BOOT), 1);
    assert_eq!(count(&log, commands::UISTOP), 1, "uistop before the kexec");
    assert_eq!(
        count(&log, commands::UISTART),
        0,
        "nothing failed after uistop"
    );
    assert_eq!(count(&log, &format!("cat > {hmod_path}")), 1);
    assert_eq!(
        count(&log, &format!("cat >> {hmod_path}")),
        2,
        "2.5 MB in 1 MB chunks"
    );
    // The three HandleHakchi(Install) commands are adjacent, in CE's order.
    let flag = log
        .iter()
        .position(|c| c == commands::FLAG_INSTALL)
        .unwrap();
    assert_eq!(log[flag + 1], commands::FLAG_UPDATE);
    assert_eq!(log[flag + 2], commands::MKDIR_TRANSFER);

    backend.with(|sim| {
        assert!(sim.online && sim.installed && !sim.memboot_shell);
        assert_eq!(sim.nand_kernel, NandKernel::Hakchi);
        assert_eq!(
            sim.backup,
            Some(stock_kernel()),
            "the installer backed up the stock kernel"
        );
        assert!(!sim.udisk, "UDISK merged");
        assert_eq!(sim.fel_memboots, 1);
        assert_eq!(sim.kexecs, 1, "one kexec after the grow");
        assert_eq!(sim.boots, 1);
    });

    let mut saw_progress = false;
    let mut saw_log = false;
    while let Ok(event) = events.try_recv() {
        match event {
            Event::Progress { label, done, total } if label == "hakchi.hmod" => {
                saw_progress = true;
                assert!(done <= total);
            }
            Event::Log { message, .. } if message.contains("installed: hakchi v1.0.4-126") => {
                saw_log = true;
            }
            _ => {}
        }
    }
    assert!(saw_progress, "upload progress reported");
    assert!(saw_log, "verify reported the installed version");
}

#[tokio::test(start_paused = true)]
async fn install_resumes_after_a_failed_upload_without_redoing_finished_steps() {
    let backend = SimBackend::new(Sim {
        fel_present: true,
        fail_hmod_upload: true,
        ..Sim::stock()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();

    let error = flow.run(&mut ctx).await.expect_err("upload fails once");
    assert!(
        error.to_string().starts_with("step 10 (upload) failed"),
        "{error}"
    );
    assert!(error.to_string().contains("No space left"), "{error}");
    assert_eq!(flow.position(), 9);
    let commands_before = backend.log().len();

    // In-process resume: continues at the upload step.
    flow.run(&mut ctx).await.expect("resume succeeds");
    let log = backend.log();
    assert_eq!(count(&log[commands_before..], commands::SUNXI_PART_GROW), 0);
    assert_eq!(
        count(&log, commands::FLAG_INSTALL),
        1,
        "prepare not repeated"
    );
    assert_eq!(count(&log, commands::BOOT), 1);
    assert!(flow.is_finished());
    backend.with(|sim| assert!(sim.installed && sim.online));
}

#[tokio::test(start_paused = true)]
async fn install_rerun_from_scratch_skips_finished_work() {
    // A failed run left the console in the memboot shell with the partition grown and the
    // install flagged; a fresh process reruns the whole flow.
    let backend = SimBackend::new(Sim {
        fel_present: true,
        fail_hmod_upload: true,
        ..Sim::stock()
    });
    {
        let (mut ctx, _events) = context(&backend, Some(package()));
        let mut flow = flows::install_kernel();
        flow.run(&mut ctx).await.expect_err("upload fails once");
    }
    let commands_before = backend.log().len();

    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();
    flow.run(&mut ctx).await.expect("second run succeeds");
    assert_eq!(
        outcomes(&flow),
        expected(&[
            ("memboot", "already"),
            ("wait-console", "already"),
            ("detect", "done"),
            ("check-md", "not-needed"),
            ("grow", "already"),
            ("reboot-grown", "already"),
            ("wait-console", "already"),
            ("detect", "done"),
            ("prepare", "already"),
            ("upload", "done"),
            ("install", "done"),
            ("wait-console", "done"),
            ("verify", "done"),
        ])
    );
    let log = backend.log();
    let second = &log[commands_before..];
    assert_eq!(count(second, "<fel memboot>"), 0);
    assert_eq!(count(second, commands::SUNXI_PART_GROW), 0);
    assert_eq!(count(second, commands::FLAG_INSTALL), 0);
    assert_eq!(count(second, commands::BOOT), 1);
    backend.with(|sim| {
        assert_eq!(sim.fel_memboots, 1);
        assert_eq!(sim.kexecs, 1);
        assert!(sim.installed);
    });
}

#[tokio::test(start_paused = true)]
async fn install_over_a_running_hakchi_kernel_uses_kexec_not_fel() {
    let backend = SimBackend::new(Sim::installed());
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();
    flow.run(&mut ctx).await.expect("repair succeeds");
    let names = outcomes(&flow);
    assert_eq!(names[0], ("memboot".to_string(), "done".to_string()));
    assert_eq!(names[4], ("grow".to_string(), "already".to_string()));
    assert_eq!(
        names[5],
        ("reboot-grown".to_string(), "already".to_string())
    );
    backend.with(|sim| {
        assert_eq!(sim.fel_memboots, 0);
        assert_eq!(sim.kexecs, 1, "memboot by kexec from the installed kernel");
        assert_eq!(sim.boots, 1);
        assert!(sim.installed && sim.online);
    });
    assert_in_order(
        &backend.log(),
        &[
            console_commands::VERSIONS,
            commands::HAS_ATAGS,
            commands::UISTOP,
            commands::MKDIR_SCRATCH,
            &format!("cat > {}", sh_quote(commands::KEXEC_BINARY)),
            &format!("cat > {}", sh_quote(commands::DETACHED_FALLBACK)),
            commands::CHMOD_KEXEC_TOOLS,
            &format!("cat > {}", sh_quote(commands::KEXEC_IMAGE)),
            &format!("cat > {}", sh_quote(commands::KEXEC_SCRIPT)),
            &commands::launch_kexec(Some(commands::KEXEC_IMAGE)),
            commands::BOOT,
        ],
    );
}

#[tokio::test(start_paused = true)]
async fn install_without_fel_device_times_out_with_instructions() {
    let backend = SimBackend::new(Sim::stock());
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();
    let error = flow.run(&mut ctx).await.expect_err("no FEL device");
    assert!(error.is_timeout());
    let text = error.to_string();
    assert!(
        text.starts_with("step 1 (memboot) failed: a console in FEL mode"),
        "{text}"
    );
    assert!(text.contains("did not happen within 20 s"), "{text}");
    assert!(text.contains("hold Reset"), "{text}");
    assert_eq!(flow.position(), 0);
    assert!(backend.log().is_empty(), "nothing was sent to a console");
}

#[tokio::test(start_paused = true)]
async fn console_that_never_reappears_times_out_with_the_usb_hint() {
    let backend = SimBackend::new(Sim {
        fel_present: true,
        memboot_comes_back: Some(ComeBack::Stays),
        ..Sim::stock()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::memboot();
    let error = flow.run(&mut ctx).await.expect_err("never comes back");
    assert!(error.is_timeout());
    let text = error.to_string();
    assert!(
        text.starts_with(
            "step 2 (wait-console) failed: the console reappearing on the USB network did not \
             happen within 60 s; check the USB cable and that FEL mode was entered"
        ),
        "{text}"
    );
    assert_eq!(flow.position(), 1);
}

#[tokio::test(start_paused = true)]
async fn memboot_flow_leaves_the_console_in_the_shell() {
    let backend = SimBackend::new(Sim {
        fel_present: true,
        ..Sim::stock()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::memboot();
    flow.run(&mut ctx).await.expect("memboot succeeds");
    assert!(ctx.state.is_memboot_shell());
    backend.with(|sim| {
        assert!(sim.online && sim.memboot_shell);
        assert_eq!(
            sim.nand_kernel,
            NandKernel::Stock,
            "nothing written to NAND"
        );
        assert_eq!(sim.boots, 0);
    });
}

#[tokio::test(start_paused = true)]
async fn uninstall_from_a_running_kernel_restores_stock_without_a_package() {
    let backend = SimBackend::new(Sim::installed());
    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::uninstall_kernel();
    flow.run(&mut ctx).await.expect("uninstall succeeds");
    assert_eq!(
        outcomes(&flow),
        expected(&[
            ("memboot", "done"),
            ("wait-console", "done"),
            ("detect", "done"),
            ("read-stock-kernel", "done"),
            ("flash-stock-kernel", "done"),
            ("remove", "done"),
            ("reboot", "done"),
            ("wait-offline", "done"),
        ])
    );
    assert_in_order(
        &backend.log(),
        &[
            commands::HAS_ATAGS,
            commands::UISTOP,
            commands::MKDIR_SCRATCH,
            &format!("cat > {}", sh_quote(commands::KEXEC_BINARY)),
            &format!("cat > {}", sh_quote(commands::DETACHED_FALLBACK)),
            commands::CHMOD_KEXEC_TOOLS,
            &format!("cat > {}", sh_quote(commands::KEXEC_SCRIPT)),
            &commands::launch_kexec(None),
            console_commands::IS_MINIMAL_MEMBOOT,
            commands::GET_BACKUP2,
            commands::IS_ORIGINAL_BOOT2,
            &format!("cat > {}", sh_quote(commands::KERNEL_PATH)),
            commands::SNTOOL_CHECK_KERNEL,
            commands::SNTOOL_KERNEL_INFO,
            commands::FLASH_BOOT2,
            console_commands::MOUNT_BASE,
            commands::INSTALL_DIR_EXISTS,
            console_commands::UMOUNT_BASE,
            console_commands::MOUNT_BASE,
            commands::MOD_UNINSTALL,
            console_commands::UMOUNT_BASE,
            commands::REBOOT,
        ],
    );
    assert_eq!(
        count(
            &backend.log(),
            &format!("cat > {}", sh_quote(commands::KEXEC_IMAGE))
        ),
        0,
        "no package: the kernel in NAND is kexec'd"
    );
    backend.with(|sim| {
        assert_eq!(sim.nand_kernel, NandKernel::Stock);
        assert!(!sim.install_dir && !sim.installed);
        assert!(sim.udisk, "partition shrunk back");
        assert!(!sim.online, "stock firmware has no network");
        assert_eq!(sim.kexecs, 1);
        assert_eq!(sim.fel_memboots, 0);
    });
}

#[tokio::test(start_paused = true)]
async fn uninstall_offline_without_package_says_what_is_needed() {
    let backend = SimBackend::new(Sim {
        online: false,
        fel_present: true,
        ..Sim::installed()
    });
    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::uninstall_kernel();
    let error = flow
        .run(&mut ctx)
        .await
        .expect_err("needs the package for FEL");
    assert!(error.to_string().contains("--hmod"), "{error}");
    assert_eq!(flow.position(), 0);
}

#[tokio::test(start_paused = true)]
async fn uninstall_without_backup_uses_the_given_dump_only_when_known() {
    let dir = tempfile::tempdir().unwrap();
    let dump = dir.path().join("kernel.img");
    std::fs::write(&dump, stock_kernel()).unwrap();

    let backend = SimBackend::new(Sim {
        backup: None,
        ..Sim::installed()
    });
    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::uninstall_kernel();
    let error = flow.run(&mut ctx).await.expect_err("no backup, no file");
    assert!(error.to_string().contains("--stock-kernel"), "{error}");
    assert_eq!(flow.position(), 3);

    ctx.options.stock_kernel = Some(dump.clone());
    let error = flow.run(&mut ctx).await.expect_err("unknown md5 refused");
    assert!(
        error.to_string().contains("--force-stock-kernel"),
        "{error}"
    );

    ctx.options.force_stock_kernel = true;
    flow.run(&mut ctx).await.expect("forced dump flashes");
    backend.with(|sim| assert_eq!(sim.nand_kernel, NandKernel::Stock));
}

#[tokio::test(start_paused = true)]
async fn dump_kernel_writes_the_backup_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("kernel.img");
    let backend = SimBackend::new(Sim::installed());
    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::dump_stock_kernel(&out);
    flow.run(&mut ctx).await.expect("dump succeeds");
    assert_eq!(std::fs::read(&out).unwrap(), stock_kernel());
    assert_eq!(
        outcomes(&flow)[3],
        ("dump-stock-kernel".to_string(), "done".to_string())
    );
    backend.with(|sim| {
        assert_eq!(
            sim.nand_kernel,
            NandKernel::Hakchi,
            "dump changes nothing on NAND"
        );
        assert!(sim.install_dir);
    });

    // The console came back from the reboot as a full hakchi boot; a second dump skips the
    // read because the file is already a valid image.
    backend.settle();
    backend.with(|sim| assert!(sim.online && !sim.memboot_shell));
    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::dump_stock_kernel(&out);
    flow.run(&mut ctx).await.expect("second dump succeeds");
    assert_eq!(
        outcomes(&flow)[3],
        ("dump-stock-kernel".to_string(), "already".to_string())
    );
    assert_eq!(count(&backend.log(), commands::GET_BACKUP2), 1);
}

#[tokio::test(start_paused = true)]
async fn mega_drive_layout_skips_the_kernel_flash_and_the_grow_reboot() {
    let backend = SimBackend::new(Sim {
        md: true,
        fel_present: true,
        ..Sim::stock()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();
    flow.run(&mut ctx).await.expect("install on MD succeeds");
    let results = outcomes(&flow);
    assert_eq!(results[3], ("check-md".to_string(), "done".to_string()));
    assert_eq!(
        results[5],
        ("reboot-grown".to_string(), "not-needed".to_string())
    );
    backend.with(|sim| assert_eq!(sim.kexecs, 0, "no reboot after grow on MD"));
    assert_in_order(
        &backend.log(),
        &[
            console_commands::MOUNT_BASE,
            &commands::project_lunar_check("/var/squashfs"),
            console_commands::UMOUNT_BASE,
        ],
    );

    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::uninstall_kernel();
    flow.run(&mut ctx).await.expect("uninstall on MD succeeds");
    let results = outcomes(&flow);
    assert_eq!(
        results[3],
        ("read-stock-kernel".to_string(), "not-needed".to_string())
    );
    assert_eq!(
        results[4],
        ("flash-stock-kernel".to_string(), "not-needed".to_string())
    );
    assert_eq!(count(&backend.log(), commands::FLASH_BOOT2), 0);
}

#[tokio::test(start_paused = true)]
async fn wrong_state_after_memboot_is_reported() {
    // The console answers after the memboot but as a full boot, not the memboot shell.
    let backend = SimBackend::new(Sim {
        memboot_comes_back: Some(ComeBack::FullBoot),
        ..Sim::installed()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::install_kernel();
    let error = flow
        .run(&mut ctx)
        .await
        .expect_err("not in the memboot shell");
    assert!(matches!(error.root(), Error::WrongState(_)), "{error}");
    assert!(
        error
            .to_string()
            .contains("expected the minimal memboot shell"),
        "{error}"
    );
    assert_eq!(flow.position(), 2);
}

#[tokio::test(start_paused = true)]
async fn failed_kexec_preparation_restarts_the_ui_and_reports_the_command() {
    // hakchi2 CE's catch block: a failure between `uistop` and the launch brings the UI back
    // and the error out; the console keeps running its kernel.
    let backend = SimBackend::new(Sim {
        fail_kexec_chmod: true,
        ..Sim::installed()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    let mut flow = flows::memboot();
    let error = flow.run(&mut ctx).await.expect_err("chmod fails");
    assert!(matches!(error.root(), Error::Device(_)), "{error}");
    let text = error.to_string();
    assert!(text.starts_with("step 1 (memboot) failed"), "{text}");
    assert!(text.contains(commands::CHMOD_KEXEC_TOOLS), "{text}");
    assert!(text.contains("Operation not permitted"), "{text}");
    let log = backend.log();
    assert_in_order(
        &log,
        &[
            commands::UISTOP,
            commands::CHMOD_KEXEC_TOOLS,
            commands::UISTART,
        ],
    );
    assert_eq!(count(&log, commands::UISTART), 1);
    assert_eq!(
        count(&log, &format!("cat > {}", sh_quote(commands::KEXEC_IMAGE))),
        0,
        "nothing more is uploaded after the failure"
    );
    assert_eq!(
        count(&log, &commands::launch_kexec(Some(commands::KEXEC_IMAGE))),
        0
    );
    backend.with(|sim| {
        assert_eq!(sim.kexecs, 0);
        assert!(
            sim.online && !sim.memboot_shell,
            "the console keeps running"
        );
    });

    // Rerunning resumes at the memboot step and goes through this time.
    flow.run(&mut ctx).await.expect("second attempt succeeds");
    assert!(ctx.state.is_memboot_shell());
    backend.with(|sim| {
        assert_eq!(sim.kexecs, 1);
        assert!(sim.online && sim.memboot_shell);
    });
}

/// The tiny7zx mod with made-up content (the real files are downloaded, never bundled).
fn fake_mods() -> Vec<LoadedMod> {
    let files = vec![
        ("bin/tiny7zx".to_string(), b"ELF fake tiny7zx".to_vec()),
        (
            "install".to_string(),
            b"chmod +x \"$transferpath/bin/tiny7zx\"\n".to_vec(),
        ),
        ("readme.md".to_string(), b"# tiny7zx".to_vec()),
    ];
    let mut builder = tar::Builder::new(Vec::new());
    for (path, data) in &files {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, path, data.as_slice())
            .unwrap();
    }
    vec![LoadedMod {
        spec: &basemods::TINY7ZX,
        files,
        tar: builder.into_inner().unwrap(),
    }]
}

#[tokio::test(start_paused = true)]
async fn base_mods_flow_uploads_installs_reboots_and_verifies() {
    let backend = SimBackend::new(Sim::installed());
    let (mut ctx, mut events) = context(&backend, None);
    ctx = ctx.with_base_mods(fake_mods());
    let mut flow = flows::install_base_mods();

    flow.run(&mut ctx).await.expect("base mods install");

    assert_eq!(
        outcomes(&flow),
        expected(&[
            ("wait-console", "already"),
            ("detect", "done"),
            ("base-mods", "done"),
            ("wait-console", "done"),
            ("detect", "done"),
            ("verify-mods", "done"),
        ])
    );
    let log = backend.log();
    let md5 = commands::md5sum_all(&[format!("{ROOTFS}/bin/tiny7zx")]);
    assert_in_order(
        &log,
        &[
            console_commands::VERSIONS,
            &md5,
            commands::MKDIR_MODS,
            &commands::mkdir_mod("tiny7zx"),
            &commands::extract_mod("tiny7zx"),
            commands::PACKS_INSTALL,
            &md5,
            &commands::mod_registered("/var/lib/hakchi", "tiny7zx"),
            commands::REBOOT,
            console_commands::VERSIONS,
            &md5,
            &commands::mod_registered("/var/lib/hakchi", "tiny7zx"),
            &commands::which("tiny7zx"),
        ],
    );
    assert_eq!(count(&log, commands::PACKS_INSTALL), 1);
    assert_eq!(count(&log, commands::REBOOT), 1);
    backend.with(|sim| {
        assert!(sim.mods_registered.contains("tiny7zx"));
        assert_eq!(
            sim.rootfs_files.get(&format!("{ROOTFS}/bin/tiny7zx")),
            Some(&b"ELF fake tiny7zx".to_vec())
        );
        assert!(
            !sim.rootfs_files.contains_key(&format!("{ROOTFS}/install")),
            "install script is consumed, not copied"
        );
        assert!(
            sim.files.keys().all(|k| !k.starts_with(commands::MODS_DIR)),
            "packs_install removed the uploaded directory"
        );
        assert!(sim.mods_live && sim.online && sim.installed);
        assert_eq!(sim.boots, 0, "the kernel install never ran");
    });
    let mut saw_console_line = false;
    let mut saw_path = false;
    while let Ok(event) = events.try_recv() {
        if let Event::Log { message, .. } = event {
            saw_console_line |= message == "console: package tiny7zx installed";
            saw_path |= message.contains("tiny7zx is on the console's PATH at /bin/tiny7zx");
        }
    }
    assert!(saw_console_line && saw_path);

    // A second run finds everything in place and does not reboot again.
    let before = backend.log().len();
    let (mut ctx, _events) = context(&backend, None);
    ctx = ctx.with_base_mods(fake_mods());
    let mut flow = flows::install_base_mods();
    flow.run(&mut ctx).await.expect("second run");
    assert_eq!(
        outcomes(&flow)[2],
        ("base-mods".to_string(), "already".to_string())
    );
    assert_eq!(count(&backend.log()[before..], commands::REBOOT), 0);
    assert_eq!(count(&backend.log()[before..], commands::PACKS_INSTALL), 0);
}

#[tokio::test(start_paused = true)]
async fn base_mods_need_a_booted_console_and_loaded_mods() {
    // In the memboot shell: refused before anything is uploaded.
    let backend = SimBackend::new(Sim {
        memboot_shell: true,
        ..Sim::installed()
    });
    let (mut ctx, _events) = context(&backend, None);
    ctx = ctx.with_base_mods(fake_mods());
    let mut flow = flows::install_base_mods();
    let error = flow.run(&mut ctx).await.expect_err("memboot shell");
    assert!(matches!(error.root(), Error::WrongState(_)), "{error}");
    assert!(
        error.to_string().contains("installed hakchi kernel"),
        "{error}"
    );
    assert_eq!(flow.position(), 1);
    assert_eq!(count(&backend.log(), commands::PACKS_INSTALL), 0);

    // Nothing loaded: the step says what to do.
    let backend = SimBackend::new(Sim::installed());
    let (mut ctx, _events) = context(&backend, None);
    let mut flow = flows::install_base_mods();
    let error = flow.run(&mut ctx).await.expect_err("no mods");
    assert!(matches!(error.root(), Error::Precondition(_)), "{error}");
    assert!(error.to_string().contains("--mods"), "{error}");
    assert_eq!(flow.position(), 2);
}

#[tokio::test(start_paused = true)]
async fn install_with_base_mods_adds_them_after_the_kernel_is_verified() {
    let backend = SimBackend::new(Sim {
        fel_present: true,
        ..Sim::stock()
    });
    let (mut ctx, _events) = context(&backend, Some(package()));
    ctx = ctx.with_base_mods(fake_mods());
    let mut flow = flows::install_kernel_with_base_mods();
    let report = flow.run(&mut ctx).await.expect("install with mods");
    assert_eq!(report.results.len(), 17);
    let results = outcomes(&flow);
    assert_eq!(
        &results[12..],
        &expected(&[
            ("verify", "done"),
            ("base-mods", "done"),
            ("wait-console", "done"),
            ("detect", "done"),
            ("verify-mods", "done"),
        ])[..]
    );
    assert_in_order(
        &backend.log(),
        &[
            commands::BOOT,
            console_commands::VERSIONS,
            commands::PACKS_INSTALL,
            commands::REBOOT,
            &commands::which("tiny7zx"),
        ],
    );
    backend.with(|sim| {
        assert_eq!(sim.boots, 1);
        assert!(sim.mods_registered.contains("tiny7zx"));
        assert!(sim.online && sim.installed && sim.mods_live);
    });
}
