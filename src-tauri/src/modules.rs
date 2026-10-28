//! Emulator modules (RetroArch and cores) from KMFD's Mod Hub: what is cached on this
//! machine, what the connected console has, downloading, and loading for the install flow.

use std::path::PathBuf;

use cartload_console::Session;
use cartload_hmod::modhub::{self, FileSum, ModArchive, Module};
use serde::Serialize;
use tracing::{info, warn};

use crate::error::CommandError;

/// Event carrying a [`ModuleDownload`] while modules download.
pub const MODULE_EVENT: &str = "module-download";

/// One catalog module, as the Settings page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModuleView {
    /// Cartload's id.
    pub id: String,
    /// The hub item (the name the console registers).
    pub item: String,
    /// Human name.
    pub title: String,
    /// The name in the Settings grid.
    pub short: String,
    /// One sentence.
    pub purpose: String,
    /// Download size (the pinned file's).
    pub approx_bytes: u64,
    /// A caution to show before installing.
    pub warning: Option<String>,
    /// Cached on this machine.
    pub cached: bool,
    /// Size of the cached file.
    pub cached_bytes: Option<u64>,
    /// Installed on the connected console (`None` when no console answered).
    pub installed: Option<bool>,
    /// Has an uninstall record on the console, so Cartload can remove it (`None` when no
    /// console answered).
    pub removable: Option<bool>,
    /// The uninstall records that remove it: its own (`_km_snes9x_xtreme`) or another
    /// installer's (`snes9x2010` from hakchi2 CE).
    pub records: Vec<String>,
    /// The hub has a different build than the one on the console (`None` until an update
    /// check ran, or when it could not tell).
    pub update: Option<bool>,
}

/// The catalog with its state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModulesStatus {
    /// Where downloads go.
    pub cache_dir: Option<String>,
    /// The hub.
    pub hub: String,
    /// The modules.
    pub modules: Vec<ModuleView>,
    /// The console answered the installed check.
    pub console_checked: bool,
}

/// Download progress of one module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModuleDownload {
    /// The module.
    pub id: String,
    /// Bytes so far.
    pub done: u64,
    /// Bytes expected.
    pub total: Option<u64>,
}

/// The cache directory, or an error when the system has none.
pub fn cache_dir() -> Result<PathBuf, CommandError> {
    modhub::default_cache_dir()
        .ok_or_else(|| CommandError::new("This system has no cache directory for downloads."))
}

/// `which` for every binary of a module, in one command: exit code 0 when all are found.
pub fn installed_check(module: &Module) -> String {
    module
        .binaries
        .iter()
        .map(|b| format!("which {b} >/dev/null"))
        .collect::<Vec<_>>()
        .join(" && ")
}

/// Where `pack_install` writes the uninstall records.
const RECORDS_DIR: &str = "/var/lib/hakchi/hmod";

/// Prints the uninstall records (`uninstall-<name>`, one per line) that delete one of
/// `binaries` from `bin/`. `pack_install` generates each record from the package's files, all
/// on one line (`rm -f "$rootfs/./uninstall" ... "$rootfs/./bin/gb" "$rootfs/./bin/gambatte"`),
/// so this finds the package that put a launcher on the console whatever it was called:
/// Cartload's `_km_snes9x_xtreme` or hakchi2 CE's `snes9x2010`. The closing quote ends the
/// name (`gb` does not match `gbc`); the launcher need not be the last file on the line.
pub fn records_check(binaries: &[&str]) -> String {
    format!(
        r#"cd {RECORDS_DIR} 2>/dev/null && grep -lE '"\$rootfs/(\./)?bin/({})"' uninstall-* 2>/dev/null; true"#,
        binaries.join("|")
    )
}

/// The record names in the output of [`records_check`].
pub fn parse_records(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("uninstall-"))
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// The records that remove `module`. A core's never include a RetroArch package (one that
/// ships `retroarch-clover`): removing a core must not take RetroArch with it.
pub async fn records(session: &Session, module: &Module) -> Result<Vec<String>, CommandError> {
    let mut found = list_records(session, module.binaries).await?;
    if module.id != modhub::RETROARCH.id {
        let frontends = list_records(session, &["retroarch-clover"]).await?;
        found.retain(|name| !frontends.contains(name));
    }
    Ok(found)
}

async fn list_records(session: &Session, binaries: &[&str]) -> Result<Vec<String>, CommandError> {
    let output = session
        .exec(&records_check(binaries))
        .await
        .map_err(|error| {
            CommandError::new("Could not read the console's uninstall records.")
                .with_detail(error.to_string())
        })?;
    Ok(parse_records(&output.stdout_lossy()))
}

/// The record names removing `modules` takes off the console, each once.
pub async fn removal_records(
    session: &Session,
    modules: &[&Module],
) -> Result<Vec<String>, CommandError> {
    let mut names: Vec<String> = Vec::new();
    for module in modules {
        for name in records(session, module).await? {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Ok(names)
}

/// The modules removing `ids` takes off the console: RetroArch takes every core with it,
/// since none of them runs without it.
pub fn removal(ids: &[String]) -> Result<Vec<&'static Module>, CommandError> {
    let mut modules = resolve(ids)?;
    if modules.iter().any(|m| m.id == modhub::RETROARCH.id) {
        modules = modhub::CATALOG.to_vec();
    }
    modules.dedup_by_key(|m| m.id);
    Ok(modules)
}

/// The refusal when games of the library still need modules that are being removed.
pub fn removal_blocked(modules: &[&Module], games: &[String]) -> Option<CommandError> {
    if games.is_empty() {
        return None;
    }
    let titles = modules
        .iter()
        .map(|m| m.title)
        .collect::<Vec<_>>()
        .join(", ");
    let shown = games.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    let more = if games.len() > 5 {
        format!(" and {} more", games.len() - 5)
    } else {
        String::new()
    };
    Some(
        CommandError::new(format!(
            "{} game{} in the library still need{} {titles}: {shown}{more}. Remove them from \
             the library (or switch them to the built-in emulator) and sync first.",
            games.len(),
            if games.len() == 1 { "" } else { "s" },
            if games.len() == 1 { "s" } else { "" },
        ))
        .with_detail(games.join("\n")),
    )
}

/// The catalog with cache state and, when a session is given, what the console has.
pub async fn status(session: Option<&Session>) -> ModulesStatus {
    let cache = modhub::default_cache_dir();
    let mut modules = Vec::with_capacity(modhub::CATALOG.len());
    let mut console_checked = false;
    for module in modhub::CATALOG {
        let installed = match session {
            Some(session) => match session.exec(&installed_check(module)).await {
                Ok(output) => {
                    console_checked = true;
                    Some(output.success())
                }
                Err(error) => {
                    warn!(%error, module = module.id, "could not ask the console about a module");
                    None
                }
            },
            None => None,
        };
        let records = match session {
            Some(session) if installed.is_some() => match records(session, module).await {
                Ok(records) => Some(records),
                Err(error) => {
                    warn!(%error, module = module.id, "could not read the uninstall records");
                    None
                }
            },
            _ => None,
        };
        // Another installer's package counts: hakchi2 CE's `snes9x2010` puts `/bin/snes` on
        // the console but not the `snes9x` binary KMFD's module adds.
        let installed = installed.map(|all| all || records.as_ref().is_some_and(|r| !r.is_empty()));
        let removable = records.as_ref().map(|r| !r.is_empty());
        let records = records.unwrap_or_default();
        let cached_bytes = cache.as_deref().and_then(|c| module.cached_size(c));
        modules.push(ModuleView {
            id: module.id.to_string(),
            item: module.item.to_string(),
            title: module.title.to_string(),
            short: module.short.to_string(),
            purpose: module.purpose.to_string(),
            approx_bytes: module.len,
            warning: module.warning.map(str::to_string),
            cached: cached_bytes.is_some(),
            cached_bytes,
            installed,
            removable,
            records,
            update: None,
        });
    }
    ModulesStatus {
        cache_dir: cache.map(|c| c.display().to_string()),
        hub: modhub::HUB_URL.to_string(),
        modules,
        console_checked,
    }
}

/// How many of a module's files the update check compares: its largest, the binaries and
/// cores that change between builds.
const FINGERPRINT_FILES: usize = 3;

/// `md5sum` of `paths` on the console; missing files are left out of the output.
pub fn sums_check(paths: &[&str]) -> String {
    let quoted: Vec<String> = paths
        .iter()
        .map(|p| cartload_console::parse::sh_quote(p))
        .collect();
    format!("md5sum {} 2>/dev/null; true", quoted.join(" "))
}

/// `md5sum` output as (path, md5) pairs.
pub fn parse_sums(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .filter_map(|line| {
            let (md5, path) = line.trim().split_once(char::is_whitespace)?;
            Some((path.trim().to_string(), md5.to_ascii_lowercase()))
        })
        .collect()
}

/// Whether the console's copies of `files` differ from them (or are missing).
pub fn differs(files: &[FileSum], console: &[(String, String)]) -> bool {
    files.iter().any(|file| {
        !console
            .iter()
            .any(|(path, md5)| *path == file.path && *md5 == file.md5)
    })
}

/// The modules `status` found on the console.
fn installed(status: &ModulesStatus) -> Vec<&'static Module> {
    status
        .modules
        .iter()
        .filter(|view| view.installed == Some(true))
        .filter_map(|view| modhub::find(&view.id))
        .collect()
}

/// The first half of the update check, with no console involved: makes sure the cache holds
/// the pinned file of each installed module, downloading what is missing. False when neither
/// the mirror nor the hub could be reached.
pub async fn refresh_installed(status: &ModulesStatus) -> bool {
    let modules = installed(status);
    let Some(cache) = modhub::default_cache_dir() else {
        return false;
    };
    if modules.is_empty() {
        return true;
    }
    match modhub::refresh(&modules, &cache, |_| {}).await {
        Ok(_) => true,
        Err(error) => {
            warn!(%error, "could not download the pinned modules");
            false
        }
    }
}

/// The second half: fills in [`ModuleView::update`] for the installed modules by comparing
/// the largest files of each cached module with the console's copies. A module the check
/// cannot judge (not cached, an unreadable archive) keeps `None`.
pub async fn compare_installed(session: &Session, status: &mut ModulesStatus) {
    let Some(cache) = modhub::default_cache_dir() else {
        return;
    };
    for module in installed(status) {
        let update = match module_differs(session, module, &cache).await {
            Ok(update) => Some(update),
            Err(error) => {
                warn!(%error, module = module.id, "could not compare a module with the console");
                None
            }
        };
        if let Some(view) = status.modules.iter_mut().find(|v| v.id == module.id) {
            view.update = update;
            view.cached = true;
            view.cached_bytes = module.cached_size(&cache);
        }
    }
}

async fn module_differs(
    session: &Session,
    module: &Module,
    cache: &std::path::Path,
) -> Result<bool, CommandError> {
    let fail = |message: &str, detail: String| CommandError::new(message).with_detail(detail);
    let archive = module
        .load(cache)
        .map_err(|e| fail("The module is not in the cache.", e.to_string()))?;
    let files = archive
        .fingerprint(FINGERPRINT_FILES)
        .map_err(|e| fail("The module's archive cannot be read.", e.to_string()))?;
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let output = session
        .exec(&sums_check(&paths))
        .await
        .map_err(|e| fail("Could not read the console's files.", e.to_string()))?;
    let update = differs(&files, &parse_sums(&output.stdout_lossy()));
    info!(
        module = module.id,
        update, "compared the module with the console"
    );
    Ok(update)
}

/// Downloads the pinned file of every module in `ids` whose cache is missing or different,
/// reporting progress.
pub async fn refresh(
    ids: &[String],
    mut progress: impl FnMut(ModuleDownload) + Send,
) -> Result<(), CommandError> {
    let modules = resolve(ids)?;
    let cache = cache_dir()?;
    modhub::refresh(&modules, &cache, |p| {
        progress(ModuleDownload {
            id: modules[p.index].id.to_string(),
            done: p.done,
            total: p.total,
        });
    })
    .await
    .map(|_| ())
    .map_err(|error| {
        CommandError::new(format!("Could not download the modules: {error}"))
            .with_detail(error.to_string())
    })
}

/// Resolves ids to catalog modules.
pub fn resolve(ids: &[String]) -> Result<Vec<&'static Module>, CommandError> {
    ids.iter()
        .map(|id| {
            modhub::find(id)
                .ok_or_else(|| CommandError::new(format!("{id} is not a module Cartload knows.")))
        })
        .collect()
}

/// Downloads the modules that are not cached yet, reporting progress.
pub async fn fetch(
    ids: &[String],
    mut progress: impl FnMut(ModuleDownload) + Send,
) -> Result<(), CommandError> {
    let modules = resolve(ids)?;
    let cache = cache_dir()?;
    info!(?ids, "fetching modules");
    modhub::fetch(&modules, &cache, |p| {
        progress(ModuleDownload {
            id: modules[p.index].id.to_string(),
            done: p.done,
            total: p.total,
        });
    })
    .await
    .map_err(|error| {
        CommandError::new(format!("Could not download the modules: {error}"))
            .with_detail(error.to_string())
    })
}

/// Loads cached modules for the install flow, RetroArch first (a core installed before the
/// frontend would still work, but the order reads better in the log).
pub fn load(ids: &[String]) -> Result<Vec<ModArchive>, CommandError> {
    let cache = cache_dir()?;
    let mut modules = resolve(ids)?;
    modules.sort_by_key(|m| if m.id == modhub::RETROARCH.id { 0 } else { 1 });
    modules.dedup_by_key(|m| m.id);
    modules
        .into_iter()
        .map(|m| {
            m.load(&cache).map_err(|error| {
                CommandError::new(format!(
                    "{} is not downloaded yet. Download it first.",
                    m.title
                ))
                .with_detail(error.to_string())
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn installed_check_joins_every_binary() {
        assert_eq!(
            installed_check(&modhub::RETROARCH),
            "which retroarch-clover >/dev/null && which nes >/dev/null"
        );
        assert_eq!(installed_check(&modhub::FCEUMM), "which fceumm >/dev/null");
    }

    #[test]
    fn removing_retroarch_takes_every_core() {
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let one = removal(&ids(&["snes9x"])).expect("known id");
        assert_eq!(one.iter().map(|m| m.id).collect::<Vec<_>>(), ["snes9x"]);
        let all = removal(&ids(&["retroarch"])).expect("known id");
        assert_eq!(all.len(), modhub::CATALOG.len());
        assert_eq!(
            records_check(modhub::SNES9X.binaries),
            r#"cd /var/lib/hakchi/hmod 2>/dev/null && grep -lE '"\$rootfs/(\./)?bin/(snes9x|snes)"' uninstall-* 2>/dev/null; true"#
        );
        // What a SNES Classic with hakchi2 CE's RetroArch answered (2026-10-28).
        assert_eq!(
            parse_records("uninstall-snes9x2010\nuninstall-_km_snes9x_xtreme\n\n"),
            ["snes9x2010", "_km_snes9x_xtreme"]
        );
        assert!(parse_records("").is_empty());
    }

    /// Runs the record search with the host's `grep -E` (POSIX ERE, like the console's BusyBox
    /// grep) over records as a SNES Classic held them (2026-10-28).
    #[cfg(unix)]
    #[test]
    fn records_check_finds_a_launcher_anywhere_on_the_line() {
        let dir = tempfile::tempdir().unwrap();
        let records = [
            (
                "uninstall-_km_gambatte",
                r#"rm -f "$rootfs/./uninstall" "$rootfs/./etc/libretro/core/km_gambatte_libretro.so" "$rootfs/./bin/km_gambatte" "$rootfs/./bin/gbc" "$rootfs/./bin/gb" "$rootfs/./bin/gambatte""#,
            ),
            (
                "uninstall-tiny7zx",
                r#"rm -f "$rootfs/./uninstall" "$rootfs/./bin/tiny7zx""#,
            ),
            (
                "uninstall-_km_retroarch_1910_ozone",
                r#"rm -f "$rootfs/./usr/bin/gb" "$rootfs/./bin/retroarch-clover" "$rootfs/./bin/nes""#,
            ),
        ];
        for (name, line) in records {
            std::fs::write(
                dir.path().join(name),
                format!("\n# auto-generated\n{line}\n"),
            )
            .unwrap();
        }
        let found = |binaries: &[&str]| {
            let command =
                records_check(binaries).replace(RECORDS_DIR, &dir.path().display().to_string());
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
                .unwrap();
            let mut names = parse_records(&String::from_utf8_lossy(&output.stdout));
            names.sort();
            names
        };
        // Not the last file on its line, and not matched by /usr/bin/gb or bin/gbc alone.
        assert_eq!(found(&["gb"]), ["_km_gambatte"]);
        assert_eq!(found(&["gb", "gbc"]), ["_km_gambatte"]);
        assert_eq!(found(&["tiny7zx"]), ["tiny7zx"]);
        assert_eq!(found(&["retroarch-clover"]), ["_km_retroarch_1910_ozone"]);
        assert!(found(&["gba"]).is_empty());
    }

    #[test]
    fn removal_is_blocked_by_the_games_that_need_it() {
        assert!(removal_blocked(&[&modhub::SNES9X], &[]).is_none());
        let one =
            removal_blocked(&[&modhub::SNES9X], &["Earthbound".to_string()]).expect("blocked");
        assert!(
            one.message
                .starts_with("1 game in the library still needs SNES core")
        );
        assert!(
            one.message.contains("Remove them from the library"),
            "{}",
            one.message
        );
        let games: Vec<String> = (1..=7).map(|i| format!("Game {i}")).collect();
        let many = removal_blocked(&[&modhub::SNES9X], &games).expect("blocked");
        assert!(many.message.contains("7 games in the library still need"));
        assert!(many.message.contains("Game 5 and 2 more"));
    }

    #[test]
    fn update_check_compares_the_console_sums() {
        assert_eq!(
            sums_check(&["/bin/gba", "/etc/libretro/core/mgba_libretro.so"]),
            "md5sum '/bin/gba' '/etc/libretro/core/mgba_libretro.so' 2>/dev/null; true"
        );
        let console =
            parse_sums("0CC175B9C0F1B6A831C399E269772661  /etc/libretro/core/mgba_libretro.so\n\n");
        assert_eq!(
            console,
            [(
                "/etc/libretro/core/mgba_libretro.so".to_string(),
                "0cc175b9c0f1b6a831c399e269772661".to_string()
            )]
        );
        let file = |path: &str, md5: &str| FileSum {
            path: path.to_string(),
            md5: md5.to_string(),
        };
        let same = [file(
            "/etc/libretro/core/mgba_libretro.so",
            "0cc175b9c0f1b6a831c399e269772661",
        )];
        assert!(!differs(&same, &console));
        let changed = [file("/etc/libretro/core/mgba_libretro.so", "ffff")];
        assert!(differs(&changed, &console));
        let missing = [same[0].clone(), file("/bin/gba", "aaaa")];
        assert!(differs(&missing, &console));
    }

    #[test]
    fn resolve_rejects_unknown_ids() {
        assert!(resolve(&["retroarch".to_string(), "fceumm".to_string()]).is_ok());
        let error = resolve(&["mame".to_string()]).expect_err("unknown");
        assert!(error.message.contains("mame"));
    }

    #[test]
    fn every_system_has_its_core_in_the_catalog() {
        for system in cartload_library::System::ALL {
            let module = modhub::find(system.core_module())
                .unwrap_or_else(|| panic!("{system} has no core module"));
            assert!(
                module.binaries.contains(&system.retroarch_launcher())
                    || (system == cartload_library::System::Nes
                        && modhub::RETROARCH.binaries.contains(&"nes")),
                "{system}: {} does not provide {}",
                module.id,
                system.retroarch_launcher()
            );
        }
        assert!(modhub::MUPEN64.warning.is_some());
    }

    #[tokio::test]
    async fn status_without_a_console_lists_the_catalog() {
        let status = status(None).await;
        assert_eq!(status.modules.len(), modhub::CATALOG.len());
        assert!(!status.console_checked);
        assert!(status.modules.iter().all(|m| m.installed.is_none()));
        assert_eq!(status.modules[0].id, "retroarch");
    }
}
