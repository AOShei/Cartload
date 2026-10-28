//! Emulator modules (RetroArch and cores) from KMFD's Mod Hub: what is cached on this
//! machine, what the connected console has, downloading, and loading for the install flow.

use std::path::PathBuf;

use cartload_console::Session;
use cartload_hmod::modhub::{self, ModArchive, Module};
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
    /// Rough download size.
    pub approx_bytes: u64,
    /// A caution to show before installing.
    pub warning: Option<String>,
    /// Cached on this machine.
    pub cached: bool,
    /// Size of the cached file.
    pub cached_bytes: Option<u64>,
    /// Installed on the connected console (`None` when no console answered).
    pub installed: Option<bool>,
    /// Installed with an uninstall record under its hub name, so Cartload can remove it
    /// (`None` when no console answered).
    pub removable: Option<bool>,
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

/// Exit code 0 when the console has the module's uninstall record (what `pack_install`
/// writes and `hakchi pack_uninstall` runs).
pub fn registered_check(module: &Module) -> String {
    format!("[ -f '/var/lib/hakchi/hmod/uninstall-{}' ]", module.item)
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
            "{} game{} in the library still need{} {titles}: {shown}{more}. Remove them from              the library (or switch them to the built-in emulator) and sync first.",
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
        let removable = match (session, installed) {
            (Some(session), Some(true)) => session
                .exec(&registered_check(module))
                .await
                .ok()
                .map(|output| output.success()),
            (_, Some(false)) => Some(false),
            _ => None,
        };
        let cached_bytes = cache.as_deref().and_then(|c| module.cached_size(c));
        modules.push(ModuleView {
            id: module.id.to_string(),
            item: module.item.to_string(),
            title: module.title.to_string(),
            short: module.short.to_string(),
            purpose: module.purpose.to_string(),
            approx_bytes: module.approx_bytes,
            warning: module.warning.map(str::to_string),
            cached: cached_bytes.is_some(),
            cached_bytes,
            installed,
            removable,
        });
    }
    ModulesStatus {
        cache_dir: cache.map(|c| c.display().to_string()),
        hub: modhub::HUB_URL.to_string(),
        modules,
        console_checked,
    }
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
            registered_check(&modhub::SNES9X),
            "[ -f '/var/lib/hakchi/hmod/uninstall-_km_snes9x_xtreme' ]"
        );
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
        let games: Vec<String> = (1..=7).map(|i| format!("Game {i}")).collect();
        let many = removal_blocked(&[&modhub::SNES9X], &games).expect("blocked");
        assert!(many.message.contains("7 games in the library still need"));
        assert!(many.message.contains("Game 5 and 2 more"));
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
