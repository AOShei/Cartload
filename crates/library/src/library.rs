//! The library on disk: one directory per game with `game.json`, the ROM in the console's
//! container, the launch entry and the art; `library.json` with the menu options; a cache of
//! the firmware's launch entries per console.

use std::{
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::{
    Error, Result, archive, codes,
    desktop::{DesktopEntry, Format},
    error::io_error,
    game::{GAMES_PATH, Game},
    hashdb::HashDb,
    images,
    menu::MenuOptions,
    rom::{self, Prepared, System},
};

/// Name of the per-game metadata file.
pub const GAME_JSON: &str = "game.json";
/// Name of the library settings file.
pub const LIBRARY_JSON: &str = "library.json";
/// Directory holding the cached firmware launch entries, per system code.
pub const ORIGINALS_DIR: &str = "originals";

/// Settings stored in `library.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryConfig {
    /// How the menu is built.
    #[serde(default)]
    pub menu: MenuOptions,
    /// System code of the console last synced or planned against (`snes-eur`).
    #[serde(default)]
    pub last_console: Option<String>,
    /// TheGamesDB API key, when stored here rather than in the environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tgdb_key: Option<String>,
    /// A folder tree edited by hand. When set it is used instead of the automatic structure
    /// in `menu`, brought in line with the library's games on every read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_menu: Option<crate::menu::Menu>,
}

/// Choices for an import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportOptions {
    /// Pack the ROM as a 7z archive for the console (hakchi2 CE's "Compress games"). Applies
    /// to SNES and NES games up to [`archive::MAX_SIZE`]; larger ROMs stay uncompressed with
    /// a warning.
    pub compress: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self { compress: true }
    }
}

/// What turning compression on or off did to a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressReport {
    /// Whether the game is compressed now.
    pub compressed: bool,
    /// Whether anything changed (the archive was built or removed, the setting flipped).
    pub changed: bool,
    /// Size of the ROM file.
    pub rom_size: u64,
    /// Size of the archive, when the game is compressed.
    pub archive_size: Option<u64>,
}

/// The result of importing one ROM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imported {
    /// The game as saved.
    pub game: Game,
    /// Whether the hash database knew the ROM.
    pub matched: bool,
    /// Whether a game with the same code existed and was replaced.
    pub replaced: bool,
    /// Things the user should know.
    pub warnings: Vec<String>,
}

/// A file of a game as it will be uploaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameFile {
    /// Name inside the game directory.
    pub name: String,
    /// Where it is.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
}

/// The games library.
#[derive(Clone, Debug)]
pub struct Library {
    root: PathBuf,
}

impl Library {
    /// Opens (creating if needed) the library at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| io_error("create directory", &root, e))?;
        Ok(Self { root })
    }

    /// The default location: `<user data dir>/cartload/library`.
    pub fn default_root() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join("cartload").join("library"))
    }

    /// Where the library lives.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory of a game.
    pub fn game_dir(&self, code: &str) -> PathBuf {
        self.root.join(code)
    }

    /// The settings.
    pub fn config(&self) -> Result<LibraryConfig> {
        let path = self.root.join(LIBRARY_JSON);
        if !path.exists() {
            return Ok(LibraryConfig::default());
        }
        read_json(&path)
    }

    /// Saves the settings.
    pub fn save_config(&self, config: &LibraryConfig) -> Result<()> {
        write_json(&self.root.join(LIBRARY_JSON), config)
    }

    /// Every game, sorted by code. Directories without a readable `game.json` are skipped
    /// with a warning.
    pub fn games(&self) -> Result<Vec<Game>> {
        let entries = std::fs::read_dir(&self.root).map_err(|e| io_error("read", &self.root, e))?;
        let mut games = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| io_error("read", &self.root, e))?;
            let name = entry.file_name();
            let Some(code) = name.to_str() else { continue };
            if !codes::is_code(code) || !entry.path().is_dir() {
                continue;
            }
            match self.game(code) {
                Ok(game) => games.push(game),
                Err(error) => warn!(code, %error, "skipping unreadable game directory"),
            }
        }
        games.sort_by(|a, b| a.code.cmp(&b.code));
        Ok(games)
    }

    /// One game by code.
    pub fn game(&self, code: &str) -> Result<Game> {
        if !codes::is_code(code) {
            return Err(Error::InvalidCode(code.to_string()));
        }
        let path = self.game_dir(code).join(GAME_JSON);
        if !path.exists() {
            return Err(Error::NoSuchGame(code.to_string()));
        }
        read_json(&path)
    }

    /// Writes `game.json` and the launch entry of a game whose directory exists.
    pub fn save(&self, game: &Game) -> Result<()> {
        let dir = self.game_dir(&game.code);
        if !dir.is_dir() {
            return Err(Error::NoSuchGame(game.code.clone()));
        }
        write_json(&dir.join(GAME_JSON), game)?;
        let desktop = dir.join(game.desktop_file());
        std::fs::write(
            &desktop,
            game.desktop_entry(GAMES_PATH).render(Format::Hakchi),
        )
        .map_err(|e| io_error("write", &desktop, e))?;
        Ok(())
    }

    /// Removes a game and its files.
    pub fn remove(&self, code: &str) -> Result<()> {
        if !codes::is_code(code) {
            return Err(Error::InvalidCode(code.to_string()));
        }
        let dir = self.game_dir(code);
        if !dir.is_dir() {
            return Err(Error::NoSuchGame(code.to_string()));
        }
        std::fs::remove_dir_all(&dir).map_err(|e| io_error("remove", &dir, e))?;
        info!(code, "removed game");
        Ok(())
    }

    /// Imports a ROM file (`.sfc`, `.smc`, `.fig`, `.swc`, `.nes`) or a zip of those with the
    /// default options (compressed). A zip gives one game per ROM inside it. The hash
    /// database, when given, supplies the title and TheGamesDB ids.
    pub fn import(&self, path: &Path, db: Option<&HashDb>) -> Result<Vec<Imported>> {
        self.import_with(path, db, &ImportOptions::default())
    }

    /// [`Library::import`] with explicit options.
    pub fn import_with(
        &self,
        path: &Path,
        db: Option<&HashDb>,
        options: &ImportOptions,
    ) -> Result<Vec<Imported>> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::UnsupportedFile(path.to_path_buf()))?;
        if file_name.to_ascii_lowercase().ends_with(".zip") {
            return self.import_zip(path, db, options);
        }
        if System::from_path(path).is_none() {
            return Err(Error::UnsupportedFile(path.to_path_buf()));
        }
        let data = std::fs::read(path).map_err(|e| io_error("read", path, e))?;
        Ok(vec![self.import_bytes_with(file_name, data, db, options)?])
    }

    fn import_zip(
        &self,
        path: &Path,
        db: Option<&HashDb>,
        options: &ImportOptions,
    ) -> Result<Vec<Imported>> {
        let file = std::fs::File::open(path).map_err(|e| io_error("open", path, e))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|source| Error::Zip {
            path: path.to_path_buf(),
            source,
        })?;
        let mut imported = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|source| Error::Zip {
                path: path.to_path_buf(),
                source,
            })?;
            if !entry.is_file() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|source| Error::Zip {
                    path: path.to_path_buf(),
                    source,
                })?
                .rsplit('/')
                .next()
                .unwrap_or("")
                .to_string();
            if System::from_path(Path::new(&name)).is_none() {
                debug!(name, "skipping archive entry");
                continue;
            }
            let mut data = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
            entry
                .read_to_end(&mut data)
                .map_err(|e| io_error("read", path, e))?;
            imported.push(self.import_bytes_with(&name, data, db, options)?);
        }
        if imported.is_empty() {
            return Err(Error::EmptyArchive(path.to_path_buf()));
        }
        Ok(imported)
    }

    /// Imports a ROM given its file name and content, with the default options.
    pub fn import_bytes(
        &self,
        file_name: &str,
        data: Vec<u8>,
        db: Option<&HashDb>,
    ) -> Result<Imported> {
        self.import_bytes_with(file_name, data, db, &ImportOptions::default())
    }

    /// [`Library::import_bytes`] with explicit options.
    pub fn import_bytes_with(
        &self,
        file_name: &str,
        data: Vec<u8>,
        db: Option<&HashDb>,
        options: &ImportOptions,
    ) -> Result<Imported> {
        let mut prepared = rom::prepare(file_name, data)?;
        let compress = options.compress && prepared.system != System::MegaDrive;
        let compress = if compress && !archive::can_compress(prepared.data.len() as u64) {
            prepared.warnings.push(
                Error::TooLargeToCompress {
                    name: prepared.file_name.clone(),
                    size: prepared.data.len() as u64,
                }
                .to_string(),
            );
            false
        } else {
            compress
        };
        let code = prepared.code();
        let record = db.and_then(|db| {
            db.lookup(prepared.crc32)
                .or_else(|| db.lookup(prepared.file_crc32))
        });
        let name = match record {
            Some(record) => record.title(),
            None => rom::title_from_file_name(file_name),
        };
        let name = if name.is_empty() { code.clone() } else { name };
        let game = Game {
            code: code.clone(),
            name,
            sort_name: String::new(),
            system: prepared.system,
            rom_file: prepared.file_name.clone(),
            crc32: prepared.crc32,
            file_crc32: prepared.file_crc32,
            original_file: file_name.to_string(),
            players: 1,
            simultaneous: false,
            release_date: "1900-01-01".to_string(),
            publisher: "UNKNOWN".to_string(),
            copyright: String::new(),
            save_count: prepared.save_count,
            application: prepared.system.application().to_string(),
            args: prepared.system.default_args().to_string(),
            tgdb_ids: record.map(|r| r.tgdb_ids.clone()).unwrap_or_default(),
            tgdb_id: None,
            description: String::new(),
            db_name: record.map(|r| r.name.clone()),
            snes: prepared.snes.clone(),
            nes: prepared.nes.clone(),
            placeholder_art: true,
            compress,
        };
        let replaced = self.write_game_dir(&game, &prepared)?;
        self.save(&game)?;
        info!(
            code,
            name = game.name,
            system = %game.system,
            matched = record.is_some(),
            replaced,
            compressed = game.compress,
            "imported game"
        );
        Ok(Imported {
            game,
            matched: record.is_some(),
            replaced,
            warnings: prepared.warnings,
        })
    }

    /// Creates (or empties and recreates, like hakchi2 CE) the game directory with the ROM and
    /// placeholder art. Returns whether a directory existed.
    fn write_game_dir(&self, game: &Game, prepared: &Prepared) -> Result<bool> {
        let dir = self.game_dir(&game.code);
        let replaced = dir.exists();
        if replaced {
            std::fs::remove_dir_all(&dir).map_err(|e| io_error("remove", &dir, e))?;
        }
        std::fs::create_dir_all(&dir).map_err(|e| io_error("create directory", &dir, e))?;
        let rom_path = dir.join(&prepared.file_name);
        std::fs::write(&rom_path, &prepared.data).map_err(|e| io_error("write", &rom_path, e))?;
        if game.compress {
            let archive_path = dir.join(game.archive_file());
            let packed = archive::pack(&prepared.file_name, &prepared.data)?;
            std::fs::write(&archive_path, packed)
                .map_err(|e| io_error("write", &archive_path, e))?;
        }
        let cover = dir.join(game.cover_file());
        std::fs::write(&cover, images::cover_placeholder(game.system)?)
            .map_err(|e| io_error("write", &cover, e))?;
        let thumbnail = dir.join(game.thumbnail_file());
        std::fs::write(&thumbnail, images::thumbnail_placeholder(game.system)?)
            .map_err(|e| io_error("write", &thumbnail, e))?;
        Ok(replaced)
    }

    /// Replaces the cover and the thumbnail of a game with ones made from `image` (PNG, JPEG,
    /// GIF or BMP), marks the art as real and saves the game.
    pub fn set_cover(
        &self,
        game: &mut Game,
        image: &[u8],
        options: &images::CoverOptions,
    ) -> Result<images::CoverSet> {
        let dir = self.game_dir(&game.code);
        if !dir.is_dir() {
            return Err(Error::NoSuchGame(game.code.clone()));
        }
        let set = images::make_cover(image, options)?;
        let cover = dir.join(game.cover_file());
        std::fs::write(&cover, &set.cover).map_err(|e| io_error("write", &cover, e))?;
        let thumbnail = dir.join(game.thumbnail_file());
        std::fs::write(&thumbnail, &set.thumbnail).map_err(|e| io_error("write", &thumbnail, e))?;
        game.placeholder_art = false;
        self.save(game)?;
        Ok(set)
    }

    /// Restores the placeholder cover and thumbnail of a game and saves it.
    pub fn clear_cover(&self, game: &mut Game) -> Result<()> {
        let dir = self.game_dir(&game.code);
        if !dir.is_dir() {
            return Err(Error::NoSuchGame(game.code.clone()));
        }
        let cover = dir.join(game.cover_file());
        std::fs::write(&cover, images::cover_placeholder(game.system)?)
            .map_err(|e| io_error("write", &cover, e))?;
        let thumbnail = dir.join(game.thumbnail_file());
        std::fs::write(&thumbnail, images::thumbnail_placeholder(game.system)?)
            .map_err(|e| io_error("write", &thumbnail, e))?;
        game.placeholder_art = true;
        self.save(game)
    }

    /// The art index and cache of this library (`<root>/art`).
    pub fn art_store(&self) -> Result<crate::art::ArtStore> {
        crate::art::ArtStore::open(self.root.join(crate::art::ART_DIR))
    }

    /// Path of the ROM file of a game.
    pub fn rom_path(&self, game: &Game) -> PathBuf {
        self.game_dir(&game.code).join(&game.rom_file)
    }

    /// Path of the packed ROM of a game (whether or not it exists).
    pub fn archive_path(&self, game: &Game) -> PathBuf {
        self.game_dir(&game.code).join(game.archive_file())
    }

    /// Makes sure the archive of a compressed game exists and is newer than its ROM, packing
    /// the ROM when it is missing or stale. Returns whether it was (re)built. Does nothing for
    /// an uncompressed game.
    pub fn ensure_archive(&self, game: &Game) -> Result<bool> {
        if !game.compress {
            return Ok(false);
        }
        let rom_path = self.rom_path(game);
        let archive_path = self.archive_path(game);
        let rom_meta = std::fs::metadata(&rom_path).map_err(|e| io_error("stat", &rom_path, e))?;
        if let Ok(archive_meta) = std::fs::metadata(&archive_path)
            && archive_meta.is_file()
            && archive_meta.len() > 0
            && let (Ok(rom_time), Ok(archive_time)) = (rom_meta.modified(), archive_meta.modified())
            && archive_time >= rom_time
        {
            return Ok(false);
        }
        let data = std::fs::read(&rom_path).map_err(|e| io_error("read", &rom_path, e))?;
        let packed = archive::pack(&game.rom_file, &data)?;
        let temp = archive_path.with_extension("7z.tmp");
        std::fs::write(&temp, &packed).map_err(|e| io_error("write", &temp, e))?;
        std::fs::rename(&temp, &archive_path).map_err(|e| io_error("rename", &temp, e))?;
        info!(
            code = game.code,
            rom = game.rom_file,
            rom_size = data.len(),
            archive_size = packed.len(),
            "packed the ROM"
        );
        Ok(true)
    }

    /// Turns compression on or off for a game: builds the archive (or removes it), updates
    /// `game.json` and the launch entry. A ROM above [`archive::MAX_SIZE`] cannot be
    /// compressed and the error says so.
    pub fn set_compress(&self, game: &mut Game, compress: bool) -> Result<CompressReport> {
        if !self.game_dir(&game.code).is_dir() {
            return Err(Error::NoSuchGame(game.code.clone()));
        }
        let rom_path = self.rom_path(game);
        let rom_size = std::fs::metadata(&rom_path)
            .map_err(|e| io_error("stat", &rom_path, e))?
            .len();
        let changed = game.compress != compress;
        game.compress = compress;
        let archive_path = self.archive_path(game);
        let archive_size = if compress {
            let built = self.ensure_archive(game)?;
            let size = std::fs::metadata(&archive_path)
                .map_err(|e| io_error("stat", &archive_path, e))?
                .len();
            if built || changed {
                self.save(game)?;
            }
            Some(size)
        } else {
            if archive_path.exists() {
                std::fs::remove_file(&archive_path)
                    .map_err(|e| io_error("remove", &archive_path, e))?;
            }
            if changed {
                self.save(game)?;
            }
            None
        };
        Ok(CompressReport {
            compressed: compress,
            changed,
            rom_size,
            archive_size,
        })
    }

    /// The files of a game that go to the console's storage: everything in the directory
    /// except `game.json`, the launch entry (which is generated per menu) and, depending on
    /// the compression setting, the raw ROM or its archive. The archive of a compressed game
    /// is built here when it is missing (a library from before compression existed).
    pub fn game_files(&self, game: &Game) -> Result<Vec<GameFile>> {
        self.ensure_archive(game)?;
        let dir = self.game_dir(&game.code);
        let entries = std::fs::read_dir(&dir).map_err(|e| io_error("read", &dir, e))?;
        let archive_file = game.archive_file();
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| io_error("read", &dir, e))?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name == GAME_JSON || name == game.desktop_file() || !path.is_file() {
                continue;
            }
            let skipped = if game.compress {
                name == game.rom_file
            } else {
                name == archive_file
            };
            if skipped || name.ends_with(".7z.tmp") {
                continue;
            }
            let metadata = entry.metadata().map_err(|e| io_error("stat", &path, e))?;
            files.push(GameFile {
                name: name.to_string(),
                size: metadata.len(),
                mtime: unix_seconds(
                    metadata
                        .modified()
                        .map_err(|e| io_error("stat", &path, e))?,
                ),
                path,
            });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    /// Modification time of a game's metadata, used as the launch entry's timestamp.
    pub fn game_mtime(&self, code: &str) -> Result<i64> {
        let path = self.game_dir(code).join(GAME_JSON);
        let metadata = std::fs::metadata(&path).map_err(|e| io_error("stat", &path, e))?;
        Ok(unix_seconds(
            metadata
                .modified()
                .map_err(|e| io_error("stat", &path, e))?,
        ))
    }

    /// Stores the firmware's launch entries of a console (`CLV-P-...` code, file content).
    pub fn store_originals(&self, system_code: &str, entries: &[(String, String)]) -> Result<()> {
        let dir = self.root.join(ORIGINALS_DIR).join(system_code);
        std::fs::create_dir_all(&dir).map_err(|e| io_error("create directory", &dir, e))?;
        for (code, text) in entries {
            if !codes::is_original(code) {
                return Err(Error::InvalidCode(code.clone()));
            }
            let path = dir.join(format!("{code}.desktop"));
            std::fs::write(&path, text).map_err(|e| io_error("write", &path, e))?;
        }
        Ok(())
    }

    /// The cached firmware launch entries of a console, sorted by code; empty when none were
    /// stored.
    pub fn originals(&self, system_code: &str) -> Result<Vec<(String, DesktopEntry, i64)>> {
        let dir = self.root.join(ORIGINALS_DIR).join(system_code);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut originals = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(|e| io_error("read", &dir, e))? {
            let entry = entry.map_err(|e| io_error("read", &dir, e))?;
            let path = entry.path();
            let Some(code) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".desktop"))
            else {
                continue;
            };
            if !codes::is_original(code) {
                continue;
            }
            let text = std::fs::read_to_string(&path).map_err(|e| io_error("read", &path, e))?;
            let metadata = std::fs::metadata(&path).map_err(|e| io_error("stat", &path, e))?;
            let mtime = unix_seconds(
                metadata
                    .modified()
                    .map_err(|e| io_error("stat", &path, e))?,
            );
            originals.push((code.to_string(), DesktopEntry::parse(&text), mtime));
        }
        originals.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(originals)
    }

    /// The firmware art files (`<code>.png`, `<code>_small.png`) of the cached originals of a
    /// console that are not cached yet, sorted by code.
    pub fn missing_original_art(&self, system_code: &str) -> Result<Vec<String>> {
        let dir = self.root.join(ORIGINALS_DIR).join(system_code);
        let mut missing = Vec::new();
        for (code, _, _) in self.originals(system_code)? {
            for suffix in codes::ORIGINAL_ART_SUFFIXES {
                let file = format!("{code}{suffix}");
                if !dir.join(&file).is_file() {
                    missing.push(file);
                }
            }
        }
        Ok(missing)
    }

    /// Stores firmware art files read from a console: (`<code>.png` or `<code>_small.png`,
    /// bytes) pairs. Empty files are skipped (the console had none); the count of files
    /// written is returned.
    pub fn store_original_art(
        &self,
        system_code: &str,
        files: &[(String, Vec<u8>)],
    ) -> Result<usize> {
        if !codes::is_system_code(system_code) {
            return Err(Error::InvalidCode(system_code.to_string()));
        }
        let dir = self.root.join(ORIGINALS_DIR).join(system_code);
        std::fs::create_dir_all(&dir).map_err(|e| io_error("create directory", &dir, e))?;
        let mut stored = 0;
        for (file, bytes) in files {
            if codes::original_art_code(file).is_none() {
                return Err(Error::InvalidCode(file.clone()));
            }
            if bytes.is_empty() {
                continue;
            }
            let path = dir.join(file);
            std::fs::write(&path, bytes).map_err(|e| io_error("write", &path, e))?;
            stored += 1;
        }
        Ok(stored)
    }

    /// The cached firmware art file `file` (`<code>.png` or `<code>_small.png`) of a console,
    /// when the names are valid and the file is there.
    pub fn original_art(&self, system_code: &str, file: &str) -> Option<PathBuf> {
        if !codes::is_system_code(system_code) || codes::original_art_code(file).is_none() {
            return None;
        }
        let path = self.root.join(ORIGINALS_DIR).join(system_code).join(file);
        path.is_file().then_some(path)
    }
}

/// Seconds since the Unix epoch (0 for times before it).
pub fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Reads a JSON file.
pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).map_err(|e| io_error("read", path, e))?;
    serde_json::from_str(&text).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes a JSON file through a temporary file in the same directory, so a crash leaves the
/// old file intact.
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })?;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, text + "\n").map_err(|e| io_error("write", &temp, e))?;
    std::fs::rename(&temp, path).map_err(|e| io_error("rename", &temp, e))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::rom::tests::{synthetic_nes, synthetic_snes};

    fn sample_db() -> HashDb {
        let rom = synthetic_snes("CHRONO TRIGGER", rom::snes::RomType::HiRom, 0x02, 0x03);
        let crc = rom::crc32(&rom);
        HashDb::parse(&format!(
            r#"<romfiles><file name="Chrono Trigger (USA).sfc" crc32="{crc:08X}"><tgdb>1255</tgdb></file></romfiles>"#
        ))
        .unwrap()
    }

    #[test]
    fn import_creates_the_game_directory_with_everything_the_console_needs() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("lib")).unwrap();
        let rom = synthetic_snes("CHRONO TRIGGER", rom::snes::RomType::HiRom, 0x02, 0x03);
        let mut headered = vec![0u8; 512];
        headered.extend_from_slice(&rom);
        let source = dir.path().join("Chrono Trigger (USA).smc");
        std::fs::write(&source, &headered).unwrap();

        let imported = library.import(&source, Some(&sample_db())).unwrap();
        assert_eq!(imported.len(), 1);
        let imported = &imported[0];
        assert!(imported.matched);
        assert!(!imported.replaced);
        let game = &imported.game;
        assert_eq!(game.name, "Chrono Trigger");
        assert_eq!(game.code, codes::game_code(rom::crc32(&rom), 'U'));
        assert_eq!(game.rom_file, "Chrono_Trigger.sfrom");
        assert_eq!(game.tgdb_ids, [1255]);
        assert_eq!(game.save_count, 3);
        assert!(game.compress, "SNES games are compressed by default");

        let game_dir = library.game_dir(&game.code);
        let mut names: Vec<String> = std::fs::read_dir(&game_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                format!("{}.desktop", game.code),
                format!("{}.png", game.code),
                format!("{}_small.png", game.code),
                "Chrono_Trigger.sfrom".to_string(),
                "Chrono_Trigger.sfrom.7z".to_string(),
                "game.json".to_string()
            ]
        );
        let sfrom = std::fs::read(game_dir.join("Chrono_Trigger.sfrom")).unwrap();
        assert_eq!(sfrom.len(), rom.len() + 96);
        let packed = std::fs::read(game_dir.join("Chrono_Trigger.sfrom.7z")).unwrap();
        assert_eq!(archive::unpack(&packed).unwrap().1, sfrom);
        let desktop = std::fs::read_to_string(game_dir.join(game.desktop_file())).unwrap();
        assert!(desktop.contains(&format!(
            "Exec=/bin/clover-canoe-shvc-wr -rom /var/games/{}/Chrono_Trigger.sfrom.7z --volume 100 -rollback-snapshot-period 600\n",
            game.code
        )));
        assert!(desktop.contains("SortRawTitle=chrono trigger\n"));

        assert_eq!(library.games().unwrap(), vec![game.clone()]);
        assert_eq!(library.game(&game.code).unwrap(), *game);
        let files = library.game_files(game).unwrap();
        let file_names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            file_names,
            [
                format!("{}.png", game.code),
                format!("{}_small.png", game.code),
                "Chrono_Trigger.sfrom.7z".to_string()
            ],
            "the archive goes to the console, not the ROM"
        );
        assert!(library.game_mtime(&game.code).unwrap() > 0);

        // Importing again replaces the directory.
        let again = library.import(&source, None).unwrap();
        assert!(again[0].replaced);
        assert!(!again[0].matched);
        assert_eq!(
            again[0].game.name, "Chrono Trigger",
            "falls back to the file name"
        );

        // Compression off: the archive goes, the ROM is what gets uploaded.
        let mut game = again[0].game.clone();
        let report = library.set_compress(&mut game, false).unwrap();
        assert!(report.changed && !report.compressed);
        assert_eq!(report.archive_size, None);
        assert_eq!(report.rom_size, sfrom.len() as u64);
        assert!(!game_dir.join("Chrono_Trigger.sfrom.7z").exists());
        assert!(!library.game(&game.code).unwrap().compress);
        let names: Vec<String> = library
            .game_files(&game)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert!(names.contains(&"Chrono_Trigger.sfrom".to_string()));
        assert!(!names.iter().any(|n| n.ends_with(".7z")));
        let desktop = std::fs::read_to_string(game_dir.join(game.desktop_file())).unwrap();
        assert!(desktop.contains("/Chrono_Trigger.sfrom --volume"));
        assert!(!library.set_compress(&mut game, false).unwrap().changed);

        // Back on: the archive is rebuilt and the entry names it again.
        let report = library.set_compress(&mut game, true).unwrap();
        assert!(report.changed && report.compressed);
        assert_eq!(
            report.archive_size,
            Some(
                std::fs::metadata(game_dir.join("Chrono_Trigger.sfrom.7z"))
                    .unwrap()
                    .len()
            )
        );
        assert!(library.game(&game.code).unwrap().compress);
        assert!(
            !library.ensure_archive(&game).unwrap(),
            "already up to date"
        );

        // A library from before compression: the archive is missing and game_files builds it.
        std::fs::remove_file(game_dir.join("Chrono_Trigger.sfrom.7z")).unwrap();
        let names: Vec<String> = library
            .game_files(&game)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert!(names.contains(&"Chrono_Trigger.sfrom.7z".to_string()));
        assert!(game_dir.join("Chrono_Trigger.sfrom.7z").exists());

        library.remove(&game.code).unwrap();
        assert!(library.games().unwrap().is_empty());
        assert!(matches!(
            library.game(&game.code),
            Err(Error::NoSuchGame(_))
        ));
        assert!(matches!(library.game("nope"), Err(Error::InvalidCode(_))));
    }

    #[test]
    fn import_zip_takes_every_rom_inside() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("lib")).unwrap();
        let zip_path = dir.path().join("games.zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("readme.txt", options).unwrap();
            writer.write_all(b"hello").unwrap();
            writer.start_file("sub/Game A.nes", options).unwrap();
            writer.write_all(&synthetic_nes(1, false)).unwrap();
            writer.start_file("Game B (Europe).sfc", options).unwrap();
            writer
                .write_all(&synthetic_snes("GAME B", rom::snes::RomType::LoRom, 0, 0))
                .unwrap();
            writer.finish().unwrap();
        }
        let options = ImportOptions { compress: false };
        let imported = library.import_with(&zip_path, None, &options).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].game.name, "Game A");
        assert_eq!(imported[0].game.system, System::Nes);
        assert_eq!(imported[1].game.name, "Game B");
        assert_eq!(imported[1].game.system, System::Snes);
        assert_eq!(library.games().unwrap().len(), 2);
        for item in &imported {
            assert!(!item.game.compress, "--no-compress import");
            assert!(!library.archive_path(&item.game).exists());
        }

        let empty = dir.path().join("empty.zip");
        {
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&empty).unwrap());
            writer
                .start_file("notes.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.finish().unwrap();
        }
        assert!(matches!(
            library.import(&empty, None),
            Err(Error::EmptyArchive(_))
        ));
        assert!(matches!(
            library.import(&dir.path().join("x.txt"), None),
            Err(Error::UnsupportedFile(_))
        ));
    }

    #[test]
    fn original_art_is_cached_per_file() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        assert!(library.missing_original_art("snes-eur").unwrap().is_empty());
        let stock = include_str!("../tests/fixtures/stock-snes-eur-CLV-P-SAAAE.desktop");
        library
            .store_originals(
                "snes-eur",
                &[
                    ("CLV-P-SAAAE".to_string(), stock.to_string()),
                    ("CLV-P-SAABE".to_string(), stock.to_string()),
                ],
            )
            .unwrap();
        assert_eq!(
            library.missing_original_art("snes-eur").unwrap(),
            [
                "CLV-P-SAAAE.png",
                "CLV-P-SAAAE_small.png",
                "CLV-P-SAABE.png",
                "CLV-P-SAABE_small.png"
            ]
        );
        let stored = library
            .store_original_art(
                "snes-eur",
                &[
                    ("CLV-P-SAAAE.png".to_string(), b"\x89PNG cover".to_vec()),
                    ("CLV-P-SAABE_small.png".to_string(), Vec::new()),
                ],
            )
            .unwrap();
        assert_eq!(stored, 1);
        assert_eq!(
            library.missing_original_art("snes-eur").unwrap(),
            [
                "CLV-P-SAAAE_small.png",
                "CLV-P-SAABE.png",
                "CLV-P-SAABE_small.png"
            ]
        );
        let path = library.original_art("snes-eur", "CLV-P-SAAAE.png").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"\x89PNG cover");
        assert!(
            library
                .original_art("snes-eur", "CLV-P-SAAAE_small.png")
                .is_none()
        );
        assert!(
            library
                .original_art("snes-usa", "CLV-P-SAAAE.png")
                .is_none()
        );
        assert!(
            library
                .original_art("../snes-eur", "CLV-P-SAAAE.png")
                .is_none()
        );
        assert!(
            library
                .original_art("snes-eur", "CLV-P-SAAAE.desktop")
                .is_none()
        );
        assert!(matches!(
            library.store_original_art("snes-eur", &[("game.json".to_string(), b"x".to_vec())]),
            Err(Error::InvalidCode(_))
        ));
        assert!(matches!(
            library.store_original_art("nope", &[("CLV-P-SAAAE.png".to_string(), b"x".to_vec())]),
            Err(Error::InvalidCode(_))
        ));
    }

    #[test]
    fn config_and_originals_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        assert_eq!(library.config().unwrap(), LibraryConfig::default());
        let config = LibraryConfig {
            menu: MenuOptions {
                structure: crate::menu::Structure::Letters,
                originals_at_root: false,
            },
            last_console: Some("snes-eur".into()),
            tgdb_key: None,
            custom_menu: None,
        };
        library.save_config(&config).unwrap();
        assert_eq!(library.config().unwrap(), config);

        assert!(library.originals("snes-eur").unwrap().is_empty());
        let stock = include_str!("../tests/fixtures/stock-snes-eur-CLV-P-SAAAE.desktop");
        library
            .store_originals(
                "snes-eur",
                &[("CLV-P-SAAAE".to_string(), stock.to_string())],
            )
            .unwrap();
        let originals = library.originals("snes-eur").unwrap();
        assert_eq!(originals.len(), 1);
        assert_eq!(originals[0].0, "CLV-P-SAAAE");
        assert_eq!(originals[0].1.name, "Super Mario World");
        assert!(matches!(
            library.store_originals("snes-eur", &[("CLV-U-AAAAA".to_string(), String::new())]),
            Err(Error::InvalidCode(_))
        ));
    }
}
