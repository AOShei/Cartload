//! ROM files: system detection, copier header removal, the checksums hakchi2 CE uses, the SNES
//! `.sfrom` container for the stock `canoe` emulator and the iNES header for `kachikachi`.
//!
//! The constants and rules come from hakchi2 CE (`Apps/SnesGame.cs`: `GetCorrectHeader`,
//! `MakeSfrom`, `SfromHeader1`, `SfromHeader2`, the preset table; `Apps/NesGame.cs` and
//! `NesFile.cs`; `NesApplication.Import` and `GenerateSafeFileName`). The output is byte for
//! byte what CE produces for the same ROM.

use std::path::Path;

use cartload_console::ConsoleFamily;
use serde::{Deserialize, Serialize};

use crate::{Error, Result, codes};

/// The systems whose games the library knows how to prepare. SNES and NES games run on the
/// console's own emulators (`canoe`, `kachikachi`) where the firmware has them; the others
/// always run through RetroArch and need its core module on the console.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum System {
    /// SNES / Super Famicom (`canoe`).
    Snes,
    /// NES / Famicom (`kachikachi`).
    Nes,
    /// Mega Drive / Genesis, through RetroArch (`/bin/md`, Genesis Plus GX).
    MegaDrive,
    /// Game Boy, through RetroArch (`/bin/gb`, Gambatte).
    GameBoy,
    /// Game Boy Color, through RetroArch (`/bin/gbc`, Gambatte).
    GameBoyColor,
    /// Game Boy Advance, through RetroArch (`/bin/gba`, mGBA).
    GameBoyAdvance,
    /// N64, through RetroArch (`/bin/n64`, Mupen64Plus). Slow on this hardware.
    N64,
}

impl System {
    /// Every system, in the order the app lists them.
    pub const ALL: [Self; 7] = [
        Self::Snes,
        Self::Nes,
        Self::MegaDrive,
        Self::GameBoy,
        Self::GameBoyColor,
        Self::GameBoyAdvance,
        Self::N64,
    ];

    /// The system a ROM file extension belongs to (case-insensitive, with or without the dot).
    /// The extensions are hakchi2 CE's (`AppTypeCollection`); `.bin` is left out because
    /// it says nothing about the system.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "sfc" | "smc" | "fig" | "swc" | "sfrom" => Some(Self::Snes),
            "nes" => Some(Self::Nes),
            "md" | "mdx" | "smd" | "gen" | "68k" => Some(Self::MegaDrive),
            "gb" | "sgb" => Some(Self::GameBoy),
            "gbc" => Some(Self::GameBoyColor),
            "gba" => Some(Self::GameBoyAdvance),
            "n64" | "z64" | "v64" => Some(Self::N64),
            _ => None,
        }
    }

    /// The system of a file, from its extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
    }

    /// Prefix letter of this system's game codes.
    pub const fn code_prefix(self) -> char {
        match self {
            Self::Snes => codes::PREFIX_SNES,
            Self::Nes => codes::PREFIX_NES,
            Self::MegaDrive => codes::PREFIX_MEGA_DRIVE,
            Self::GameBoy => codes::PREFIX_GAME_BOY,
            Self::GameBoyColor => codes::PREFIX_GAME_BOY_COLOR,
            Self::GameBoyAdvance => codes::PREFIX_GAME_BOY_ADVANCE,
            Self::N64 => codes::PREFIX_N64,
        }
    }

    /// The console family whose firmware emulator runs this system's games, when one does.
    pub const fn native_family(self) -> Option<ConsoleFamily> {
        match self {
            Self::Snes => Some(ConsoleFamily::Snes),
            Self::Nes => Some(ConsoleFamily::Nes),
            _ => None,
        }
    }

    /// Short name, as the app shows it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Snes => "SNES",
            Self::Nes => "NES",
            Self::MegaDrive => "Mega Drive / Genesis",
            Self::GameBoy => "Game Boy",
            Self::GameBoyColor => "Game Boy Color",
            Self::GameBoyAdvance => "Game Boy Advance",
            Self::N64 => "N64",
        }
    }

    /// The launch command, including the `-rom` switch where the emulator takes one
    /// (`clover-canoe-shvc-wr -rom <file>`, `clover-kachikachi-wr <file>`, `md <file>`).
    pub const fn application(self) -> &'static str {
        match self {
            Self::Snes => "/bin/clover-canoe-shvc-wr -rom",
            Self::Nes => "/bin/clover-kachikachi-wr",
            Self::MegaDrive => "/bin/md",
            Self::GameBoy => "/bin/gb",
            Self::GameBoyColor => "/bin/gbc",
            Self::GameBoyAdvance => "/bin/gba",
            Self::N64 => "/bin/n64",
        }
    }

    /// The RetroArch launcher a core module puts on the console for this system: what a
    /// launch entry calls when the game runs through RetroArch (`/bin/snes <file>`).
    pub const fn retroarch_launcher(self) -> &'static str {
        match self {
            Self::Snes => "snes",
            Self::Nes => "nes",
            Self::MegaDrive => "md",
            Self::GameBoy => "gb",
            Self::GameBoyColor => "gbc",
            Self::GameBoyAdvance => "gba",
            Self::N64 => "n64",
        }
    }

    /// The id of the core module (`cartload_hmod::modhub`) that plays this system's games
    /// through RetroArch.
    pub const fn core_module(self) -> &'static str {
        match self {
            Self::Snes => "snes9x",
            Self::Nes => "fceumm",
            Self::MegaDrive => "genesis",
            Self::GameBoy | Self::GameBoyColor => "gambatte",
            Self::GameBoyAdvance => "mgba",
            Self::N64 => "mupen64",
        }
    }

    /// Whether the system's games only ever run through RetroArch.
    pub const fn retroarch_only(self) -> bool {
        self.native_family().is_none()
    }

    /// The arguments hakchi2 CE puts after the ROM path.
    pub const fn default_args(self) -> &'static str {
        match self {
            Self::Snes => "--volume 100 -rollback-snapshot-period 600",
            Self::Nes => {
                "--guest-overscan-dimensions 0,0,9,3 --initial-fadein-durations 3,2 --volume 75 \
                 --enable-armet"
            }
            _ => "",
        }
    }
}

impl std::fmt::Display for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Whether `name` ends in an extension the importer accepts (a ROM or a zip).
pub fn is_importable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip")
        || Path::new(&lower)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(System::from_extension)
            .is_some()
}

/// CRC-32 (IEEE, the one zip and hakchi2 CE's `Shared.CRC32` use) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

/// The name of a ROM file inside its game directory: hakchi2 CE's `GenerateSafeFileName`.
/// Region and dump tags in parentheses or brackets are dropped, anything but letters, digits,
/// `.`, `!` and `-` becomes `_`, the stem is cut to 53 characters, and leading or trailing
/// `_` and `.` are removed.
pub fn safe_file_name(input: &str) -> String {
    let (stem, extension) = split_extension(input);
    let stem = strip_tags(stem);
    let mut safe = String::new();
    let mut pending_underscore = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '!' | '-') {
            if pending_underscore {
                safe.push('_');
                pending_underscore = false;
            }
            safe.push(c);
        } else {
            pending_underscore = true;
        }
    }
    if pending_underscore {
        safe.push('_');
    }
    // 64 characters, leaving room for the longest extension CE may add (".sfrom.lzma").
    const MAX_STEM: usize = 64 - ".sfrom.lzma".len();
    if safe.len() > MAX_STEM {
        safe.truncate(MAX_STEM);
    }
    format!("{}{extension}", safe.trim_matches(|c| c == '_' || c == '.'))
}

/// The display name hakchi2 CE derives from a file name: no extension, no tags in parentheses
/// or brackets, underscores as spaces, single spaces.
pub fn title_from_file_name(input: &str) -> String {
    let (stem, _) = split_extension(input);
    let title = strip_tags(stem).replace('_', " ");
    let mut collapsed = String::with_capacity(title.len());
    let mut last_space = false;
    for c in title.chars() {
        if c == ' ' {
            if !last_space {
                collapsed.push(c);
            }
            last_space = true;
        } else {
            collapsed.push(c);
            last_space = false;
        }
    }
    collapsed.trim().to_string()
}

/// Splits `name.ext` into (`name`, `.ext`); a name without a dot has an empty extension.
fn split_extension(input: &str) -> (&str, &str) {
    let file = input.rsplit(['/', '\\']).next().unwrap_or(input);
    match file.rfind('.') {
        Some(dot) if dot > 0 => (&file[..dot], &file[dot..]),
        _ => (file, ""),
    }
}

/// Removes ` (...)` and ` [...]` groups, like CE's `Regex.Replace(name, @" ?\(.*?\)| ?\[.*?\]", "")`.
fn strip_tags(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let space_before = c == ' ' && matches!(chars.get(i + 1), Some('(') | Some('['));
        let start = if space_before { i + 1 } else { i };
        let opener = chars.get(start).copied();
        if matches!(opener, Some('(') | Some('[')) {
            let close = if opener == Some('(') { ')' } else { ']' };
            if let Some(end) = chars[start + 1..].iter().position(|&x| x == close) {
                i = start + 1 + end + 1;
                continue;
            }
            // No closing bracket: the lazy `.*?` finds no match, the text stays.
        }
        out.push(c);
        i += 1;
    }
    out
}

/// A ROM prepared for the console: converted to the container the stock emulator expects,
/// with the data the launch entry needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    /// Target system.
    pub system: System,
    /// File name inside the game directory (`Chrono_Trigger.sfrom`).
    pub file_name: String,
    /// The bytes to store.
    pub data: Vec<u8>,
    /// CRC-32 the game code is derived from: the headerless ROM for SNES, PRG+CHR for NES.
    pub crc32: u32,
    /// CRC-32 of the input file (after copier header removal), for hash database lookups
    /// that key on the whole file.
    pub file_crc32: u32,
    /// Save slots the console offers: 3 when the cartridge has battery-backed RAM, else 0.
    pub save_count: u8,
    /// SNES details, when the system is SNES.
    pub snes: Option<SnesInfo>,
    /// NES details, when the system is NES.
    pub nes: Option<NesInfo>,
    /// Things that may go wrong on the console (unsupported mapper, known problem game, ...).
    pub warnings: Vec<String>,
    /// Things that were done and are fine (a copier header removed, ...).
    pub notes: Vec<String>,
}

impl Prepared {
    /// The game code (`CLV-U-ZJSQU`).
    pub fn code(&self) -> String {
        codes::game_code(self.crc32, self.system.code_prefix())
    }
}

/// Converts a ROM file (by name and content) into what the console needs.
pub fn prepare(file_name: &str, data: Vec<u8>) -> Result<Prepared> {
    let system = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .and_then(System::from_extension)
        .ok_or_else(|| Error::UnsupportedFile(file_name.into()))?;
    match system {
        System::Snes => prepare_snes(file_name, data),
        System::Nes => prepare_nes(file_name, data),
        system => Ok(prepare_plain(system, file_name, data)),
    }
}

/// A ROM RetroArch loads as it is: stored unchanged, the code from the whole file.
fn prepare_plain(system: System, file_name: &str, data: Vec<u8>) -> Prepared {
    let safe = safe_file_name(file_name);
    let (mut stem, extension) = split_extension(&safe);
    if stem.is_empty() {
        stem = "game";
    }
    let crc = crc32(&data);
    Prepared {
        system,
        file_name: format!("{stem}{}", extension.to_ascii_lowercase()),
        crc32: crc,
        file_crc32: crc,
        save_count: 0,
        snes: None,
        nes: None,
        warnings: Vec::new(),
        notes: Vec::new(),
        data,
    }
}

fn prepare_snes(file_name: &str, mut data: Vec<u8>) -> Result<Prepared> {
    let already_sfrom = file_name.to_ascii_lowercase().ends_with(".sfrom");
    let safe = safe_file_name(file_name);
    let (mut stem, _) = split_extension(&safe);
    if stem.is_empty() {
        stem = "game";
    }
    if already_sfrom {
        let crc = crc32(&data);
        return Ok(Prepared {
            system: System::Snes,
            file_name: format!("{stem}.sfrom"),
            crc32: crc,
            file_crc32: crc,
            save_count: 0,
            snes: None,
            nes: None,
            warnings: Vec::new(),
            notes: vec!["already an .sfrom file; stored unchanged".to_string()],
            data,
        });
    }
    let mut warnings = Vec::new();
    let mut notes = Vec::new();
    if !data.len().is_multiple_of(1024) {
        if data.len() < 512 {
            return Err(Error::InvalidRom {
                name: file_name.to_string(),
                reason: "file too small to be a ROM".to_string(),
            });
        }
        data.drain(..512);
        notes.push("removed a 512-byte copier header".to_string());
    }
    let crc = crc32(&data);
    let (sfrom, info) = snes::make_sfrom(&data).map_err(|reason| Error::InvalidRom {
        name: file_name.to_string(),
        reason,
    })?;
    if info.problem_game {
        warnings.push(format!(
            "\"{}\" is on hakchi2 CE's list of games that do not run well on the stock emulator",
            info.title
        ));
    }
    Ok(Prepared {
        system: System::Snes,
        file_name: format!("{stem}.sfrom"),
        data: sfrom,
        crc32: crc,
        file_crc32: crc,
        save_count: if info.sram_size > 0 { 3 } else { 0 },
        snes: Some(info),
        nes: None,
        warnings,
        notes,
    })
}

fn prepare_nes(file_name: &str, data: Vec<u8>) -> Result<Prepared> {
    let file_crc32 = crc32(&data);
    let header = nes::Header::parse(&data).map_err(|reason| Error::InvalidRom {
        name: file_name.to_string(),
        reason,
    })?;
    let normalised = nes::normalise(&data, &header);
    let rom_crc32 = nes::rom_crc32(&normalised);
    let mut warnings = Vec::new();
    if !nes::SUPPORTED_MAPPERS.contains(&header.mapper) {
        warnings.push(format!(
            "mapper {} is not supported by the stock NES emulator; the game may not start",
            header.mapper
        ));
    }
    if header.four_screen {
        warnings
            .push("four-screen mirroring is not supported by the stock NES emulator".to_string());
    }
    if header.trainer {
        warnings.push(
            "the ROM has a 512-byte trainer, which the stock emulator may ignore".to_string(),
        );
    }
    let safe = safe_file_name(file_name);
    let (mut stem, _) = split_extension(&safe);
    if stem.is_empty() {
        stem = "game";
    }
    Ok(Prepared {
        system: System::Nes,
        file_name: format!("{stem}.nes"),
        data: normalised,
        crc32: rom_crc32,
        file_crc32,
        save_count: if header.battery { 3 } else { 0 },
        snes: None,
        nes: Some(NesInfo {
            mapper: header.mapper,
            prg_size: header.prg_size(),
            chr_size: header.chr_size(),
            battery: header.battery,
            pal: header.pal,
        }),
        warnings,
        notes: Vec::new(),
    })
}

/// What was learnt from an SNES ROM while converting it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnesInfo {
    /// Internal title from the ROM header, trimmed.
    pub title: String,
    /// LoROM or HiROM.
    pub rom_type: snes::RomType,
    /// Preset id written to the `.sfrom` footer (0 when none applies).
    pub preset_id: u16,
    /// Special chip flag written to the footer (`0x0C` for Super FX).
    pub chip: u8,
    /// Cartridge RAM size code from the header (0 when there is no SRAM).
    pub sram_size: u8,
    /// Whether hakchi2 CE lists the game as problematic on `canoe`.
    pub problem_game: bool,
}

/// What was learnt from an NES ROM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NesInfo {
    /// iNES mapper number.
    pub mapper: u8,
    /// PRG ROM size in bytes.
    pub prg_size: usize,
    /// CHR ROM size in bytes.
    pub chr_size: usize,
    /// Battery-backed RAM present.
    pub battery: bool,
    /// PAL flag from header byte 9.
    pub pal: bool,
}

/// SNES ROM header and the `.sfrom` container.
pub mod snes {
    use serde::{Deserialize, Serialize};

    /// Memory map of the cartridge; the values are what the `.sfrom` footer stores.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum RomType {
        /// LoROM (`0x14`).
        LoRom,
        /// HiROM (`0x15`).
        HiRom,
    }

    impl RomType {
        /// The footer byte.
        pub const fn code(self) -> u8 {
            match self {
                Self::LoRom => 0x14,
                Self::HiRom => 0x15,
            }
        }

        /// Offset of the internal header for this map.
        pub const fn header_offset(self) -> usize {
            match self {
                Self::LoRom => 0x7FC0,
                Self::HiRom => 0xFFC0,
            }
        }
    }

    /// The 32-byte internal header at `$7FC0` (LoROM) or `$FFC0` (HiROM).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Header {
        /// Raw 21-byte title field.
        pub title_raw: [u8; 21],
        /// `$xFD5`: map mode and speed.
        pub makeup: u8,
        /// `$xFD6`: cartridge type (chips).
        pub rom_type: u8,
        /// `$xFD7`: ROM size code.
        pub rom_size: u8,
        /// `$xFD8`: RAM size code.
        pub sram_size: u8,
        /// `$xFD9`: destination code.
        pub country: u8,
        /// `$xFDA`: licensee.
        pub license: u8,
        /// `$xFDB`: version.
        pub version: u8,
        /// `$xFDC`: checksum complement.
        pub checksum_complement: u16,
        /// `$xFDE`: checksum.
        pub checksum: u16,
    }

    impl Header {
        /// Size of the header in bytes.
        pub const LEN: usize = 32;

        /// Reads the header at `offset`, if the ROM is large enough.
        pub fn read(rom: &[u8], offset: usize) -> Option<Self> {
            let bytes = rom.get(offset..offset + Self::LEN)?;
            let mut title_raw = [0u8; 21];
            title_raw.copy_from_slice(&bytes[..21]);
            Some(Self {
                title_raw,
                makeup: bytes[21],
                rom_type: bytes[22],
                rom_size: bytes[23],
                sram_size: bytes[24],
                country: bytes[25],
                license: bytes[26],
                version: bytes[27],
                checksum_complement: u16::from_le_bytes([bytes[28], bytes[29]]),
                checksum: u16::from_le_bytes([bytes[30], bytes[31]]),
            })
        }

        /// The title as hakchi2 CE reads it: empty when the field contains a 0 or 0xFF byte or
        /// starts with a space, otherwise the ASCII text without trailing spaces.
        pub fn title(&self) -> String {
            if self.title_raw.contains(&0)
                || self.title_raw.contains(&0xFF)
                || self.title_raw[0] == 0x20
            {
                return String::new();
            }
            let mut end = self.title_raw.len();
            while end > 0 && self.title_raw[end - 1] == 0x20 {
                end -= 1;
            }
            self.title_raw[..end]
                .iter()
                .map(|&b| char::from(b))
                .collect()
        }

        /// Whether the checksum and its complement agree.
        pub fn checksum_valid(&self) -> bool {
            (self.checksum ^ 0xFFFF) == self.checksum_complement
        }

        fn checksum_absent(&self) -> bool {
            !self.checksum_valid() || self.checksum == 0 || self.checksum_complement == 0
        }

        fn title_is_ascii(title: &str) -> bool {
            title.chars().all(|c| (c as u32) > 31 && (c as u32) <= 127)
        }
    }

    /// Finds the real header and the memory map (hakchi2 CE `GetCorrectHeader`).
    pub fn detect(rom: &[u8]) -> Result<(Header, RomType), String> {
        let lo = Header::read(rom, RomType::LoRom.header_offset())
            .ok_or_else(|| "ROM smaller than 32 KiB, no internal header".to_string())?;
        let hi = Header::read(rom, RomType::HiRom.header_offset());
        let Some(hi) = hi else {
            // Under 64 KiB there is only room for a LoROM header.
            return Ok((lo, RomType::LoRom));
        };
        let title_lo = lo.title();
        let title_hi = hi.title();
        let rom_type = if lo.checksum_valid() && hi.checksum_absent() {
            RomType::LoRom
        } else if lo.checksum_absent() && hi.checksum_valid() {
            RomType::HiRom
        } else if !title_lo.is_empty() && title_hi.is_empty() {
            RomType::LoRom
        } else if title_lo.is_empty() && !title_hi.is_empty() {
            RomType::HiRom
        } else if title_lo == title_hi && (lo.makeup & 1) == 0 {
            RomType::LoRom
        } else if title_lo == title_hi && (hi.makeup & 1) == 1 {
            RomType::HiRom
        } else {
            let lo_ascii = Header::title_is_ascii(&title_lo);
            let hi_ascii = Header::title_is_ascii(&title_hi);
            if lo_ascii && !hi_ascii {
                RomType::LoRom
            } else if !lo_ascii && hi_ascii {
                RomType::HiRom
            } else {
                return Err("can't detect ROM type, seems like ROM is corrupted".to_string());
            }
        };
        Ok(match rom_type {
            RomType::LoRom => (lo, RomType::LoRom),
            RomType::HiRom => (hi, RomType::HiRom),
        })
    }

    /// Cartridge types with a Super FX chip.
    pub const SFX_TYPES: [u8; 4] = [0x13, 0x14, 0x15, 0x1A];
    /// Cartridge types with a DSP-1 chip.
    pub const DSP1_TYPES: [u8; 2] = [0x03, 0x05];
    /// Cartridge types with an SA-1 chip.
    pub const SA1_TYPES: [u8; 2] = [0x34, 0x35];
    /// Preset id used for DSP-1 games (Super Mario Kart's).
    pub const PRESET_DSP1: u16 = 0x10BD;
    /// Preset id used for SA-1 games (Super Mario RPG's).
    pub const PRESET_SA1: u16 = 0x109C;
    /// Footer chip flag for Super FX.
    pub const CHIP_SUPER_FX: u8 = 0x0C;

    /// Internal titles with a known `canoe` preset id (hakchi2 CE `knownPresets`).
    pub const KNOWN_PRESETS: &[(&str, u16)] = &[
        ("SUPER MARIOWORLD", 0x1011),
        ("F-ZERO", 0x1018),
        ("SUPER MARIO KART", 0x10BD),
        ("Super Metroid", 0x1040),
        ("EARTH BOUND", 0x1070),
        ("Kirby's Dream Course", 0x1058),
        ("DONKEY KONG COUNTRY", 0x1077),
        ("KIRBY SUPER DELUXE", 0x109F),
        ("Super Punch-Out!!", 0x10A9),
        ("MEGAMAN X", 0x1109),
        ("SUPER GHOULS'N GHOST", 0x1003),
        ("Street Fighter2 Turb", 0x1065),
        ("SUPER MARIO RPG", 0x109E),
        ("FINAL FANTASY 3", 0x10DC),
        ("SUPER CASTLEVANIA 4", 0x1030),
        ("CONTRA3 THE ALIEN WA", 0x1036),
        ("STAR FOX", 0x1242),
        ("STARFOX2", 0x123C),
        ("SHVC FIREEMBLEM", 0x102B),
        ("SUPER DONKEY KONG", 0x1023),
        ("ROCKMAN X", 0x110A),
        ("CHOHMAKAIMURA", 0x1004),
        ("SeikenDensetsu 2", 0x10B2),
        ("FINAL FANTASY 6", 0x10DD),
        ("CONTRA SPIRITS", 0x1037),
        ("ganbare goemon", 0x1048),
        ("SUPER FORMATION SOCC", 0x1240),
        ("YOSSY'S ISLAND", 0x1243),
        ("FINAL FIGHT", 0x100E),
        ("DIDDY'S KONG QUEST", 0x105D),
        ("BREATH OF FIRE 2", 0x1068),
        ("FINAL FIGHT 2", 0x10E1),
        ("MEGAMAN X2", 0x1117),
        ("FINAL FIGHT 3", 0x10E3),
        ("GENGHIS KHAN 2", 0x10C4),
        ("CASTLEVANIA DRACULA", 0x1131),
        ("STREET FIGHTER ALPHA", 0x10DF),
        ("MEGAMAN 7", 0x113A),
        ("MEGAMAN X3", 0x113D),
        ("Breath of Fire", 0x1144),
    ];

    /// Internal titles of games hakchi2 CE knows do not run well on `canoe`.
    pub const PROBLEM_GAMES: &[&str] = &[
        "ActRaiser-2 USA",
        "ALIEN vs. PREDATOR",
        "ASTERIX",
        "BATMAN--REVENGE JOKER",
        "CHAMPIONSHIP POOL",
        "ClayFighter 2",
        "CLOCK TOWER SFX",
        "COOL WORLD",
        "CRYSTAL BEANS",
        "CYBER KNIGHT 2",
        "ASCII DARK LAW",
        "DIRT TRAX FX",
        "DBZ HYPER DIMENSION",
        "DRAGON BALL Z HD",
        "DRAGONBALL Z 2",
        "SFX DRAGONBALLZ2",
        "SFX SUPERBUTOUDEN2",
        "DUNGEON MASTER",
        "EARTHWORM JIM 2",
        "F1 WORLD CHAMP EDTION",
        "FACEBALL 2000",
        "THE FIREMEN     PAL",
        "HAMMERIN' HARRY (JPN)",
        "HARMELUNNOBAIOLINHIKI",
        "HOME ALONE",
        "HUMAN GRANDPRIX",
        "HUMAN GRANDPRIX 3",
        "ILLUSION OF GAIA USA",
        "ILLUSION OF TIME ENG",
        "JumpinDerby",
        "KRUSTYS SUPERFUNHOUSE",
        "Mario's Time Machine",
        "MARKOS MAGIC FOOTBALL",
        "POWER RANGERS FIGHT",
        "MOMOTETSU HAPPY",
        "NHL HOCKEY 1998",
        "RENDERING RANGER R2",
        "ROBOTREK 1 USA",
        "ROMANCING SAGA3",
        "SECRET OF EVERMORE",
        "SIM CITY 2000",
        "SMASH TENNIS",
        "Star Ocean",
        "STREET FIGHTER ALPHA2",
        "SUPER BASES LOADED 2",
        "PANIC BOMBER WORLD",
        "TALES OF PHANTASIA",
        "TERRANIGMA P",
        "TOP GEAR 3000",
        "UNIRACERS",
        "WARIO'S WOODS",
        "WORLD CLASS RUGBY",
        "WORLD CUP STRIKER",
        "WORLD MASTERS GOLF",
        "WWF SUPER WRESTLEMANI",
        "WRESTLEMANIA",
        "SENSIBLE SOCCER",
    ];

    /// Looks up the preset id for an internal title.
    pub fn known_preset(title: &str) -> Option<u16> {
        KNOWN_PRESETS
            .iter()
            .find(|(known, _)| *known == title)
            .map(|(_, id)| *id)
    }

    /// Size of the leading `.sfrom` header.
    pub const HEADER1_LEN: usize = 48;
    /// Size of the trailing `.sfrom` footer (hakchi2 CE's "header 2").
    pub const HEADER2_LEN: usize = 48;

    /// The 48-byte header at the start of an `.sfrom` file (hakchi2 CE `SfromHeader1`).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Header1 {
        /// 0x00: always `0x00000100`.
        pub unknown1: u32,
        /// 0x04: total file size.
        pub file_size: u32,
        /// 0x08: always `0x00000030` (offset of the ROM).
        pub unknown2: u32,
        /// 0x0C: offset of the end of the ROM.
        pub rom_end: u32,
        /// 0x10: offset of the footer; equal to the file size here.
        pub footer_start: u32,
        /// 0x14: offset of the "header 2" block (the footer).
        pub header2: u32,
        /// 0x18: offset of a "header 3" block; equal to the file size here.
        pub header3: u32,
        /// 0x1C: zero.
        pub unknown3: u32,
        /// 0x20: `rom_end + 27`.
        pub flags: u32,
        /// 0x24: eight bytes, `WUP-XXXX`.
        pub vc_game_id: [u8; 8],
        /// 0x2C: zero.
        pub unknown4: u32,
    }

    impl Header1 {
        /// The header for a ROM of `rom_size` bytes.
        pub fn new(rom_size: u32) -> Self {
            let file_size = rom_size + HEADER1_LEN as u32 + HEADER2_LEN as u32;
            let rom_end = HEADER1_LEN as u32 + rom_size;
            Self {
                unknown1: 0x0000_0100,
                file_size,
                unknown2: 0x0000_0030,
                rom_end,
                footer_start: file_size,
                header2: rom_end,
                header3: file_size,
                unknown3: 0,
                flags: rom_end + 27,
                vc_game_id: *b"WUP-XXXX",
                unknown4: 0,
            }
        }

        /// Little-endian layout.
        pub fn to_bytes(&self) -> [u8; HEADER1_LEN] {
            let mut out = [0u8; HEADER1_LEN];
            let words = [
                self.unknown1,
                self.file_size,
                self.unknown2,
                self.rom_end,
                self.footer_start,
                self.header2,
                self.header3,
                self.unknown3,
                self.flags,
            ];
            for (i, word) in words.iter().enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            out[0x24..0x2C].copy_from_slice(&self.vc_game_id);
            out[0x2C..0x30].copy_from_slice(&self.unknown4.to_le_bytes());
            out
        }

        /// Parses the header at the start of `bytes`.
        pub fn parse(bytes: &[u8]) -> Option<Self> {
            let b = bytes.get(..HEADER1_LEN)?;
            let word = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
            let mut vc_game_id = [0u8; 8];
            vc_game_id.copy_from_slice(&b[0x24..0x2C]);
            Some(Self {
                unknown1: word(0x00),
                file_size: word(0x04),
                unknown2: word(0x08),
                rom_end: word(0x0C),
                footer_start: word(0x10),
                header2: word(0x14),
                header3: word(0x18),
                unknown3: word(0x1C),
                flags: word(0x20),
                vc_game_id,
                unknown4: word(0x2C),
            })
        }
    }

    /// The 48-byte footer after the ROM (hakchi2 CE `SfromHeader2`, packed).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Header2 {
        /// 0x00: frames per second (60).
        pub fps: u8,
        /// 0x01: ROM size.
        pub rom_size: u32,
        /// 0x05: PCM data size (0).
        pub pcm_size: u32,
        /// 0x09: footer size (0).
        pub footer_size: u32,
        /// 0x0D: emulator preset id.
        pub preset_id: u16,
        /// 0x0F: controllers (2).
        pub max_controllers: u8,
        /// 0x10: volume (0x5A).
        pub volume: u8,
        /// 0x11: `0x14` LoROM, `0x15` HiROM.
        pub rom_type: u8,
        /// 0x12: special chip flags.
        pub chip: u32,
        /// 0x1B: 1.
        pub unknown3: u32,
        /// 0x1F: 1.
        pub unknown4: u32,
    }

    impl Header2 {
        /// The footer hakchi2 CE writes.
        pub fn new(rom_size: u32, preset_id: u16, rom_type: RomType, chip: u8) -> Self {
            Self {
                fps: 60,
                rom_size,
                pcm_size: 0,
                footer_size: 0,
                preset_id,
                max_controllers: 2,
                volume: 0x5A,
                rom_type: rom_type.code(),
                chip: u32::from(chip),
                unknown3: 1,
                unknown4: 1,
            }
        }

        /// Packed little-endian layout (bytes 0x16..0x1B and 0x23..0x30 are padding).
        pub fn to_bytes(&self) -> [u8; HEADER2_LEN] {
            let mut out = [0u8; HEADER2_LEN];
            out[0x00] = self.fps;
            out[0x01..0x05].copy_from_slice(&self.rom_size.to_le_bytes());
            out[0x05..0x09].copy_from_slice(&self.pcm_size.to_le_bytes());
            out[0x09..0x0D].copy_from_slice(&self.footer_size.to_le_bytes());
            out[0x0D..0x0F].copy_from_slice(&self.preset_id.to_le_bytes());
            out[0x0F] = self.max_controllers;
            out[0x10] = self.volume;
            out[0x11] = self.rom_type;
            out[0x12..0x16].copy_from_slice(&self.chip.to_le_bytes());
            out[0x1B..0x1F].copy_from_slice(&self.unknown3.to_le_bytes());
            out[0x1F..0x23].copy_from_slice(&self.unknown4.to_le_bytes());
            out
        }

        /// Parses a footer.
        pub fn parse(bytes: &[u8]) -> Option<Self> {
            let b = bytes.get(..HEADER2_LEN)?;
            let word = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
            Some(Self {
                fps: b[0],
                rom_size: word(0x01),
                pcm_size: word(0x05),
                footer_size: word(0x09),
                preset_id: u16::from_le_bytes([b[0x0D], b[0x0E]]),
                max_controllers: b[0x0F],
                volume: b[0x10],
                rom_type: b[0x11],
                chip: word(0x12),
                unknown3: word(0x1B),
                unknown4: word(0x1F),
            })
        }
    }

    /// Wraps a headerless ROM into the `.sfrom` container (hakchi2 CE `MakeSfrom`).
    pub fn make_sfrom(rom: &[u8]) -> Result<(Vec<u8>, super::SnesInfo), String> {
        let (header, rom_type) = detect(rom)?;
        let title = header.title();
        let rom_size = u32::try_from(rom.len()).map_err(|_| "ROM too large".to_string())?;
        let chip = if SFX_TYPES.contains(&header.rom_type) {
            CHIP_SUPER_FX
        } else {
            0
        };
        let preset_id = match known_preset(&title) {
            Some(id) => id,
            None if DSP1_TYPES.contains(&header.rom_type) => PRESET_DSP1,
            None if SA1_TYPES.contains(&header.rom_type) => PRESET_SA1,
            None => 0,
        };
        let header1 = Header1::new(rom_size);
        let header2 = Header2::new(rom_size, preset_id, rom_type, chip);
        let mut out = Vec::with_capacity(HEADER1_LEN + rom.len() + HEADER2_LEN);
        out.extend_from_slice(&header1.to_bytes());
        out.extend_from_slice(rom);
        out.extend_from_slice(&header2.to_bytes());
        let info = super::SnesInfo {
            problem_game: PROBLEM_GAMES.contains(&title.as_str()),
            title,
            rom_type,
            preset_id,
            chip,
            sram_size: header.sram_size,
        };
        Ok((out, info))
    }
}

/// iNES header handling for the stock NES emulator.
pub mod nes {
    /// Mappers the stock `kachikachi` emulator supports (hakchi2 CE `supportedMappers`).
    pub const SUPPORTED_MAPPERS: [u8; 12] = [0, 1, 2, 3, 4, 5, 7, 9, 10, 86, 87, 184];

    /// The 16-byte iNES header.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Header {
        /// PRG ROM in 16 KiB units.
        pub prg_banks: u8,
        /// CHR ROM in 8 KiB units.
        pub chr_banks: u8,
        /// Mapper number.
        pub mapper: u8,
        /// Vertical (true) or horizontal mirroring.
        pub vertical_mirroring: bool,
        /// Battery-backed RAM.
        pub battery: bool,
        /// 512-byte trainer before PRG.
        pub trainer: bool,
        /// Four-screen VRAM.
        pub four_screen: bool,
        /// VS Unisystem.
        pub vs_unisystem: bool,
        /// PlayChoice-10.
        pub playchoice10: bool,
        /// PAL (header byte 9 bit 0).
        pub pal: bool,
        /// Bytes 12..16 were not zero: an archaic header whose bytes 7..16 CE discards.
        pub archaic: bool,
    }

    impl Header {
        /// Parses the header, as hakchi2 CE's `NesFile` constructor does.
        pub fn parse(data: &[u8]) -> Result<Self, String> {
            if data.len() < 16 || &data[..4] != b"NES\x1a" {
                return Err("not an iNES file (missing NES\\x1a signature)".to_string());
            }
            let archaic = data[12..16].iter().any(|&b| b != 0);
            let byte7 = if archaic { 0 } else { data[7] };
            let byte9 = if archaic { 0 } else { data[9] };
            let flags6 = data[6];
            Ok(Self {
                prg_banks: data[4],
                chr_banks: data[5],
                mapper: (flags6 >> 4) | (byte7 & 0xF0),
                vertical_mirroring: flags6 & 1 != 0,
                battery: flags6 & 0b10 != 0,
                trainer: flags6 & 0b100 != 0,
                four_screen: flags6 & 0b1000 != 0,
                vs_unisystem: byte7 & 1 != 0,
                playchoice10: byte7 & 0b10 != 0,
                pal: byte9 & 1 != 0,
                archaic,
            })
        }

        /// PRG ROM size in bytes.
        pub fn prg_size(&self) -> usize {
            usize::from(self.prg_banks) * 16384
        }

        /// CHR ROM size in bytes.
        pub fn chr_size(&self) -> usize {
            usize::from(self.chr_banks) * 8192
        }

        /// The header hakchi2 CE's `NesFile.GetRaw` writes back: the same flags, zeroed
        /// padding bytes.
        pub fn to_bytes(&self) -> [u8; 16] {
            let mut h = [0u8; 16];
            h[..4].copy_from_slice(b"NES\x1a");
            h[4] = self.prg_banks;
            h[5] = self.chr_banks;
            let mut flags6 = 0u8;
            if self.vertical_mirroring {
                flags6 |= 1;
            }
            if self.battery {
                flags6 |= 0b10;
            }
            if self.trainer {
                flags6 |= 0b100;
            }
            if self.four_screen {
                flags6 |= 0b1000;
            }
            h[6] = flags6 | (self.mapper << 4);
            let mut flags7 = self.mapper & 0xF0;
            if self.vs_unisystem {
                flags7 |= 1;
            }
            if self.playchoice10 {
                flags7 |= 0b10;
            }
            h[7] = flags7;
            if self.pal {
                h[9] = 1;
            }
            h
        }
    }

    /// Rewrites the header in CE's normalised form and keeps trainer, PRG and CHR in file
    /// order. Missing bytes at the end of a short ROM are zero-filled, extra bytes dropped.
    pub fn normalise(data: &[u8], header: &Header) -> Vec<u8> {
        let body_len = if header.trainer { 512 } else { 0 } + header.prg_size() + header.chr_size();
        let mut out = Vec::with_capacity(16 + body_len);
        out.extend_from_slice(&header.to_bytes());
        let body = data.get(16..).unwrap_or(&[]);
        let take = body.len().min(body_len);
        out.extend_from_slice(&body[..take]);
        out.resize(16 + body_len, 0);
        out
    }

    /// CRC-32 of PRG+CHR (everything after the header and trainer): the checksum hakchi2 CE
    /// derives the game code from and looks up in its NES database.
    pub fn rom_crc32(normalised: &[u8]) -> u32 {
        let header = Header::parse(normalised).ok();
        let start = 16 + header.map_or(0, |h| if h.trainer { 512 } else { 0 });
        super::crc32(normalised.get(start..).unwrap_or(&[]))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// A synthetic headerless HiROM image with a valid internal header.
    pub(crate) fn synthetic_snes(
        title: &str,
        rom_type: snes::RomType,
        cart_type: u8,
        sram: u8,
    ) -> Vec<u8> {
        let mut rom = vec![0u8; 128 * 1024];
        let offset = rom_type.header_offset();
        let mut title_bytes = [0x20u8; 21];
        for (dst, src) in title_bytes.iter_mut().zip(title.bytes()) {
            *dst = src;
        }
        rom[offset..offset + 21].copy_from_slice(&title_bytes);
        rom[offset + 21] = match rom_type {
            snes::RomType::LoRom => 0x20,
            snes::RomType::HiRom => 0x21,
        };
        rom[offset + 22] = cart_type;
        rom[offset + 23] = 0x07;
        rom[offset + 24] = sram;
        rom[offset + 25] = 0x01;
        let checksum: u16 = 0x1234;
        rom[offset + 28..offset + 30].copy_from_slice(&(checksum ^ 0xFFFF).to_le_bytes());
        rom[offset + 30..offset + 32].copy_from_slice(&checksum.to_le_bytes());
        rom
    }

    #[test]
    fn systems_from_extensions() {
        assert_eq!(System::from_extension(".SMC"), Some(System::Snes));
        assert_eq!(System::from_extension("sfc"), Some(System::Snes));
        assert_eq!(System::from_extension(".nes"), Some(System::Nes));
        assert_eq!(System::from_extension(".md"), Some(System::MegaDrive));
        assert_eq!(System::from_extension(".bin"), None);
        assert_eq!(System::from_extension("GBC"), Some(System::GameBoyColor));
        assert_eq!(System::from_extension(".z64"), Some(System::N64));
        assert!(System::N64.retroarch_only());
        assert!(!System::Snes.retroarch_only());
        assert_eq!(System::from_extension(".zip"), None);
        assert!(is_importable("Game (USA).zip"));
        assert!(is_importable("game.NES"));
        assert!(!is_importable("readme.txt"));
    }

    #[test]
    fn crc32_matches_the_reference_vector() {
        // The standard check value of CRC-32/IEEE.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn safe_file_names_follow_ce() {
        assert_eq!(safe_file_name("Chrono Trigger.smc"), "Chrono_Trigger.smc");
        assert_eq!(
            safe_file_name("Legend of Zelda, The - A Link to the Past (USA).sfc"),
            "Legend_of_Zelda_The_-_A_Link_to_the_Past.sfc"
        );
        assert_eq!(
            safe_file_name("Super Punch-Out!! [!].smc"),
            "Super_Punch-Out!!.smc"
        );
        assert_eq!(safe_file_name("__weird name__.nes"), "weird_name.nes");
        let long = format!("{}.sfc", "a".repeat(80));
        let safe = safe_file_name(&long);
        assert_eq!(safe.len(), 53 + 4);
        assert!(safe.ends_with(".sfc"));
    }

    #[test]
    fn titles_from_file_names_follow_ce() {
        assert_eq!(
            title_from_file_name("Chrono Trigger (USA).sfc"),
            "Chrono Trigger"
        );
        assert_eq!(
            title_from_file_name("Mega Man 3 (Europe) (Rev 1).nes"),
            "Mega Man 3"
        );
        assert_eq!(
            title_from_file_name("Super_Mario__World [!].smc"),
            "Super Mario World"
        );
        assert_eq!(title_from_file_name("/path/to/Game.zip"), "Game");
        assert_eq!(
            title_from_file_name("Unbalanced (paren.nes"),
            "Unbalanced (paren"
        );
    }

    #[test]
    fn snes_header_detection_and_title() {
        let rom = synthetic_snes("CHRONO TRIGGER", snes::RomType::HiRom, 0x02, 0x03);
        let (header, rom_type) = snes::detect(&rom).unwrap();
        assert_eq!(rom_type, snes::RomType::HiRom);
        assert_eq!(header.title(), "CHRONO TRIGGER");
        assert_eq!(header.sram_size, 3);
        assert!(header.checksum_valid());

        let rom = synthetic_snes("F-ZERO", snes::RomType::LoRom, 0x00, 0x00);
        let (header, rom_type) = snes::detect(&rom).unwrap();
        assert_eq!(rom_type, snes::RomType::LoRom);
        assert_eq!(header.title(), "F-ZERO");

        let mut title = [0x20u8; 21];
        title[0] = b'A';
        title[5] = 0;
        let header = snes::Header {
            title_raw: title,
            makeup: 0,
            rom_type: 0,
            rom_size: 0,
            sram_size: 0,
            country: 0,
            license: 0,
            version: 0,
            checksum_complement: 0,
            checksum: 0,
        };
        assert_eq!(header.title(), "", "a NUL byte makes the title empty");
    }

    #[test]
    fn sfrom_headers_have_the_ce_layout() {
        // Chrono Trigger: 4 MiB ROM gives a 4194400-byte .sfrom (trace: "Size: `4194400 bytes`").
        let h1 = snes::Header1::new(4_194_304);
        assert_eq!(h1.file_size, 4_194_400);
        assert_eq!(h1.rom_end, 4_194_352);
        assert_eq!(h1.header2, 4_194_352);
        assert_eq!(h1.flags, 4_194_352 + 27);
        let bytes = h1.to_bytes();
        assert_eq!(&bytes[0..4], &[0x00, 0x01, 0x00, 0x00]);
        assert_eq!(&bytes[8..12], &[0x30, 0x00, 0x00, 0x00]);
        assert_eq!(&bytes[0x24..0x2C], b"WUP-XXXX");
        assert_eq!(snes::Header1::parse(&bytes).unwrap(), h1);

        let h2 = snes::Header2::new(4_194_304, 0x10BD, snes::RomType::HiRom, 0x0C);
        let bytes = h2.to_bytes();
        assert_eq!(bytes[0x00], 60);
        assert_eq!(&bytes[0x01..0x05], &4_194_304u32.to_le_bytes());
        assert_eq!(&bytes[0x0D..0x0F], &[0xBD, 0x10]);
        assert_eq!(bytes[0x0F], 2);
        assert_eq!(bytes[0x10], 0x5A);
        assert_eq!(bytes[0x11], 0x15);
        assert_eq!(&bytes[0x12..0x16], &[0x0C, 0, 0, 0]);
        assert_eq!(&bytes[0x16..0x1B], &[0; 5]);
        assert_eq!(&bytes[0x1B..0x1F], &[1, 0, 0, 0]);
        assert_eq!(&bytes[0x1F..0x23], &[1, 0, 0, 0]);
        assert_eq!(&bytes[0x23..0x30], &[0; 13]);
        assert_eq!(snes::Header2::parse(&bytes).unwrap(), h2);
    }

    #[test]
    fn make_sfrom_wraps_the_rom_and_detects_chips_and_presets() {
        let rom = synthetic_snes("CHRONO TRIGGER", snes::RomType::HiRom, 0x02, 0x03);
        let (sfrom, info) = snes::make_sfrom(&rom).unwrap();
        assert_eq!(sfrom.len(), rom.len() + 96);
        assert_eq!(&sfrom[48..48 + rom.len()], &rom[..]);
        assert_eq!(info.preset_id, 0);
        assert_eq!(info.chip, 0);
        assert_eq!(info.rom_type, snes::RomType::HiRom);
        assert_eq!(info.title, "CHRONO TRIGGER");
        assert!(!info.problem_game);

        let (_, info) =
            snes::make_sfrom(&synthetic_snes("STAR FOX", snes::RomType::LoRom, 0x13, 0)).unwrap();
        assert_eq!(info.preset_id, 0x1242, "known preset wins");
        assert_eq!(info.chip, snes::CHIP_SUPER_FX);

        let (_, info) =
            snes::make_sfrom(&synthetic_snes("PILOTWINGS", snes::RomType::LoRom, 0x03, 0)).unwrap();
        assert_eq!(info.preset_id, snes::PRESET_DSP1);
        let (_, info) = snes::make_sfrom(&synthetic_snes(
            "SOME SA1 GAME",
            snes::RomType::HiRom,
            0x35,
            0,
        ))
        .unwrap();
        assert_eq!(info.preset_id, snes::PRESET_SA1);
        let (_, info) =
            snes::make_sfrom(&synthetic_snes("UNIRACERS", snes::RomType::LoRom, 0x00, 0)).unwrap();
        assert!(info.problem_game);
    }

    #[test]
    fn prepare_snes_strips_copier_headers_and_names_the_file() {
        let rom = synthetic_snes("CHRONO TRIGGER", snes::RomType::HiRom, 0x02, 0x03);
        let mut headered = vec![0xEEu8; 512];
        headered.extend_from_slice(&rom);
        let prepared = prepare("Chrono Trigger (USA).smc", headered).unwrap();
        assert_eq!(prepared.system, System::Snes);
        assert_eq!(prepared.file_name, "Chrono_Trigger.sfrom");
        assert_eq!(
            prepared.crc32,
            crc32(&rom),
            "the code hash is the headerless ROM's"
        );
        assert_eq!(prepared.code(), codes::game_code(crc32(&rom), 'U'));
        assert_eq!(prepared.save_count, 3);
        assert_eq!(prepared.data.len(), rom.len() + 96);
        assert!(prepared.warnings.is_empty());
        assert!(prepared.notes.iter().any(|n| n.contains("copier header")));

        let plain = prepare("game.sfc", rom.clone()).unwrap();
        assert!(plain.warnings.is_empty());
        assert!(plain.notes.is_empty());
        assert_eq!(plain.data, prepared.data);
    }

    #[test]
    fn prepare_rejects_garbage_and_keeps_retroarch_roms_unchanged() {
        assert!(matches!(
            prepare("x.sfc", vec![0u8; 1024]),
            Err(Error::InvalidRom { .. })
        ));
        let md = prepare("Sonic the Hedgehog (USA, Europe).MD", vec![7u8; 1024]).expect("md");
        assert_eq!(md.system, System::MegaDrive);
        assert_eq!(md.file_name, "Sonic_the_Hedgehog.md");
        assert_eq!(md.data, vec![7u8; 1024]);
        assert_eq!(md.crc32, crc32(&[7u8; 1024]));
        assert!(md.code().starts_with("CLV-G-"));
        assert!(matches!(
            prepare("x.txt", vec![]),
            Err(Error::UnsupportedFile(_))
        ));
        assert!(matches!(
            prepare("x.nes", b"junk".to_vec()),
            Err(Error::InvalidRom { .. })
        ));
    }

    /// A synthetic iNES image: 2 PRG banks, 1 CHR bank, mapper `mapper`, optional battery.
    pub(crate) fn synthetic_nes(mapper: u8, battery: bool) -> Vec<u8> {
        let mut data = vec![0u8; 16];
        data[..4].copy_from_slice(b"NES\x1a");
        data[4] = 2;
        data[5] = 1;
        data[6] = (mapper << 4) | 1 | if battery { 0b10 } else { 0 };
        data[7] = mapper & 0xF0;
        let body: Vec<u8> = (0..(2 * 16384 + 8192)).map(|i| (i % 251) as u8).collect();
        data.extend_from_slice(&body);
        data
    }

    #[test]
    fn nes_header_round_trip_and_code_hash() {
        let data = synthetic_nes(4, true);
        let header = nes::Header::parse(&data).unwrap();
        assert_eq!(header.mapper, 4);
        assert!(header.battery);
        assert!(header.vertical_mirroring);
        assert_eq!(header.prg_size(), 32768);
        assert_eq!(header.to_bytes(), data[..16]);
        let normalised = nes::normalise(&data, &header);
        assert_eq!(normalised, data);
        assert_eq!(nes::rom_crc32(&normalised), crc32(&data[16..]));

        let prepared = prepare("Game (USA).nes", data.clone()).unwrap();
        assert_eq!(prepared.system, System::Nes);
        assert_eq!(prepared.file_name, "Game.nes");
        assert_eq!(prepared.crc32, crc32(&data[16..]));
        assert_eq!(prepared.file_crc32, crc32(&data));
        assert_eq!(prepared.save_count, 3);
        assert!(prepared.warnings.is_empty());
        assert_eq!(prepared.code().as_bytes()[4], b'H');
    }

    #[test]
    fn nes_archaic_headers_are_cleaned_and_unsupported_mappers_warn() {
        let mut data = synthetic_nes(0xA5, false);
        data[12..16].copy_from_slice(b"DiSk");
        let header = nes::Header::parse(&data).unwrap();
        assert!(header.archaic);
        assert_eq!(
            header.mapper, 0x05,
            "the high nibble in byte 7 is discarded"
        );
        let normalised = nes::normalise(&data, &header);
        assert_eq!(&normalised[7..16], &[0; 9]);
        assert_eq!(normalised.len(), data.len());

        let prepared = prepare("odd.nes", synthetic_nes(66, false)).unwrap();
        assert!(prepared.warnings.iter().any(|w| w.contains("mapper 66")));

        let short = synthetic_nes(0, false)[..16 + 1000].to_vec();
        let header = nes::Header::parse(&short).unwrap();
        assert_eq!(nes::normalise(&short, &header).len(), 16 + 2 * 16384 + 8192);
    }
}
