//! Typed wrappers over the device-side `hakchi` script and the connect sequence.
//!
//! The console does the heavy lifting: `/bin/hakchi <function>` runs a shell function from
//! `/etc/preinit.d/b0010_functions` in the ramdisk. The command strings here are the ones
//! hakchi2 CE sends (`hakchi.cs`, `MemoryStats.cs`, `GameCacheTask.cs`), verbatim, so that the
//! trace in `docs/reference/hakchi-ce-sync-trace.log` is reproduced.

use std::{collections::BTreeMap, future::Future};

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::{
    ConsoleFamily, ConsoleType, Error, ExecOutput, Firmware, Region, Result, Session,
    StorageReport, Versions,
    parse::{
        check_device_path, classify_games_usage, format_config, is_shell_identifier, parse_config,
        parse_current_firmware, parse_df_summary, parse_dir_listing, parse_du_summary, parse_hwid,
        parse_yes_no, sh_quote,
    },
};

/// Something that runs shell commands on the console. [`Session`] is the real one; tests use
/// scripted stand-ins.
pub trait Shell: Send + Sync {
    /// Runs `command`, feeding `stdin` to it when given, and collects the result.
    fn exec<'a>(
        &'a self,
        command: &'a str,
        stdin: Option<&'a [u8]>,
    ) -> impl Future<Output = Result<ExecOutput>> + Send + 'a;
}

/// The exact command strings sent to the console.
pub mod commands {
    /// `hakchi ismdPartitioning`: exit 0 on a Mega Drive Mini NAND layout.
    pub const IS_MD_PARTITIONING: &str = "hakchi ismdPartitioning";
    /// Exit 0 when the console was membooted with the minimal (recovery) configuration.
    pub const IS_MINIMAL_MEMBOOT: &str = r#"source /hakchi/config; [ "$cf_memboot" = "y" ]"#;
    /// Hardware id: four 32-bit words from the SoC's SID.
    pub const HWID: &str = "hakchi hwid";
    /// Prints `yes` when `/var/version` exists.
    pub const VERSION_EXISTS: &str = r#"[ -f /var/version ] && echo "yes""#;
    /// Boot, kernel and script versions on one line, then Cartload's system version (nothing
    /// with hakchi's own package).
    pub const VERSIONS: &str = r#"source /var/version && echo "$bootVersion $kernelVersion $hakchiVersion $cartloadVersion""#;
    /// System code such as `snes-eur`.
    pub const SYSTEM_CODE: &str = r#"hakchi eval 'echo "$sftype-$sfregion"'"#;
    /// `_nand_` or the path of the custom firmware in use.
    pub const CURRENT_FIRMWARE: &str = "hakchi currentFirmware";
    /// Path of the games sync storage.
    pub const FIND_GAME_SYNC_STORAGE: &str = "hakchi findGameSyncStorage";
    /// The `p0000_config` file with the `cfg_` variables.
    pub const CONFIG: &str = "cat /etc/preinit.d/p0000_config";
    /// `df` of the sync storage as `total | used | free` in 1 KiB blocks.
    pub const STORAGE: &str =
        r#"df "$(hakchi findGameSyncStorage)" | tail -n 1 | awk '{ print $2 " | " $3 " | " $4 }'"#;
    /// Non-empty output when save states are on external storage.
    pub const EXTERNAL_SAVES: &str = "mount | grep /var/lib/clover/profiles";
    /// Size of the save states directory in 1 KiB blocks.
    pub const SAVES_SIZE: &str = r#"du -s "$(readlink /var/saves)" | awk '{ print $1 }'"#;
    /// Directory holding the original games inside the firmware squashfs.
    pub const ORIGINAL_GAMES_PATH: &str = r#"hakchi eval 'echo "$squashfs$gamepath"'"#;
    /// `hakchi mmcUsed`: exit 0 when the firmware script enables the SD card controller.
    pub const MMC_USED: &str = "hakchi mmcUsed";
    /// Mounts `/var` (tmpfs), the firmware squashfs and the NAND data partition on `/var/lib`.
    pub const MOUNT_BASE: &str = "hakchi mount_base";
    /// Undoes [`MOUNT_BASE`].
    pub const UMOUNT_BASE: &str = "hakchi umount_base";
    /// Binds the synced games over the stock games directory and links the menu.
    pub const OVERMOUNT_GAMES: &str = "hakchi overmount_games";
    /// Freezes the UI processes (`SIGSTOP`).
    pub const UI_PAUSE: &str = "hakchi uipause";
    /// Resumes the UI processes (`SIGCONT`).
    pub const UI_RESUME: &str = "hakchi uiresume";
    /// Restarts the stock UI.
    pub const UI_START: &str = "hakchi uistart";
    /// Stops the stock UI.
    pub const UI_STOP: &str = "hakchi uistop";
    /// Evaluates a script read from stdin in the `hakchi` environment.
    pub const EVAL_STDIN: &str = "hakchi eval";

    /// `du` of the immediate subdirectories of `path` as `path | kilobytes` lines.
    pub fn games_sizes(path: &str) -> String {
        format!(r#"du -d 1 "{path}" | head -n -1 | awk '{{ print $2 " | " $1 }}'"#)
    }

    /// Prints `YES` or `NO` depending on whether `path` is a directory.
    pub fn dir_exists(path: &str) -> String {
        format!(r#"[ -d "{path}" ] && echo YES || echo NO"#)
    }

    /// Lists the subdirectories of `path`, one per line with a trailing slash.
    pub fn list_dirs(path: &str) -> String {
        format!(r#"ls "{path}" -p -1 | grep '/'"#)
    }

    /// `hakchi get <var>`.
    pub fn get(var: &str) -> String {
        format!("hakchi get {var}")
    }

    /// `hakchi eval '<shell>'`.
    pub fn eval(shell_quoted: &str) -> String {
        format!("hakchi eval {shell_quoted}")
    }
}

/// Everything learnt about a console at connect time (hakchi2 CE `Shell_OnConnected`,
/// `MemoryStats.Refresh` and `GameCacheTask.UpdateLocal`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConsoleInfo {
    /// Hardware id, 32 lowercase hex digits.
    pub hwid: String,
    /// The NAND uses the Mega Drive Mini partition layout.
    pub md_partitioning: bool,
    /// The console runs a minimal memboot (recovery). Nothing below is read in that case.
    pub minimal_memboot: bool,
    /// Versions from `/var/version`; `None` when the file is missing (very old kernel).
    pub versions: Option<Versions>,
    /// Whether the kernel and script are recent enough to work with. Nothing below is read
    /// when this is false.
    pub can_interact: bool,
    /// Detected console model and region.
    pub console: ConsoleType,
    /// Raw system code (`snes-eur`).
    pub system_code: Option<String>,
    /// Firmware in use.
    pub firmware: Option<Firmware>,
    /// Games sync storage (`/var/lib/hakchi/games` on NAND, `/media/hakchi/games` on USB).
    pub game_sync_path: Option<String>,
    /// Stock games directory inside the firmware (`$gamepath`, `/usr/share/games` on SNES).
    pub original_games_path: Option<String>,
    /// hakchi rootfs overlay (`$rootfs`).
    pub rootfs_path: Option<String>,
    /// Firmware squashfs mount (`$squashfs`).
    pub squashfs_path: Option<String>,
    /// `cfg_` variables from `p0000_config`, without the prefix.
    pub config: BTreeMap<String, String>,
    /// Storage usage of the sync storage.
    pub storage: Option<StorageReport>,
    /// Ids of the games built into the firmware (`CLV-P-...`); empty on Mega Drive.
    pub original_games: Vec<String>,
}

impl ConsoleInfo {
    /// Hardware family of the detected console.
    pub fn family(&self) -> ConsoleFamily {
        self.console.family()
    }

    /// Region of the detected console.
    pub fn region(&self) -> Region {
        self.console.region()
    }

    /// Kernel version string, when known.
    pub fn kernel_version(&self) -> Option<&str> {
        self.versions.as_ref().map(|v| v.kernel.as_str())
    }

    /// Total, used and free bytes of the sync storage, when known.
    pub fn storage_usage(&self) -> Option<crate::StorageUsage> {
        self.storage.as_ref().map(|s| s.usage)
    }
}

/// Typed wrappers over the `hakchi` script, on top of any [`Shell`].
#[derive(Clone, Copy, Debug)]
pub struct Hakchi<'a, S: Shell = Session> {
    shell: &'a S,
}

impl<'a, S: Shell> Hakchi<'a, S> {
    /// Wraps a shell.
    pub fn new(shell: &'a S) -> Self {
        Self { shell }
    }

    /// The underlying shell.
    pub fn shell(&self) -> &'a S {
        self.shell
    }

    /// Runs a command that must succeed and returns its trimmed stdout.
    async fn simple(&self, command: &str) -> Result<String> {
        let output = self.shell.exec(command, None).await?.checked(command)?;
        Ok(output.stdout_trimmed())
    }

    /// Runs a command and reports whether it exited with status 0.
    async fn succeeds(&self, command: &str) -> Result<bool> {
        Ok(self.shell.exec(command, None).await?.success())
    }

    /// `hakchi findGameSyncStorage`: the directory games are synced to.
    pub async fn find_game_sync_storage(&self) -> Result<String> {
        let path = self.simple(commands::FIND_GAME_SYNC_STORAGE).await?;
        if path.is_empty() {
            return Err(Error::Parse {
                what: "game sync storage path",
                input: path,
            });
        }
        Ok(path)
    }

    /// `hakchi get <var>`: the value of a variable of the hakchi environment (`gamepath`,
    /// `rootfs`, `squashfs`, `cfg_...`).
    pub async fn get(&self, var: &str) -> Result<String> {
        if !is_shell_identifier(var) {
            return Err(Error::InvalidArgument(format!(
                "{var:?} is not a variable name"
            )));
        }
        self.simple(&commands::get(var)).await
    }

    /// `hakchi eval '<shell>'`: runs shell code inside the hakchi environment and returns the
    /// raw result; non-zero exits are not errors (hakchi2 CE ignores them for example for
    /// `umount "$gamepath"`).
    pub async fn eval(&self, shell: &str) -> Result<ExecOutput> {
        self.shell
            .exec(&commands::eval(&sh_quote(shell)), None)
            .await
    }

    /// `hakchi eval` with the script on stdin, for multi-line scripts and config writes.
    pub async fn eval_script(&self, script: &[u8]) -> Result<ExecOutput> {
        self.shell.exec(commands::EVAL_STDIN, Some(script)).await
    }

    /// `hakchi currentFirmware`.
    pub async fn current_firmware(&self) -> Result<Firmware> {
        parse_current_firmware(&self.simple(commands::CURRENT_FIRMWARE).await?)
    }

    /// `hakchi hwid`, normalised to 32 lowercase hex digits.
    pub async fn hwid(&self) -> Result<String> {
        parse_hwid(&self.simple(commands::HWID).await?)
    }

    /// `hakchi mmcUsed`: whether the firmware script enables the SD card controller.
    pub async fn mmc_used(&self) -> Result<bool> {
        self.succeeds(commands::MMC_USED).await
    }

    /// `hakchi ismdPartitioning`.
    pub async fn is_md_partitioning(&self) -> Result<bool> {
        self.succeeds(commands::IS_MD_PARTITIONING).await
    }

    /// Whether the console runs a minimal memboot (`cf_memboot=y`).
    pub async fn is_minimal_memboot(&self) -> Result<bool> {
        self.succeeds(commands::IS_MINIMAL_MEMBOOT).await
    }

    /// `hakchi mount_base`: mounts `/var`, the firmware squashfs and the data partition.
    pub async fn mount_base(&self) -> Result<()> {
        self.simple(commands::MOUNT_BASE).await.map(drop)
    }

    /// `hakchi umount_base`.
    pub async fn umount_base(&self) -> Result<()> {
        self.simple(commands::UMOUNT_BASE).await.map(drop)
    }

    /// `hakchi overmount_games`: binds the synced games over the stock directory. Returns
    /// whether games were found and mounted (the script exits 1 when there are none).
    pub async fn overmount_games(&self) -> Result<bool> {
        let output = self.shell.exec(commands::OVERMOUNT_GAMES, None).await?;
        debug!(stdout = %output.stdout_trimmed(), "overmount_games");
        Ok(output.success())
    }

    /// `hakchi uipause`. The exit status is ignored: the script ends with a `killall` of the
    /// Mega Drive UI process, which fails on other consoles even though the UI was paused.
    pub async fn ui_pause(&self) -> Result<()> {
        self.shell.exec(commands::UI_PAUSE, None).await.map(drop)
    }

    /// `hakchi uiresume`. Exit status ignored, see [`Hakchi::ui_pause`].
    pub async fn ui_resume(&self) -> Result<()> {
        self.shell.exec(commands::UI_RESUME, None).await.map(drop)
    }

    /// `hakchi uistart`: restarts the stock UI. Exit status ignored.
    pub async fn ui_start(&self) -> Result<()> {
        self.shell.exec(commands::UI_START, None).await.map(drop)
    }

    /// `hakchi uistop`: stops the stock UI. Exit status ignored.
    pub async fn ui_stop(&self) -> Result<()> {
        self.shell.exec(commands::UI_STOP, None).await.map(drop)
    }

    /// Versions from `/var/version`, or `None` when the file does not exist.
    pub async fn versions(&self) -> Result<Option<Versions>> {
        let exists = self.shell.exec(commands::VERSION_EXISTS, None).await?;
        if exists.stdout_trimmed() != "yes" {
            return Ok(None);
        }
        Versions::parse_echo(&self.simple(commands::VERSIONS).await?).map(Some)
    }

    /// `$sftype-$sfregion`, the system code (`snes-eur`).
    pub async fn system_code(&self) -> Result<String> {
        self.simple(commands::SYSTEM_CODE).await
    }

    /// The `cfg_` variables from `p0000_config`, without the prefix. An unreadable file gives
    /// an empty map, as in hakchi2 CE.
    pub async fn config(&self) -> Result<BTreeMap<String, String>> {
        let output = self.shell.exec(commands::CONFIG, None).await?;
        if !output.success() {
            warn!(
                exit_code = ?output.exit_code,
                "could not read p0000_config; treating the config as empty"
            );
            return Ok(BTreeMap::new());
        }
        Ok(parse_config(&output.stdout_lossy()))
    }

    /// Writes `cfg_` variables through `hakchi eval` (hakchi2 CE `SyncConfig`).
    pub async fn set_config(&self, config: &BTreeMap<String, String>) -> Result<()> {
        for key in config.keys() {
            if !is_shell_identifier(key) {
                return Err(Error::InvalidArgument(format!(
                    "{key:?} is not a config variable name"
                )));
            }
        }
        self.eval_script(format_config(config).as_bytes())
            .await?
            .checked(commands::EVAL_STDIN)?;
        Ok(())
    }

    /// `df` of the sync storage.
    pub async fn storage(&self) -> Result<crate::StorageUsage> {
        parse_df_summary(&self.simple(commands::STORAGE).await?)
    }

    /// Storage report of the sync storage at `sync_path` (hakchi2 CE `MemoryStats.Refresh`).
    pub async fn storage_report(&self, sync_path: &str) -> Result<StorageReport> {
        check_device_path(sync_path)?;
        let usage = self.storage().await?;
        let external_saves = !self
            .shell
            .exec(commands::EXTERNAL_SAVES, None)
            .await?
            .stdout_trimmed()
            .is_empty();
        let saves = self
            .shell
            .exec(commands::SAVES_SIZE, None)
            .await?
            .stdout_trimmed()
            .parse::<u64>()
            .unwrap_or(0)
            * 1024;
        let dirs = parse_du_summary(&self.simple(&commands::games_sizes(sync_path)).await?)?;
        Ok(StorageReport {
            path: sync_path.to_string(),
            usage,
            saves,
            external_saves,
            games: classify_games_usage(&dirs),
        })
    }

    /// Ids of the games built into the firmware (`CLV-P-...`), read from the firmware
    /// squashfs (hakchi2 CE `GameCacheTask.UpdateLocal`). Falls back to the known directory
    /// for the console type when `$squashfs$gamepath` is not a directory. Empty on Mega Drive,
    /// where the stock games are not laid out as directories.
    pub async fn original_games(
        &self,
        console: ConsoleType,
        squashfs: &str,
    ) -> Result<Vec<String>> {
        if console.family() == ConsoleFamily::MegaDrive {
            return Ok(Vec::new());
        }
        let mut path = self.simple(commands::ORIGINAL_GAMES_PATH).await?;
        check_device_path(&path)?;
        let exists = self.simple(&commands::dir_exists(&path)).await?;
        if parse_yes_no(&exists) != Some(true) {
            path = format!("{squashfs}{}", console.games_squashfs_path());
            check_device_path(&path)?;
            let exists = self.simple(&commands::dir_exists(&path)).await?;
            if parse_yes_no(&exists) != Some(true) {
                return Err(Error::Parse {
                    what: "original games directory",
                    input: path,
                });
            }
        }
        // grep exits 1 when there is nothing to list; that is an empty list, not an error.
        let listing = self.shell.exec(&commands::list_dirs(&path), None).await?;
        Ok(parse_dir_listing(&listing.stdout_lossy()))
    }

    /// Runs the connect sequence and returns everything learnt about the console.
    pub async fn info(&self) -> Result<ConsoleInfo> {
        let mut info = ConsoleInfo {
            md_partitioning: self.is_md_partitioning().await?,
            minimal_memboot: self.is_minimal_memboot().await?,
            hwid: self.hwid().await?,
            ..ConsoleInfo::default()
        };
        debug!(hwid = info.hwid, "detected device unique id");
        if info.minimal_memboot {
            debug!("minimal memboot: skipping version and storage detection");
            return Ok(info);
        }
        info.versions = self.versions().await?;
        let versions = info
            .versions
            .clone()
            .unwrap_or_else(Versions::outdated_placeholder);
        debug!(%versions, "detected versions");
        info.can_interact = versions.can_interact();
        if !info.can_interact {
            warn!(
                %versions,
                reflash = versions.requires_reflash(),
                rootfs_update = versions.requires_rootfs_update(),
                "console software is too old to work with"
            );
            return Ok(info);
        }
        let system_code = self.system_code().await?;
        info.console = ConsoleType::from_system_code(&system_code);
        info.system_code = Some(system_code);
        info.firmware = Some(self.current_firmware().await?);
        let sync_path = self.find_game_sync_storage().await?;
        info.original_games_path = Some(self.get("gamepath").await?);
        info.rootfs_path = Some(self.get("rootfs").await?);
        let squashfs = self.get("squashfs").await?;
        info.config = self.config().await?;
        info.storage = Some(self.storage_report(&sync_path).await?);
        info.game_sync_path = Some(sync_path);
        info.original_games = self.original_games(info.console, &squashfs).await?;
        info.squashfs_path = Some(squashfs);
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{collections::VecDeque, sync::Mutex};

    use super::*;

    /// One expected command, the stdin it must receive (if checked) and the reply.
    type Step = (String, Option<Vec<u8>>, ExecOutput);

    /// Replays a scripted conversation, asserting every command string.
    struct Scripted {
        steps: Mutex<VecDeque<Step>>,
    }

    impl Scripted {
        fn new(steps: Vec<(&str, ExecOutput)>) -> Self {
            Self {
                steps: Mutex::new(
                    steps
                        .into_iter()
                        .map(|(command, output)| (command.to_string(), None, output))
                        .collect(),
                ),
            }
        }

        fn expect_stdin(self, command: &str, stdin: &[u8], output: ExecOutput) -> Self {
            self.steps.lock().unwrap().push_back((
                command.to_string(),
                Some(stdin.to_vec()),
                output,
            ));
            self
        }

        fn finished(&self) -> bool {
            self.steps.lock().unwrap().is_empty()
        }
    }

    impl Shell for Scripted {
        fn exec<'a>(
            &'a self,
            command: &'a str,
            stdin: Option<&'a [u8]>,
        ) -> impl Future<Output = Result<ExecOutput>> + Send + 'a {
            let next = self.steps.lock().unwrap().pop_front();
            async move {
                let (expected, expected_stdin, output) =
                    next.unwrap_or_else(|| panic!("unexpected command {command:?}"));
                assert_eq!(command, expected, "command sent to the console");
                if let Some(expected_stdin) = expected_stdin {
                    assert_eq!(
                        stdin,
                        Some(expected_stdin.as_slice()),
                        "stdin for {command}"
                    );
                }
                Ok(output)
            }
        }
    }

    fn ok(stdout: &str) -> ExecOutput {
        ExecOutput {
            exit_code: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn code(exit_code: u32, stdout: &str) -> ExecOutput {
        ExecOutput {
            exit_code: Some(exit_code),
            ..ok(stdout)
        }
    }

    // The original games of the European SNES Classic, as `ls -p -1` sorts them.
    const ORIGINAL_GAMES: [&str; 21] = [
        "CLV-P-SAAAE",
        "CLV-P-SAABE",
        "CLV-P-SAAEE",
        "CLV-P-SAAFE",
        "CLV-P-SAAHE",
        "CLV-P-SAAJE",
        "CLV-P-SAAKE",
        "CLV-P-SAALE",
        "CLV-P-SAAQE",
        "CLV-P-SAAXE",
        "CLV-P-SABCE",
        "CLV-P-SABDE",
        "CLV-P-SABHE",
        "CLV-P-SABQE",
        "CLV-P-SABRE",
        "CLV-P-SABTE",
        "CLV-P-SACBE",
        "CLV-P-SACCE",
        "CLV-P-SADGE",
        "CLV-P-SADJE",
        "CLV-P-SADKE",
    ];

    /// The connect sequence of docs/reference/hakchi-ce-sync-trace.log, command by command,
    /// with the exit codes the trace records.
    fn trace_connect_sequence() -> Scripted {
        let listing: String = ORIGINAL_GAMES.iter().map(|g| format!("{g}/\n")).collect();
        Scripted::new(vec![
            ("hakchi ismdPartitioning", code(1, "")),
            (
                r#"source /hakchi/config; [ "$cf_memboot" = "y" ]"#,
                code(2, ""),
            ),
            ("hakchi hwid", ok("0461872a 87280207 9d2818c7 6c118000\n")),
            (r#"[ -f /var/version ] && echo "yes""#, ok("yes\n")),
            // The trace's command plus `$cartloadVersion` (empty on hakchi's own package, as
            // here: echo leaves a trailing space).
            (
                r#"source /var/version && echo "$bootVersion $kernelVersion $hakchiVersion $cartloadVersion""#,
                ok("1.0.3 3.4.113.29-madmonkey v1.0.4-126 \n"),
            ),
            (
                r#"hakchi eval 'echo "$sftype-$sfregion"'"#,
                ok("snes-eur\n"),
            ),
            ("hakchi currentFirmware", ok("_nand_\n")),
            ("hakchi findGameSyncStorage", ok("/var/lib/hakchi/games\n")),
            ("hakchi get gamepath", ok("/usr/share/games\n")),
            ("hakchi get rootfs", ok("/var/lib/hakchi/rootfs\n")),
            ("hakchi get squashfs", ok("/var/squashfs\n")),
            (
                "cat /etc/preinit.d/p0000_config",
                ok("cfg_cloverapp_home='no'\ncfg_mdui_region=''\n"),
            ),
            (
                r#"df "$(hakchi findGameSyncStorage)" | tail -n 1 | awk '{ print $2 " | " $3 " | " $4 }'"#,
                ok("361060 | 28860 | 321060\n"),
            ),
            ("mount | grep /var/lib/clover/profiles", code(1, "")),
            (
                r#"du -s "$(readlink /var/saves)" | awk '{ print $1 }'"#,
                ok("104\n"),
            ),
            (
                r#"du -d 1 "/var/lib/hakchi/games" | head -n -1 | awk '{ print $2 " | " $1 }'"#,
                ok(""),
            ),
            (
                r#"hakchi eval 'echo "$squashfs$gamepath"'"#,
                ok("/var/squashfs/usr/share/games\n"),
            ),
            (
                r#"[ -d "/var/squashfs/usr/share/games" ] && echo YES || echo NO"#,
                ok("YES\n"),
            ),
            (
                r#"ls "/var/squashfs/usr/share/games" -p -1 | grep '/'"#,
                ok(&listing),
            ),
        ])
    }

    #[tokio::test]
    async fn info_replays_the_trace_connect_sequence() {
        let shell = trace_connect_sequence();
        let info = Hakchi::new(&shell).info().await.unwrap();
        assert!(shell.finished(), "every command of the trace was sent");

        assert_eq!(info.hwid, "0461872a872802079d2818c76c118000");
        assert!(!info.md_partitioning);
        assert!(!info.minimal_memboot);
        assert!(info.can_interact);
        assert_eq!(info.console, ConsoleType::SnesEur);
        assert_eq!(info.family(), ConsoleFamily::Snes);
        assert_eq!(info.region(), Region::Eur);
        assert_eq!(info.system_code.as_deref(), Some("snes-eur"));
        assert_eq!(info.kernel_version(), Some("3.4.113.29-madmonkey"));
        assert_eq!(
            info.versions.as_ref().unwrap().to_string(),
            "boot 1.0.3, kernel 3.4.113.29-madmonkey, script v1.0.4-126"
        );
        assert_eq!(info.firmware, Some(Firmware::Nand));
        assert_eq!(
            info.game_sync_path.as_deref(),
            Some("/var/lib/hakchi/games")
        );
        assert_eq!(
            info.original_games_path.as_deref(),
            Some("/usr/share/games")
        );
        assert_eq!(info.rootfs_path.as_deref(), Some("/var/lib/hakchi/rootfs"));
        assert_eq!(info.squashfs_path.as_deref(), Some("/var/squashfs"));
        assert_eq!(info.config["cloverapp_home"], "no");
        assert_eq!(info.config["mdui_region"], "");

        let storage = info.storage.as_ref().unwrap();
        assert_eq!(storage.path, "/var/lib/hakchi/games");
        assert_eq!(info.storage_usage().unwrap().total, 361060 * 1024);
        assert!(!storage.external_saves);
        assert_eq!(storage.saves, 104 * 1024);
        assert_eq!(
            storage.summary(info.console),
            "Storage size: 352.6MB, used: 28.2MB, free: 313.5MB\n    \
             Used by all games: 0.0MB\n    \
             Used by non multi-boot games: 0.0MB\n    \
             Used by current games collection: 0.0MB\n    \
             Used by save-states: 0.1MB\n    \
             Used by other files (mods, configs, etc.): 28.1MB\n    \
             Reserved memory: 0.0MB\n    \
             Available for games: 313.5MB"
        );
        assert_eq!(info.original_games, ORIGINAL_GAMES);
    }

    #[tokio::test]
    async fn info_stops_early_on_a_minimal_memboot() {
        let shell = Scripted::new(vec![
            ("hakchi ismdPartitioning", code(1, "")),
            (r#"source /hakchi/config; [ "$cf_memboot" = "y" ]"#, ok("")),
            ("hakchi hwid", ok("0461872a 87280207 9d2818c7 6c118000")),
        ]);
        let info = Hakchi::new(&shell).info().await.unwrap();
        assert!(shell.finished());
        assert!(info.minimal_memboot);
        assert!(!info.can_interact);
        assert_eq!(info.versions, None);
        assert_eq!(info.console, ConsoleType::Unknown);
    }

    #[tokio::test]
    async fn info_stops_when_the_kernel_is_too_old() {
        let shell = Scripted::new(vec![
            ("hakchi ismdPartitioning", code(1, "")),
            (
                r#"source /hakchi/config; [ "$cf_memboot" = "y" ]"#,
                code(2, ""),
            ),
            ("hakchi hwid", ok("0461872a 87280207 9d2818c7 6c118000")),
            (r#"[ -f /var/version ] && echo "yes""#, code(1, "")),
        ]);
        let info = Hakchi::new(&shell).info().await.unwrap();
        assert!(shell.finished());
        assert_eq!(info.versions, None);
        assert!(!info.can_interact);
        assert_eq!(info.storage, None);
    }

    #[tokio::test]
    async fn original_games_fall_back_to_the_console_default_directory() {
        let shell = Scripted::new(vec![
            (
                r#"hakchi eval 'echo "$squashfs$gamepath"'"#,
                ok("/var/squashfs\n"),
            ),
            (
                r#"[ -d "/var/squashfs" ] && echo YES || echo NO"#,
                ok("NO\n"),
            ),
            (
                r#"[ -d "/var/squashfs/usr/share/games/nes/kachikachi" ] && echo YES || echo NO"#,
                ok("YES\n"),
            ),
            (
                r#"ls "/var/squashfs/usr/share/games/nes/kachikachi" -p -1 | grep '/'"#,
                ok("CLV-P-NAAAE/\nCLV-P-NAABE/\n"),
            ),
        ]);
        let games = Hakchi::new(&shell)
            .original_games(ConsoleType::Nes, "/var/squashfs")
            .await
            .unwrap();
        assert!(shell.finished());
        assert_eq!(games, ["CLV-P-NAAAE", "CLV-P-NAABE"]);

        let shell = Scripted::new(vec![]);
        let games = Hakchi::new(&shell)
            .original_games(ConsoleType::MdEur, "/var/squashfs")
            .await
            .unwrap();
        assert!(games.is_empty());
    }

    #[tokio::test]
    async fn wrappers_send_the_expected_commands() {
        let shell = Scripted::new(vec![
            ("hakchi mount_base", ok("")),
            ("hakchi umount_base", ok("")),
            (
                "hakchi overmount_games",
                code(1, "no romz found at: /var/lib/hakchi/games/000"),
            ),
            ("hakchi uipause", code(1, "")),
            ("hakchi uiresume", code(1, "")),
            ("hakchi uistart", ok("")),
            ("hakchi uistop", ok("")),
            ("hakchi mmcUsed", code(1, "")),
            ("hakchi get cfg_mdui_region", ok("\n")),
            ("hakchi eval 'umount \"$gamepath\"'", code(1, "")),
            ("hakchi eval 'echo '\\''quoted'\\'''", ok("quoted")),
        ])
        .expect_stdin("hakchi eval", b"cfg_usb_wifi='it\\'s'\n", ok(""));
        let hakchi = Hakchi::new(&shell);
        hakchi.mount_base().await.unwrap();
        hakchi.umount_base().await.unwrap();
        assert!(!hakchi.overmount_games().await.unwrap());
        hakchi.ui_pause().await.unwrap();
        hakchi.ui_resume().await.unwrap();
        hakchi.ui_start().await.unwrap();
        hakchi.ui_stop().await.unwrap();
        assert!(!hakchi.mmc_used().await.unwrap());
        assert_eq!(hakchi.get("cfg_mdui_region").await.unwrap(), "");
        assert_eq!(
            hakchi.eval("umount \"$gamepath\"").await.unwrap().exit_code,
            Some(1)
        );
        assert_eq!(
            hakchi.eval("echo 'quoted'").await.unwrap().stdout_trimmed(),
            "quoted"
        );
        let mut config = BTreeMap::new();
        config.insert("usb_wifi".to_string(), "it's".to_string());
        hakchi.set_config(&config).await.unwrap();
        assert!(shell.finished());
    }

    #[tokio::test]
    async fn failures_and_bad_arguments_are_reported() {
        let shell = Scripted::new(vec![
            ("hakchi hwid", code(127, "")),
            ("hakchi findGameSyncStorage", ok("\n")),
            ("hakchi currentFirmware", ok("")),
        ]);
        let hakchi = Hakchi::new(&shell);
        assert!(matches!(
            hakchi.hwid().await,
            Err(Error::CommandFailed {
                exit_code: Some(127),
                ..
            })
        ));
        assert!(matches!(
            hakchi.find_game_sync_storage().await,
            Err(Error::Parse { .. })
        ));
        assert!(matches!(
            hakchi.current_firmware().await,
            Err(Error::Parse { .. })
        ));
        assert!(matches!(
            hakchi.get("bad name").await,
            Err(Error::InvalidArgument(_))
        ));
        assert!(matches!(
            hakchi.storage_report("/tmp/$(reboot)").await,
            Err(Error::InvalidArgument(_))
        ));
        let mut config = BTreeMap::new();
        config.insert("bad key".to_string(), String::new());
        assert!(matches!(
            hakchi.set_config(&config).await,
            Err(Error::InvalidArgument(_))
        ));
        assert!(shell.finished());
    }
}
