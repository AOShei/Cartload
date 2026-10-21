//! A game in the library: the metadata stored in `game.json` and the launch entry derived
//! from it.

use serde::{Deserialize, Serialize};

use crate::{
    archive,
    desktop::{DesktopEntry, fix_apostrophes, sort_title},
    rom::{NesInfo, SnesInfo, System},
};

/// Directory the console mounts the current menu's games at.
pub const GAMES_PATH: &str = "/var/games";
/// Directory of the save profiles on a hakchi console.
pub const PROFILE_PATH: &str = "/var/saves";

/// One game, as stored in `game.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    /// `CLV-U-ZJSQU`: the directory name and the console's identity for the game.
    pub code: String,
    /// Title shown in the menu.
    pub name: String,
    /// Sort key; derived from the name when empty.
    #[serde(default)]
    pub sort_name: String,
    /// Target system.
    pub system: System,
    /// ROM file name inside the game directory (`Chrono_Trigger.sfrom`).
    pub rom_file: String,
    /// CRC-32 the code was derived from (headerless SNES ROM, NES PRG+CHR).
    pub crc32: u32,
    /// CRC-32 of the imported file after copier header removal.
    #[serde(default)]
    pub file_crc32: u32,
    /// Name of the imported file.
    #[serde(default)]
    pub original_file: String,
    /// Players.
    #[serde(default = "one")]
    pub players: u8,
    /// Whether the players play at the same time.
    #[serde(default)]
    pub simultaneous: bool,
    /// Release date (`1995-03-11`).
    #[serde(default = "default_release_date")]
    pub release_date: String,
    /// Publisher.
    #[serde(default = "default_publisher")]
    pub publisher: String,
    /// Copyright line.
    #[serde(default)]
    pub copyright: String,
    /// Save slots (3 with battery RAM, else 0).
    #[serde(default)]
    pub save_count: u8,
    /// The emulator command, including `-rom` where the emulator takes it.
    pub application: String,
    /// Arguments after the ROM path.
    #[serde(default)]
    pub args: String,
    /// TheGamesDB ids from the hash database, best match first.
    #[serde(default)]
    pub tgdb_ids: Vec<u32>,
    /// TheGamesDB id the art and metadata were taken from.
    #[serde(default)]
    pub tgdb_id: Option<u32>,
    /// Description shown by the console (`[Description]` of the launch entry).
    #[serde(default)]
    pub description: String,
    /// Name the hash database gave the ROM file, when it was found.
    #[serde(default)]
    pub db_name: Option<String>,
    /// SNES conversion details.
    #[serde(default)]
    pub snes: Option<SnesInfo>,
    /// NES header details.
    #[serde(default)]
    pub nes: Option<NesInfo>,
    /// Whether the cover PNGs are placeholders (no real art yet).
    #[serde(default)]
    pub placeholder_art: bool,
    /// Whether the ROM goes to the console as a 7z archive that `tiny7zx` extracts at launch
    /// (hakchi2 CE's "Compress games"). On by default for SNES and NES games; a `game.json`
    /// without the field is treated as on.
    #[serde(default = "default_compress")]
    pub compress: bool,
}

fn one() -> u8 {
    1
}

fn default_compress() -> bool {
    true
}

fn default_release_date() -> String {
    "1900-01-01".to_string()
}

fn default_publisher() -> String {
    "UNKNOWN".to_string()
}

impl Game {
    /// The title as written to the console (apostrophe fix applied).
    pub fn display_name(&self) -> String {
        fix_apostrophes(&self.name)
    }

    /// The sort key as written to the console.
    pub fn sort_title(&self) -> String {
        if self.sort_name.trim().is_empty() {
            sort_title(&self.name)
        } else {
            fix_apostrophes(&self.sort_name)
        }
    }

    /// File name of the cover image.
    pub fn cover_file(&self) -> String {
        format!("{}.png", self.code)
    }

    /// File name of the thumbnail.
    pub fn thumbnail_file(&self) -> String {
        format!("{}_small.png", self.code)
    }

    /// File name of the launch entry.
    pub fn desktop_file(&self) -> String {
        format!("{}.desktop", self.code)
    }

    /// File name of the packed ROM (`Chrono_Trigger.sfrom.7z`).
    pub fn archive_file(&self) -> String {
        archive::archive_name(&self.rom_file)
    }

    /// The ROM file as the console gets it: the archive when compressed, else the ROM.
    pub fn console_rom_file(&self) -> String {
        if self.compress {
            self.archive_file()
        } else {
            self.rom_file.clone()
        }
    }

    /// The launch entry with the game's files under `games_path/<code>/` (hakchi2 CE writes
    /// `/var/games` locally and rewrites the paths when uploading). A compressed game's
    /// command names the `.7z`; the console's wrapper extracts it at launch.
    pub fn desktop_entry(&self, games_path: &str) -> DesktopEntry {
        let rom = format!("{games_path}/{}/{}", self.code, self.console_rom_file());
        let exec = if self.args.is_empty() {
            format!("{} {rom}", self.application)
        } else {
            format!("{} {rom} {}", self.application, self.args)
        };
        DesktopEntry {
            exec,
            path: format!("{PROFILE_PATH}/{}", self.code),
            name: self.display_name(),
            icon: format!("{games_path}/{}/{}", self.code, self.cover_file()),
            code: self.code.clone(),
            test_id: 0,
            players: self.players,
            simultaneous: self.simultaneous,
            release_date: self.release_date.clone(),
            save_count: self.save_count,
            sort_title: self.sort_title(),
            publisher: self.publisher.clone(),
            copyright: self.copyright.clone(),
            description: self.description.clone(),
            ..DesktopEntry::new()
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::desktop::Format;

    pub(crate) fn chrono_trigger() -> Game {
        Game {
            code: "CLV-U-ZJSQU".into(),
            name: "Chrono Trigger".into(),
            sort_name: String::new(),
            system: System::Snes,
            rom_file: "Chrono_Trigger.sfrom".into(),
            crc32: 0x2D20_6BF7,
            file_crc32: 0x2D20_6BF7,
            original_file: "Chrono Trigger.smc".into(),
            players: 1,
            simultaneous: false,
            release_date: "1995-03-11".into(),
            publisher: "Squaresoft".into(),
            copyright: String::new(),
            save_count: 3,
            application: System::Snes.application().into(),
            args: System::Snes.default_args().into(),
            tgdb_ids: vec![1255],
            tgdb_id: None,
            description: String::new(),
            db_name: Some("Chrono Trigger (USA).sfc".into()),
            snes: None,
            nes: None,
            placeholder_art: true,
            compress: true,
        }
    }

    /// The entry hakchi2 CE uploaded for the compressed game of the trace, byte for byte. The
    /// one field Cartload has no source for is `country`: CE fills it from its own cart
    /// database (`snescarts.xml`, `SnesGame.TryAutofill`), which is not part of the library.
    #[test]
    fn compressed_entry_matches_what_ce_uploaded_byte_for_byte() {
        let game = chrono_trigger();
        assert!(game.compress);
        assert_eq!(game.console_rom_file(), "Chrono_Trigger.sfrom.7z");
        let mut entry = game.desktop_entry("/var/lib/hakchi/games/snes-eur/.storage");
        entry.country = "us".into();
        assert_eq!(
            entry.render(Format::Hakchi),
            include_str!("../tests/fixtures/ce-001-CLV-U-ZJSQU.desktop")
        );
    }

    #[test]
    fn uncompressed_entry_names_the_rom_itself() {
        let mut game = chrono_trigger();
        game.compress = false;
        assert_eq!(game.console_rom_file(), "Chrono_Trigger.sfrom");
        let entry = game.desktop_entry("/var/lib/hakchi/games/snes-eur/.storage");
        assert_eq!(
            entry.exec,
            "/bin/clover-canoe-shvc-wr -rom /var/lib/hakchi/games/snes-eur/.storage/CLV-U-ZJSQU/Chrono_Trigger.sfrom --volume 100 -rollback-snapshot-period 600"
        );
    }

    #[test]
    fn local_entry_uses_var_games_and_the_name_fixes() {
        let mut game = chrono_trigger();
        game.name = "Kirby's Dream Land '94".into();
        let entry = game.desktop_entry(GAMES_PATH);
        assert_eq!(
            entry.exec,
            "/bin/clover-canoe-shvc-wr -rom /var/games/CLV-U-ZJSQU/Chrono_Trigger.sfrom.7z --volume 100 -rollback-snapshot-period 600"
        );
        assert_eq!(entry.icon, "/var/games/CLV-U-ZJSQU/CLV-U-ZJSQU.png");
        assert_eq!(entry.path, "/var/saves/CLV-U-ZJSQU");
        assert_eq!(entry.name, "Kirby's Dream Land `94");
        assert_eq!(entry.sort_title, "kirby's dream land `94");
        game.sort_name = "Custom".into();
        assert_eq!(game.desktop_entry(GAMES_PATH).sort_title, "Custom");
    }

    #[test]
    fn json_round_trip_with_defaults() {
        let game = chrono_trigger();
        let json = serde_json::to_string_pretty(&game).expect("json");
        let back: Game = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, game);
        let minimal = r#"{"code":"CLV-H-AAAAA","name":"X","system":"nes","rom_file":"X.nes","crc32":1,"application":"/bin/clover-kachikachi-wr"}"#;
        let game: Game = serde_json::from_str(minimal).expect("parse");
        assert_eq!(game.players, 1);
        assert_eq!(game.publisher, "UNKNOWN");
        assert_eq!(game.release_date, "1900-01-01");
        assert!(
            game.compress,
            "a game.json from before compression is compressed"
        );
    }
}
