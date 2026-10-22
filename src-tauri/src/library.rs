//! The games library inside the app: one [`Library`] opened at startup, the folder tree the
//! console will show (automatic structure or the tree edited by hand), imports with one
//! event per file, cover art from TheGamesDB, and the views the Library page shows.
//!
//! Long library tasks (an import, an art fetch, a sync) run one at a time: each takes the
//! [`LibraryState::begin`] slot and the others are refused with a sentence naming the one in
//! progress.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use cartload_console::ConsoleInfo;
use cartload_library::{
    Game, HashDb, Library, Menu, MenuOptions,
    art::{self, ApiKey, ArtEvent, ArtOptions, ArtReport, Client, HttpTransport},
    codes, hashdb,
    images::CoverOptions,
    menu::{self, Entry, FolderImage, GameRef, MenuItem, MenuPath},
    rom,
};
use serde::{Deserialize, Serialize};
use tauri::http;
use tracing::{info, warn};

use crate::{error::CommandError, status::ConsoleStatus};

/// Event carrying an [`ImportEvent`] per file while an import runs.
pub const IMPORT_EVENT: &str = "library-import";
/// Event carrying an [`ArtProgress`] while art is fetched.
pub const ART_EVENT: &str = "art-progress";
/// The URI scheme the covers are served on (`cover://localhost/<code>/<file>`).
pub const COVER_SCHEME: &str = "cover";

/// Extensions the open dialog offers (ROMs and zips of them).
pub const IMPORT_EXTENSIONS: [&str; 6] = ["sfc", "smc", "fig", "swc", "nes", "zip"];

/// A game as the Library page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GameView {
    /// The game as stored.
    pub game: Game,
    /// Bytes of the files that go to the console (ROM and art).
    pub size: u64,
    /// URL of the cover; changes when the art changes.
    pub cover: String,
    /// URL of the thumbnail.
    pub thumbnail: String,
    /// Short system name (`SNES`, `NES`).
    pub system_name: String,
}

/// The library as a whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LibraryView {
    /// Where it lives.
    pub root: String,
    /// Every game, sorted by code.
    pub games: Vec<GameView>,
    /// Bytes of all game files.
    pub total_size: u64,
}

/// A firmware game, for the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OriginalView {
    /// `CLV-P-...`.
    pub code: String,
    /// Title from its launch entry.
    pub name: String,
    /// URL of the firmware's cover, when cached.
    pub cover: Option<String>,
    /// URL of the firmware's thumbnail, when cached.
    pub thumbnail: Option<String>,
}

/// The folder tree the console will show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MenuView {
    /// The root menu's entries.
    pub entries: Vec<Entry>,
    /// The automatic structure options.
    pub options: MenuOptions,
    /// Whether the tree was edited by hand (then `options` are what "apply" would use).
    pub custom: bool,
    /// System code of the console the tree is for (`snes-eur`), when known.
    pub system_code: Option<String>,
    /// The firmware games of that console, when cached.
    pub originals: Vec<OriginalView>,
    /// The pictures chosen for folders, by stem.
    pub folder_art: BTreeMap<String, FolderArtView>,
}

/// Where the webview loads a folder's chosen picture from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FolderArtView {
    /// 228 × 204.
    pub cover: String,
    /// 40 × 40.
    pub thumbnail: String,
}

/// One game that an import produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportedGame {
    /// Game code.
    pub code: String,
    /// Title.
    pub name: String,
    /// `SNES`, `NES`.
    pub system: String,
    /// Whether the hash database knew the ROM.
    pub matched: bool,
    /// Whether the game replaced one with the same code.
    pub replaced: bool,
    /// Things that may go wrong on the console.
    pub warnings: Vec<String>,
    /// Things that were done and are fine.
    pub notes: Vec<String>,
}

/// What became of one file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ImportOutcome {
    /// One or more games.
    Imported {
        /// The games.
        games: Vec<ImportedGame>,
    },
    /// Nothing; the error says why.
    Failed {
        /// The error.
        error: String,
    },
    /// Not a file the importer takes.
    Skipped {
        /// Why.
        reason: String,
    },
}

/// Payload of the import event: a file is about to be imported (no outcome yet) or done.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportEvent {
    /// Zero-based index of the file.
    pub index: usize,
    /// Number of files.
    pub total: usize,
    /// The file.
    pub path: String,
    /// The outcome once known.
    pub outcome: Option<ImportOutcome>,
}

/// What an import did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    /// One entry per file, in order.
    pub files: Vec<ImportEvent>,
    /// Games imported.
    pub imported: usize,
    /// Games the hash database did not know.
    pub unmatched: usize,
    /// Files that failed.
    pub failed: usize,
    /// Files skipped.
    pub skipped: usize,
    /// Codes of the games imported.
    pub codes: Vec<String>,
}

/// The fields of a game the detail panel edits.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GameEdit {
    /// Which game.
    pub code: String,
    /// Title.
    pub name: String,
    /// Sort key, empty to derive it from the title.
    #[serde(default)]
    pub sort_name: String,
    /// Publisher.
    #[serde(default)]
    pub publisher: String,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub release_date: String,
    /// 1 or 2.
    #[serde(default = "one")]
    pub players: u8,
    /// Both players at once.
    #[serde(default)]
    pub simultaneous: bool,
    /// Copyright line.
    #[serde(default)]
    pub copyright: String,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// The emulator command (advanced).
    #[serde(default)]
    pub application: String,
    /// Arguments after the ROM path (advanced).
    #[serde(default)]
    pub args: String,
    /// Compress the ROM for the console (7z); `None` leaves the setting alone.
    #[serde(default)]
    pub compress: Option<bool>,
}

fn one() -> u8 {
    1
}

/// Applies an edit to a game, checking the fields.
pub fn apply_edit(game: &mut Game, edit: &GameEdit) -> Result<(), CommandError> {
    let name = edit.name.trim();
    if name.is_empty() {
        return Err(CommandError::new("The game needs a name."));
    }
    if !(1..=2).contains(&edit.players) {
        return Err(CommandError::new("Players must be 1 or 2."));
    }
    let release_date = if edit.release_date.trim().is_empty() {
        "1900-01-01".to_string()
    } else {
        art::valid_date(&edit.release_date).ok_or_else(|| {
            CommandError::new(
                "The release date must be written as YYYY-MM-DD, for example 1995-03-11.",
            )
        })?
    };
    let application = edit.application.trim();
    if application.is_empty() {
        return Err(CommandError::new(
            "The launch command needs the emulator, for example /bin/clover-canoe-shvc-wr -rom.",
        ));
    }
    game.name = name.to_string();
    game.sort_name = edit.sort_name.trim().to_string();
    game.publisher = {
        let publisher = edit.publisher.trim();
        if publisher.is_empty() {
            "UNKNOWN".to_string()
        } else {
            publisher.to_string()
        }
    };
    game.release_date = release_date;
    game.players = edit.players;
    game.simultaneous = edit.players == 2 && edit.simultaneous;
    game.copyright = edit.copyright.trim().to_string();
    game.description = edit.description.trim().to_string();
    game.application = application.to_string();
    game.args = edit.args.trim().to_string();
    Ok(())
}

/// A change to the folder tree other than a move.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MenuEdit {
    /// A new, empty folder.
    AddFolder {
        /// The menu it goes in.
        parent: MenuPath,
        /// Its title.
        name: String,
    },
    /// A new title for a folder.
    RenameFolder {
        /// The folder.
        path: MenuPath,
        /// The title.
        name: String,
    },
    /// Removes a folder; its contents move to the parent menu.
    RemoveFolder {
        /// The folder.
        path: MenuPath,
    },
    /// A picture for a folder: an image file to make it from, or none for the plain icon.
    SetFolderImage {
        /// The folder.
        path: MenuPath,
        /// The image file, any supported format.
        file: Option<String>,
    },
}

/// Progress of an art fetch: the crate's event with a running count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArtProgress {
    /// Games finished (done or failed).
    pub done: usize,
    /// Games in the run.
    pub total: usize,
    /// What happened.
    pub event: ArtEvent,
}

/// What an art fetch did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArtOutcome {
    /// The crate's report.
    pub report: ArtReport,
    /// One line summing it up.
    pub summary: String,
}

/// Where the TheGamesDB key comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyStatus {
    /// A key is stored in the library settings.
    pub stored: bool,
    /// The `CARTLOAD_TGDB_KEY` variable is set (it wins over the stored key).
    pub environment: bool,
    /// Requests left this month, when the API has said.
    pub remaining: Option<u32>,
}

enum HashDbSlot {
    Unloaded,
    Loaded(Arc<HashDb>),
    Unavailable,
}

/// Keeps the long-task slot taken while alive.
pub struct BusyGuard {
    state: Arc<LibraryState>,
}

impl std::fmt::Debug for BusyGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BusyGuard({:?})", self.state.busy_with())
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        *self
            .state
            .busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// The library and what the app keeps around it.
pub struct LibraryState {
    library: Library,
    hash_db: tokio::sync::Mutex<HashDbSlot>,
    busy: Mutex<Option<&'static str>>,
}

impl LibraryState {
    /// Opens the library at `CARTLOAD_LIBRARY` or the user data directory.
    pub fn open() -> Result<Self, CommandError> {
        let root = match std::env::var_os("CARTLOAD_LIBRARY") {
            Some(root) => PathBuf::from(root),
            None => Library::default_root()
                .ok_or_else(|| CommandError::new("No user data directory on this system."))?,
        };
        Self::at(root)
    }

    /// Opens the library at `root`.
    pub fn at(root: impl Into<PathBuf>) -> Result<Self, CommandError> {
        let library = Library::open(root)?;
        info!(root = %library.root().display(), "library");
        Ok(Self {
            library,
            hash_db: tokio::sync::Mutex::new(HashDbSlot::Unloaded),
            busy: Mutex::new(None),
        })
    }

    /// The library.
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// Takes the long-task slot for `what` (`import`, `art`, `sync`), or says what is running.
    pub fn begin(self: &Arc<Self>, what: &'static str) -> Result<BusyGuard, CommandError> {
        let mut busy = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(running) = *busy {
            return Err(CommandError::new(format!(
                "Wait for the {} to finish first.",
                describe_task(running)
            )));
        }
        *busy = Some(what);
        Ok(BusyGuard {
            state: Arc::clone(self),
        })
    }

    /// The task in progress, if any.
    pub fn busy_with(&self) -> Option<&'static str> {
        *self.busy.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every game with its size and cover URL.
    pub fn view(&self) -> Result<LibraryView, CommandError> {
        let mut games = Vec::new();
        let mut total_size = 0;
        for game in self.library.games()? {
            let view = self.game_view(game)?;
            total_size += view.size;
            games.push(view);
        }
        Ok(LibraryView {
            root: self.library.root().display().to_string(),
            games,
            total_size,
        })
    }

    /// One game with its size and cover URL.
    pub fn game_view(&self, game: Game) -> Result<GameView, CommandError> {
        let files = self.library.game_files(&game)?;
        let size = files.iter().map(|f| f.size).sum();
        let version = |name: &str| {
            files
                .iter()
                .find(|f| f.name == name)
                .map_or(0, |f| f.mtime.unsigned_abs() ^ f.size)
        };
        let cover = cover_url(&game.code, &game.cover_file(), version(&game.cover_file()));
        let thumbnail = cover_url(
            &game.code,
            &game.thumbnail_file(),
            version(&game.thumbnail_file()),
        );
        Ok(GameView {
            system_name: short_system_name(game.system).to_string(),
            size,
            cover,
            thumbnail,
            game,
        })
    }

    /// Saves an edit and returns the game.
    pub fn update(&self, edit: &GameEdit) -> Result<GameView, CommandError> {
        let mut game = self.library.game(&edit.code)?;
        apply_edit(&mut game, edit)?;
        if let Some(compress) = edit.compress
            && compress != game.compress
        {
            let report = self.library.set_compress(&mut game, compress)?;
            info!(
                code = game.code,
                compress,
                archive_size = report.archive_size,
                "compression changed"
            );
        }
        self.library.save(&game)?;
        info!(code = game.code, name = game.name, "game updated");
        self.game_view(game)
    }

    /// Removes games and takes them out of the edited tree.
    pub fn remove(&self, codes: &[String]) -> Result<usize, CommandError> {
        let mut removed = 0;
        for code in codes {
            self.library.remove(code)?;
            removed += 1;
        }
        self.reconcile_custom(None)?;
        Ok(removed)
    }

    /// The system code the tree is for: the connected console's, else the last one seen.
    pub fn system_code(&self, status: &ConsoleStatus) -> Result<Option<String>, CommandError> {
        if let ConsoleStatus::Connected { info, .. } = status
            && let Some(code) = &info.system_code
        {
            return Ok(Some(code.clone()));
        }
        Ok(self.library.config()?.last_console)
    }

    /// The library's games and the firmware games of `system_code`, as the menu builder
    /// takes them.
    pub fn game_refs(&self, system_code: Option<&str>) -> Result<Vec<GameRef>, CommandError> {
        let mut refs: Vec<GameRef> = self
            .library
            .games()?
            .iter()
            .map(|g| GameRef {
                code: g.code.clone(),
                sort_title: g.sort_title(),
                original: false,
            })
            .collect();
        if let Some(system_code) = system_code {
            for (code, entry, _) in self.library.originals(system_code)? {
                refs.push(GameRef {
                    code,
                    sort_title: entry.sort_title.clone(),
                    original: true,
                });
            }
        }
        Ok(refs)
    }

    /// The tree for `system_code`: the edited one, brought in line with the library, or the
    /// automatic structure. Also says whether it is the edited one.
    pub fn menu(
        &self,
        system_code: Option<&str>,
    ) -> Result<(Menu, MenuOptions, bool), CommandError> {
        let mut config = self.library.config()?;
        let refs = self.game_refs(system_code)?;
        match config.custom_menu.take() {
            Some(mut custom) => {
                if custom.reconcile(&refs) {
                    config.custom_menu = Some(custom.clone());
                    self.library.save_config(&config)?;
                }
                Ok((custom, config.menu, true))
            }
            None => Ok((menu::build(&refs, &config.menu), config.menu, false)),
        }
    }

    /// The tree as the page shows it.
    pub fn menu_view(&self, status: &ConsoleStatus) -> Result<MenuView, CommandError> {
        let system_code = self.system_code(status)?;
        let (menu, options, custom) = self.menu(system_code.as_deref())?;
        let originals = match &system_code {
            Some(system) => self
                .library
                .originals(system)?
                .into_iter()
                .map(|(code, entry, _)| {
                    let art = |suffix: &str| {
                        let file = format!("{code}{suffix}");
                        self.library
                            .original_art(system, &file)
                            .and_then(|path| std::fs::metadata(path).ok())
                            .map(|meta| original_art_url(system, &code, &file, meta.len()))
                    };
                    OriginalView {
                        cover: art(".png"),
                        thumbnail: art("_small.png"),
                        code,
                        name: entry.name,
                    }
                })
                .collect(),
            None => Vec::new(),
        };
        let folder_art = self.folder_art(&menu.entries);
        Ok(MenuView {
            entries: menu.entries,
            options,
            custom,
            system_code,
            originals,
            folder_art,
        })
    }

    /// Replaces the edited tree with an automatic structure.
    pub fn set_structure(&self, options: MenuOptions) -> Result<(), CommandError> {
        if let menu::Structure::Count { max } = options.structure
            && max == 0
        {
            return Err(CommandError::new(
                "A folder needs room for at least one game.",
            ));
        }
        let mut config = self.library.config()?;
        config.menu = options;
        config.custom_menu = None;
        self.library.save_config(&config)?;
        info!(structure = %config.menu.structure, "menu structure set");
        Ok(())
    }

    /// Changes the tree, turning an automatic structure into an edited tree first.
    pub fn edit_menu(
        &self,
        status: &ConsoleStatus,
        change: impl FnOnce(&mut Menu) -> cartload_library::Result<()>,
    ) -> Result<(), CommandError> {
        let system_code = self.system_code(status)?;
        let (mut menu, _, _) = self.menu(system_code.as_deref())?;
        change(&mut menu)?;
        let mut config = self.library.config()?;
        config.custom_menu = Some(menu);
        Ok(self.library.save_config(&config)?)
    }

    /// Moves items in the tree.
    pub fn move_items(
        &self,
        status: &ConsoleStatus,
        items: &[MenuItem],
        target: &[usize],
        index: Option<usize>,
    ) -> Result<(), CommandError> {
        self.edit_menu(status, |menu| menu.move_items(items, target, index))
    }

    /// Applies a folder edit.
    pub fn apply_menu_edit(
        &self,
        status: &ConsoleStatus,
        edit: &MenuEdit,
    ) -> Result<(), CommandError> {
        // The image file is read here, outside the tree edit, so a missing file is a plain
        // command error and the tree stays untouched.
        let picture = match edit {
            MenuEdit::SetFolderImage {
                file: Some(file), ..
            } => Some(std::fs::read(file).map_err(|e| {
                CommandError::new(format!("The image could not be read: {file}"))
                    .with_detail(e.to_string())
            })?),
            _ => None,
        };
        self.edit_menu(status, |menu| match edit {
            MenuEdit::AddFolder { parent, name } => menu.add_folder(parent, name).map(|_| ()),
            MenuEdit::RenameFolder { path, name } => menu.rename_folder(path, name),
            MenuEdit::RemoveFolder { path } => menu.remove_folder(path),
            MenuEdit::SetFolderImage { path, file } => {
                let previous = match menu.folder_image(path) {
                    Some(FolderImage::Custom(stem)) => Some(stem.clone()),
                    _ => None,
                };
                let image = match (file, &picture) {
                    (Some(_), Some(bytes)) => FolderImage::Custom(
                        self.library
                            .store_folder_image(bytes, &CoverOptions::default())?,
                    ),
                    _ => FolderImage::Folder,
                };
                menu.set_folder_image(path, image)?;
                if let Some(stem) = previous {
                    self.library.remove_folder_image(&stem);
                }
                Ok(())
            }
        })
    }

    /// The URLs of every custom folder picture in the tree.
    fn folder_art(&self, entries: &[Entry]) -> BTreeMap<String, FolderArtView> {
        let mut out = BTreeMap::new();
        for entry in entries {
            if let Entry::Folder(folder) = entry {
                if let FolderImage::Custom(stem) = &folder.image {
                    let (cover, thumbnail) = self.library.folder_image_files(stem);
                    out.insert(
                        stem.clone(),
                        FolderArtView {
                            cover: folder_art_url(stem, &format!("{stem}.png"), file_size(&cover)),
                            thumbnail: folder_art_url(
                                stem,
                                &format!("{stem}_small.png"),
                                file_size(&thumbnail),
                            ),
                        },
                    );
                }
                out.extend(self.folder_art(&folder.entries));
            }
        }
        out
    }

    /// Brings the edited tree in line with the library, when there is one. With `into`, the
    /// games that are new to the tree go to that folder.
    fn reconcile_custom(&self, into: Option<(&[usize], &[String])>) -> Result<(), CommandError> {
        let mut config = self.library.config()?;
        let Some(mut custom) = config.custom_menu.take() else {
            return Ok(());
        };
        let refs = self.game_refs(config.last_console.as_deref())?;
        let mut changed = custom.reconcile(&refs);
        if let Some((target, codes)) = into
            && !target.is_empty()
        {
            let items: Vec<MenuItem> = codes
                .iter()
                .map(|code| MenuItem::Game { code: code.clone() })
                .collect();
            match custom.move_items(&items, target, None) {
                Ok(()) => changed = true,
                Err(error) => warn!(%error, "imported games stay in the root menu"),
            }
        }
        if changed {
            config.custom_menu = Some(custom);
            self.library.save_config(&config)?;
        }
        Ok(())
    }

    /// Caches the firmware's launch entries of a connected console once, for the tree and
    /// the sync, and the firmware's art of each stock game that has none cached yet. Needs
    /// the session; reads only; failures are logged, not raised.
    pub async fn ensure_originals(&self, info: &ConsoleInfo, session: &cartload_console::Session) {
        let Some(code) = info.system_code.as_deref() else {
            return;
        };
        if info.original_games.is_empty() {
            return;
        }
        let paths = match cartload_library::DevicePaths::from_info(info) {
            Ok(paths) => paths,
            Err(error) => {
                warn!(%error, "no device paths for the firmware entries");
                return;
            }
        };
        match self.library.originals(code) {
            Ok(cached) if !cached.is_empty() => {}
            Ok(_) => {
                let originals =
                    match cartload_library::sync::fetch_originals(session, &paths.squashfs_games)
                        .await
                    {
                        Ok(originals) => originals,
                        Err(error) => {
                            warn!(%error, "could not read the firmware's launch entries");
                            return;
                        }
                    };
                match self.library.store_originals(code, &originals) {
                    Ok(()) => info!(
                        count = originals.len(),
                        code, "cached the firmware's launch entries"
                    ),
                    Err(error) => {
                        warn!(%error, "could not store the firmware entries");
                        return;
                    }
                }
            }
            Err(error) => {
                warn!(%error, "could not read the cached firmware entries");
                return;
            }
        }
        self.ensure_original_art(code, &paths.squashfs_games, session)
            .await;
    }

    /// Fetches the cover and thumbnail the firmware ships for each cached stock game, skipping
    /// files already cached. A failure leaves the glyph in the grid and is logged.
    async fn ensure_original_art(
        &self,
        code: &str,
        squashfs_games: &str,
        session: &cartload_console::Session,
    ) {
        let missing = match self.library.missing_original_art(code) {
            Ok(missing) => missing,
            Err(error) => {
                warn!(%error, "could not check the cached firmware art");
                return;
            }
        };
        if missing.is_empty() {
            return;
        }
        match cartload_library::sync::fetch_original_art(session, squashfs_games, &missing).await {
            Ok(files) => match self.library.store_original_art(code, &files) {
                Ok(stored) => info!(
                    stored,
                    wanted = missing.len(),
                    code,
                    "cached the firmware's art"
                ),
                Err(error) => warn!(%error, "could not store the firmware's art"),
            },
            Err(error) => warn!(%error, "could not read the firmware's art"),
        }
    }

    /// The hash database: loaded once, downloaded into the cache when missing. `None` when
    /// it cannot be had (names then come from the file names).
    pub async fn hash_db(&self) -> Option<Arc<HashDb>> {
        let mut slot = self.hash_db.lock().await;
        match &*slot {
            HashDbSlot::Loaded(db) => return Some(Arc::clone(db)),
            HashDbSlot::Unavailable => return None,
            HashDbSlot::Unloaded => {}
        }
        let loaded = load_hash_db().await;
        *slot = match &loaded {
            Some(db) => HashDbSlot::Loaded(Arc::clone(db)),
            None => HashDbSlot::Unavailable,
        };
        loaded
    }

    /// Imports files and folders (walked for ROMs and zips), one event per file. New games
    /// go to `folder` when the tree is edited by hand.
    pub async fn import(
        &self,
        paths: &[PathBuf],
        folder: Option<&[usize]>,
        mut emit: impl FnMut(&ImportEvent),
    ) -> Result<ImportReport, CommandError> {
        let files = expand_paths(paths);
        if files.is_empty() {
            return Err(CommandError::new(
                "Nothing to import: choose .sfc, .smc, .fig, .swc or .nes files, zips of them, or a folder holding some.",
            ));
        }
        let db = self.hash_db().await;
        let total = files.len();
        let mut report = ImportReport {
            files: Vec::with_capacity(total),
            imported: 0,
            unmatched: 0,
            failed: 0,
            skipped: 0,
            codes: Vec::new(),
        };
        for (index, path) in files.into_iter().enumerate() {
            let mut event = ImportEvent {
                index,
                total,
                path: path.display().to_string(),
                outcome: None,
            };
            emit(&event);
            let outcome = if is_importable(&path) {
                let library = self.library.clone();
                let db = db.clone();
                let file = path.clone();
                let result =
                    tokio::task::spawn_blocking(move || library.import(&file, db.as_deref()))
                        .await
                        .map_err(|e| CommandError::new(format!("import task failed: {e}")))?;
                match result {
                    Ok(imported) => {
                        let games: Vec<ImportedGame> = imported
                            .into_iter()
                            .map(|i| ImportedGame {
                                code: i.game.code.clone(),
                                name: i.game.name.clone(),
                                system: short_system_name(i.game.system).to_string(),
                                matched: i.matched,
                                replaced: i.replaced,
                                warnings: i.warnings,
                                notes: i.notes,
                            })
                            .collect();
                        report.imported += games.len();
                        report.unmatched += games.iter().filter(|g| !g.matched).count();
                        report.codes.extend(games.iter().map(|g| g.code.clone()));
                        ImportOutcome::Imported { games }
                    }
                    Err(error) => {
                        warn!(path = %path.display(), %error, "import failed");
                        report.failed += 1;
                        ImportOutcome::Failed {
                            error: error.to_string(),
                        }
                    }
                }
            } else {
                report.skipped += 1;
                ImportOutcome::Skipped {
                    reason: "not a ROM (.sfc, .smc, .fig, .swc, .nes) or a zip".to_string(),
                }
            };
            event.outcome = Some(outcome);
            emit(&event);
            report.files.push(event);
        }
        self.reconcile_custom(folder.map(|f| (f, report.codes.as_slice())))?;
        info!(
            imported = report.imported,
            unmatched = report.unmatched,
            failed = report.failed,
            skipped = report.skipped,
            "import finished"
        );
        Ok(report)
    }

    /// The TheGamesDB key status.
    pub fn key_status(&self) -> Result<KeyStatus, CommandError> {
        let config = self.library.config()?;
        let remaining = self
            .library
            .art_store()
            .and_then(|store| store.allowance())
            .ok()
            .and_then(|a| a.remaining);
        Ok(KeyStatus {
            stored: config.tgdb_key.is_some(),
            environment: ApiKey::from_env().is_some(),
            remaining,
        })
    }

    /// Stores or clears the TheGamesDB key.
    pub fn set_key(&self, key: Option<&str>) -> Result<KeyStatus, CommandError> {
        let mut config = self.library.config()?;
        config.tgdb_key = match key.map(str::trim) {
            Some(key) if !key.is_empty() => Some(
                ApiKey::new(key)
                    .ok_or_else(|| CommandError::new("The key is empty."))?
                    .as_str()
                    .to_string(),
            ),
            _ => None,
        };
        self.library.save_config(&config)?;
        info!(stored = config.tgdb_key.is_some(), "TheGamesDB key changed");
        self.key_status()
    }

    /// Fetches art and metadata for `codes`, one event per step.
    pub async fn fetch_art(
        &self,
        codes: &[String],
        force: bool,
        mut emit: impl FnMut(&ArtProgress),
    ) -> Result<ArtOutcome, CommandError> {
        if codes.is_empty() {
            return Err(CommandError::new("Select a game first."));
        }
        let config = self.library.config()?;
        let key = ApiKey::resolve(None, config.tgdb_key.as_deref()).ok_or_else(|| {
            CommandError::new(
                "Fetching art needs a TheGamesDB API key. Add one on the Settings page; keys are free at thegamesdb.net.",
            )
        })?;
        let client = Client::new(HttpTransport::new()?, key);
        let options = ArtOptions {
            force,
            ..ArtOptions::default()
        };
        let total = codes.len();
        let mut done = 0;
        let report = art::fetch_art(&self.library, &client, codes, &options, |event| {
            if matches!(event, ArtEvent::Done { .. } | ArtEvent::Failed { .. }) {
                done += 1;
            }
            emit(&ArtProgress { done, total, event });
        })
        .await
        .map_err(|error| match error {
            cartload_library::Error::InvalidApiKey { .. } => CommandError::from_error(&error)
                .with_message("TheGamesDB rejected the API key. Check it on the Settings page."),
            cartload_library::Error::AllowanceExhausted { .. } => {
                CommandError::from_error(&error).with_message(format!("{error}. Try again later."))
            }
            other => CommandError::from_error(&other),
        })?;
        let summary = report.summary();
        info!("{summary}");
        Ok(ArtOutcome { report, summary })
    }

    /// Uses an image file as the cover of a game.
    pub fn set_cover(&self, code: &str, path: &Path) -> Result<GameView, CommandError> {
        let set = art::set_cover_from_file(&self.library, code, path, &CoverOptions::default())?;
        info!(code, cover = ?set.cover_size, "cover set from a file");
        self.game_view(self.library.game(code)?)
    }

    /// The HTTP response of the cover protocol for `path` (`/<code>/<file>`).
    pub fn cover_response(&self, path: &str) -> http::Response<Vec<u8>> {
        match self.cover_bytes(path) {
            Some(bytes) => http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, "image/png")
                .header(http::header::CACHE_CONTROL, "max-age=31536000, immutable")
                // WebKitGTK treats `cover://localhost` as a foreign origin and refuses to
                // display it from the page's origin (`http://localhost:1420` in dev,
                // `tauri://localhost` in a build) unless the load goes through CORS, so the
                // `<img>` tags load with `crossorigin` and this header allows them, the same
                // way Tauri's own asset protocol does.
                .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(bytes)
                .unwrap_or_else(|_| http::Response::new(Vec::new())),
            None => http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(Vec::new())
                .unwrap_or_else(|_| http::Response::new(Vec::new())),
        }
    }

    fn cover_bytes(&self, path: &str) -> Option<Vec<u8>> {
        let path = path.split('?').next().unwrap_or(path);
        let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match segments.as_slice() {
            // `/<code>/<file>`: a library game's own art.
            [code, file] => {
                if !codes::is_code(code) || !is_art_file(code, file) {
                    return None;
                }
                std::fs::read(self.library.game_dir(code).join(file)).ok()
            }
            // `/folders/<stem>/<file>`: a picture chosen for a folder.
            ["folders", stem, file] => {
                if !is_folder_stem(stem) || !is_art_file(stem, file) {
                    return None;
                }
                std::fs::read(self.library.folders_dir().join(file)).ok()
            }
            // `/<system>/<code>/<file>`: the firmware's art of a stock game, cached per console.
            [system, code, file] => {
                if !codes::is_original(code) || !is_art_file(code, file) {
                    return None;
                }
                std::fs::read(self.library.original_art(system, file)?).ok()
            }
            _ => None,
        }
    }
}

/// Whether `stem` names a stored folder picture (`f` and hex digits, as the library makes
/// them): nothing else reaches the file system.
fn is_folder_stem(stem: &str) -> bool {
    let mut chars = stem.chars();
    chars.next() == Some('f') && stem.len() > 1 && chars.all(|c| c.is_ascii_hexdigit())
}

/// Whether `file` is the cover (`<code>.png`) or the thumbnail (`<code>_small.png`) of `code`.
fn is_art_file(code: &str, file: &str) -> bool {
    file == format!("{code}.png") || file == format!("{code}_small.png")
}

/// `SNES`, `NES`, `Mega Drive`.
pub fn short_system_name(system: cartload_library::System) -> &'static str {
    match system {
        cartload_library::System::Snes => "SNES",
        cartload_library::System::Nes => "NES",
        cartload_library::System::MegaDrive => "Mega Drive",
    }
}

fn describe_task(what: &str) -> &str {
    match what {
        "import" => "import",
        "art" => "art fetch",
        "sync" => "sync",
        other => other,
    }
}

/// The URL the webview loads a cover from.
pub fn cover_url(code: &str, file: &str, version: u64) -> String {
    if cfg!(any(windows, target_os = "android")) {
        format!("http://{COVER_SCHEME}.localhost/{code}/{file}?v={version}")
    } else {
        format!("{COVER_SCHEME}://localhost/{code}/{file}?v={version}")
    }
}

/// Size of a file, 0 when it is missing: enough of a version for a picture stored under a
/// fresh stem each time.
fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// The URL the webview loads a folder's chosen picture from (`/folders/<stem>/<file>`).
pub fn folder_art_url(stem: &str, file: &str, version: u64) -> String {
    cover_url(&format!("folders/{stem}"), file, version)
}

/// The URL the webview loads a stock game's firmware art from (`/<system>/<code>/<file>`).
pub fn original_art_url(system: &str, code: &str, file: &str, version: u64) -> String {
    cover_url(&format!("{system}/{code}"), file, version)
}

fn is_importable(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(rom::is_importable)
}

/// Files and the importable files inside folders (walked, hidden entries skipped), in a
/// stable order. Files given directly are kept even when they are not importable, so they
/// get a "skipped" line.
pub fn expand_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                !p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
            })
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, out);
            } else if is_importable(&path) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            walk(path, &mut out);
        } else {
            out.push(path.clone());
        }
    }
    out
}

async fn load_hash_db() -> Option<Arc<HashDb>> {
    let path = match hashdb::default_cache_path() {
        Some(path) => path,
        None => {
            warn!("no cache directory for the hash database; names come from the file names");
            return None;
        }
    };
    if !path.exists() {
        info!(url = hashdb::ROMFILES_URL, "downloading the hash database");
        match hashdb::fetch(&path).await {
            Ok(len) => info!(path = %path.display(), len, "hash database cached"),
            Err(error) => {
                warn!(%error, "could not download the hash database; names come from the file names");
                return None;
            }
        }
    }
    let load_path = path.clone();
    match tokio::task::spawn_blocking(move || HashDb::load(&load_path)).await {
        Ok(Ok(db)) => {
            info!(path = %path.display(), records = db.len(), "hash database loaded");
            Some(Arc::new(db))
        }
        Ok(Err(error)) => {
            warn!(%error, "could not read the hash database; names come from the file names");
            None
        }
        Err(error) => {
            warn!(%error, "hash database task failed");
            None
        }
    }
}

impl From<cartload_library::Error> for CommandError {
    fn from(error: cartload_library::Error) -> Self {
        Self::from_error(&error)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cartload_library::menu::Structure;
    use serde_json::json;

    /// A minimal iNES image: 16-byte header, one 16 KiB PRG bank filled with `fill`.
    pub(crate) fn nes_rom(fill: u8, battery: bool) -> Vec<u8> {
        let mut rom = vec![0u8; 16 + 16 * 1024];
        rom[..4].copy_from_slice(b"NES\x1a");
        rom[4] = 1;
        rom[5] = 0;
        rom[6] = if battery { 0x02 } else { 0x00 };
        for byte in &mut rom[16..] {
            *byte = fill;
        }
        rom
    }

    pub(crate) fn temp_state() -> (tempfile::TempDir, Arc<LibraryState>) {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(LibraryState::at(dir.path().join("lib")).unwrap());
        (dir, state)
    }

    fn no_console() -> ConsoleStatus {
        ConsoleStatus::Disconnected {
            hint: crate::status::Hint::Nothing,
        }
    }

    async fn import_two(dir: &Path, state: &LibraryState) -> ImportReport {
        let a = dir.join("Alpha Game.nes");
        let b = dir.join("sub").join("Beta (USA).nes");
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, nes_rom(1, false)).unwrap();
        std::fs::write(&b, nes_rom(2, true)).unwrap();
        std::fs::write(dir.join("sub").join("notes.txt"), "x").unwrap();
        let mut events = Vec::new();
        let report = state
            .import(&[a, dir.join("sub"), dir.join("missing.txt")], None, |e| {
                events.push(e.clone())
            })
            .await
            .unwrap();
        assert_eq!(events.len(), 6, "a start and an end event per file");
        assert!(events[0].outcome.is_none());
        report
    }

    #[tokio::test]
    async fn import_walks_folders_and_reports_per_file() {
        let (dir, state) = temp_state();
        let report = import_two(dir.path(), &state).await;
        assert_eq!(report.files.len(), 3);
        assert_eq!(report.imported, 2);
        assert_eq!(report.unmatched, 2, "no hash database in tests");
        assert_eq!(report.skipped, 1);
        assert_eq!(report.failed, 0);
        assert!(matches!(
            report.files[2].outcome,
            Some(ImportOutcome::Skipped { .. })
        ));
        let view = state.view().unwrap();
        assert_eq!(view.games.len(), 2);
        let names: Vec<&str> = view.games.iter().map(|g| g.game.name.as_str()).collect();
        assert!(names.contains(&"Alpha Game") && names.contains(&"Beta"));
        let first = &view.games[0];
        // Compressed by default: the size is the archive's plus the art, not the ROM's.
        assert!(first.game.compress);
        let archive = state.library().archive_path(&first.game);
        let archive_size = std::fs::metadata(&archive).unwrap().len();
        assert!(archive_size > 0 && first.size > archive_size);
        assert!(
            first.size < 16 * 1024,
            "a zero-filled 16 KiB ROM packs small"
        );
        assert!(
            first
                .cover
                .contains(&format!("/{}/{}.png?v=", first.game.code, first.game.code))
        );
        assert_eq!(first.system_name, "NES");
        assert_eq!(
            view.total_size,
            view.games.iter().map(|g| g.size).sum::<u64>()
        );

        // A broken file fails on its own; the others still import.
        let bad = dir.path().join("bad.sfc");
        std::fs::write(&bad, b"nope").unwrap();
        let report = state.import(&[bad], None, |_| {}).await.unwrap();
        assert_eq!(report.failed, 1);
        assert!(matches!(
            &report.files[0].outcome,
            Some(ImportOutcome::Failed { error }) if !error.is_empty()
        ));
        assert!(state.import(&[], None, |_| {}).await.is_err());
    }

    #[tokio::test]
    async fn edits_are_checked_and_saved() {
        let (dir, state) = temp_state();
        let report = import_two(dir.path(), &state).await;
        let code = report.codes[0].clone();
        let mut edit = GameEdit {
            code: code.clone(),
            name: " Alpha ".into(),
            sort_name: String::new(),
            publisher: "".into(),
            release_date: "1991-07-04".into(),
            players: 2,
            simultaneous: true,
            copyright: "".into(),
            description: "A game.".into(),
            application: "/bin/clover-kachikachi-wr".into(),
            args: "--guest-overscan-dimensions 0,0,9,3".into(),
            compress: None,
        };
        let view = state.update(&edit).unwrap();
        assert_eq!(view.game.name, "Alpha");
        assert_eq!(view.game.publisher, "UNKNOWN");
        assert_eq!(view.game.release_date, "1991-07-04");
        assert!(view.game.simultaneous);
        assert_eq!(state.library().game(&code).unwrap().description, "A game.");
        edit.release_date = "July 1991".into();
        assert!(
            state
                .update(&edit)
                .unwrap_err()
                .message
                .contains("YYYY-MM-DD")
        );
        edit.release_date = String::new();
        edit.players = 3;
        assert!(state.update(&edit).unwrap_err().message.contains("1 or 2"));
        edit.players = 1;
        edit.name = "  ".into();
        assert!(state.update(&edit).is_err());
        edit.name = "Alpha".into();
        edit.application = String::new();
        assert!(state.update(&edit).is_err());
        edit.application = "/bin/x".into();
        let view = state.update(&edit).unwrap();
        assert_eq!(view.game.release_date, "1900-01-01");
        assert!(!view.game.simultaneous, "one player is never simultaneous");
    }

    #[tokio::test]
    async fn menu_is_automatic_until_edited_then_follows_the_library() {
        let (dir, state) = temp_state();
        let report = import_two(dir.path(), &state).await;
        let status = no_console();
        let view = state.menu_view(&status).unwrap();
        assert!(!view.custom);
        assert_eq!(view.entries.len(), 2, "two games fit in one menu");
        assert!(view.system_code.is_none());

        state
            .apply_menu_edit(
                &status,
                &MenuEdit::AddFolder {
                    parent: vec![],
                    name: "Favourites".into(),
                },
            )
            .unwrap();
        let view = state.menu_view(&status).unwrap();
        assert!(view.custom);
        assert_eq!(view.entries.len(), 3);
        let folder = vec![2];
        state
            .move_items(
                &status,
                &[MenuItem::Game {
                    code: report.codes[0].clone(),
                }],
                &folder,
                None,
            )
            .unwrap();
        let view = state.menu_view(&status).unwrap();
        assert_eq!(view.entries.len(), 2);
        let Entry::Folder(favourites) = &view.entries[1] else {
            panic!("folder expected");
        };
        assert_eq!(favourites.entries.len(), 2, "the game and the back link");

        // A new game lands in the chosen folder; a removed game leaves the tree.
        let c = dir.path().join("Gamma.nes");
        std::fs::write(&c, nes_rom(3, false)).unwrap();
        let report2 = state.import(&[c], Some(&[1]), |_| {}).await.unwrap();
        let view = state.menu_view(&status).unwrap();
        let Entry::Folder(favourites) = &view.entries[1] else {
            panic!("folder expected");
        };
        assert_eq!(favourites.entries.len(), 3);
        assert_eq!(state.remove(&[report2.codes[0].clone()]).unwrap(), 1);
        let view = state.menu_view(&status).unwrap();
        let Entry::Folder(favourites) = &view.entries[1] else {
            panic!("folder expected");
        };
        assert_eq!(favourites.entries.len(), 2);

        state
            .apply_menu_edit(
                &status,
                &MenuEdit::RenameFolder {
                    path: vec![1],
                    name: "Best".into(),
                },
            )
            .unwrap();
        state
            .apply_menu_edit(&status, &MenuEdit::RemoveFolder { path: vec![1] })
            .unwrap();
        let view = state.menu_view(&status).unwrap();
        assert!(view.custom);
        assert_eq!(view.entries.len(), 2);

        // Applying a structure drops the edits.
        state
            .set_structure(MenuOptions {
                structure: Structure::Letters,
                originals_at_root: true,
            })
            .unwrap();
        let view = state.menu_view(&status).unwrap();
        assert!(!view.custom);
        assert_eq!(view.options.structure, Structure::Letters);
        assert!(matches!(view.entries[0], Entry::Folder(_)));
        assert!(
            state
                .set_structure(MenuOptions {
                    structure: Structure::Count { max: 0 },
                    originals_at_root: true
                })
                .is_err()
        );
    }

    #[test]
    fn busy_slot_is_exclusive_and_released() {
        let (_dir, state) = temp_state();
        let guard = state.begin("import").unwrap();
        assert_eq!(state.busy_with(), Some("import"));
        let error = state.begin("sync").unwrap_err();
        assert_eq!(error.message, "Wait for the import to finish first.");
        drop(guard);
        assert!(state.busy_with().is_none());
        let guard = state.begin("art").unwrap();
        assert!(
            state
                .begin("import")
                .unwrap_err()
                .message
                .contains("art fetch")
        );
        drop(guard);
    }

    #[tokio::test]
    async fn cover_protocol_serves_only_game_art() {
        let (dir, state) = temp_state();
        let report = import_two(dir.path(), &state).await;
        let code = &report.codes[0];
        let ok = state.cover_response(&format!("/{code}/{code}.png?v=12"));
        assert_eq!(ok.status(), http::StatusCode::OK);
        assert_eq!(ok.headers()[http::header::CONTENT_TYPE], "image/png");
        assert!(ok.body().starts_with(b"\x89PNG"));
        let small = state.cover_response(&format!("/{code}/{code}_small.png"));
        assert_eq!(small.status(), http::StatusCode::OK);
        for bad in [
            format!("/{code}/game.json"),
            format!("/{code}/../library.json"),
            "/CLV-U-ZZZZZ/CLV-U-ZZZZZ.png".to_string(),
            format!("/{code}"),
            format!("/{code}/{code}.png/extra"),
            "/nope/nope.png".to_string(),
        ] {
            assert_eq!(
                state.cover_response(&bad).status(),
                http::StatusCode::NOT_FOUND,
                "{bad}"
            );
        }
        let url = cover_url("CLV-U-AAAAA", "CLV-U-AAAAA.png", 7);
        assert!(url.ends_with("/CLV-U-AAAAA/CLV-U-AAAAA.png?v=7"));
        assert!(url.contains("cover"));

        // The firmware's art of a stock game, cached per console under `originals/<system>/`.
        state
            .library
            .store_originals(
                "snes-eur",
                &[(
                    "CLV-P-SAAAE".to_string(),
                    "[Desktop Entry]\nName=Super Mario World\n".to_string(),
                )],
            )
            .unwrap();
        state
            .library
            .store_original_art(
                "snes-eur",
                &[("CLV-P-SAAAE.png".to_string(), ok.body().clone())],
            )
            .unwrap();
        let stock = state.cover_response("/snes-eur/CLV-P-SAAAE/CLV-P-SAAAE.png?v=3");
        assert_eq!(stock.status(), http::StatusCode::OK);
        assert!(stock.body().starts_with(b"\x89PNG"));
        for bad in [
            "/snes-eur/CLV-P-SAAAE/CLV-P-SAAAE_small.png",
            "/snes-usa/CLV-P-SAAAE/CLV-P-SAAAE.png",
            "/snes-eur/CLV-P-SAABE/CLV-P-SAAAE.png",
            "/snes-eur/CLV-P-SAAAE/CLV-P-SAAAE.desktop",
            "/snes-eur/CLV-P-SAAAE/CLV-P-SAAAE.png/extra",
            "/snes-eur/CLV-P-SAAAE/",
            "/CLV-P-SAAAE/CLV-P-SAAAE.png",
            "/originals/snes-eur/CLV-P-SAAAE.png",
            "/../originals/snes-eur/CLV-P-SAAAE.png",
            "/snes-eur/../snes-eur/CLV-P-SAAAE.png",
        ] {
            assert_eq!(
                state.cover_response(bad).status(),
                http::StatusCode::NOT_FOUND,
                "{bad}"
            );
        }
        let url = original_art_url("snes-eur", "CLV-P-SAAAE", "CLV-P-SAAAE.png", 3);
        assert!(url.ends_with("/snes-eur/CLV-P-SAAAE/CLV-P-SAAAE.png?v=3"));
        let mut config = state.library.config().unwrap();
        config.last_console = Some("snes-eur".into());
        state.library.save_config(&config).unwrap();
        let menu = state.menu_view(&no_console()).unwrap();
        let original = menu
            .originals
            .iter()
            .find(|o| o.code == "CLV-P-SAAAE")
            .unwrap();
        assert!(
            original
                .cover
                .as_deref()
                .is_some_and(|u| u.contains("/snes-eur/CLV-P-SAAAE/CLV-P-SAAAE.png?v="))
        );
        assert!(original.thumbnail.is_none());
    }

    #[test]
    fn key_is_stored_and_cleared() {
        let (_dir, state) = temp_state();
        let status = state.key_status().unwrap();
        assert!(!status.stored);
        let status = state.set_key(Some(" abc ")).unwrap();
        assert!(status.stored);
        assert_eq!(
            state.library().config().unwrap().tgdb_key.as_deref(),
            Some("abc")
        );
        let status = state.set_key(Some("")).unwrap();
        assert!(!status.stored);
        assert!(state.set_key(None).is_ok());
    }

    #[test]
    fn payload_shapes() {
        let edit: MenuEdit =
            serde_json::from_value(json!({"op": "add_folder", "parent": [], "name": "X"})).unwrap();
        assert_eq!(
            edit,
            MenuEdit::AddFolder {
                parent: vec![],
                name: "X".into()
            }
        );
        let edit: MenuEdit =
            serde_json::from_value(json!({"op": "remove_folder", "path": [1, 2]})).unwrap();
        assert_eq!(edit, MenuEdit::RemoveFolder { path: vec![1, 2] });
        let item: MenuItem =
            serde_json::from_value(json!({"kind": "game", "code": "CLV-U-AAAAA"})).unwrap();
        assert_eq!(
            item,
            MenuItem::Game {
                code: "CLV-U-AAAAA".into()
            }
        );
        let event = ImportEvent {
            index: 0,
            total: 1,
            path: "x".into(),
            outcome: Some(ImportOutcome::Skipped { reason: "r".into() }),
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap()["outcome"]["state"],
            "skipped"
        );
        let edit: GameEdit =
            serde_json::from_value(json!({"code": "CLV-H-AAAAA", "name": "N"})).unwrap();
        assert_eq!(edit.players, 1);
    }

    #[test]
    fn expand_keeps_given_files_and_walks_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("b/.hidden")).unwrap();
        std::fs::write(root.join("b/two.NES"), b"").unwrap();
        std::fs::write(root.join("b/.hidden/three.nes"), b"").unwrap();
        std::fs::write(root.join("b/readme.txt"), b"").unwrap();
        std::fs::write(root.join("a.zip"), b"").unwrap();
        std::fs::write(root.join("c.txt"), b"").unwrap();
        let files = expand_paths(&[root.join("b"), root.join("a.zip"), root.join("c.txt")]);
        assert_eq!(
            files,
            vec![
                root.join("b/two.NES"),
                root.join("a.zip"),
                root.join("c.txt")
            ]
        );
    }
}
