//! The console model and the parsers for what the device prints.
//!
//! Everything here is pure: the strings come from the `hakchi` script, `df`, `du` and `ls` over
//! SSH, and the mDNS TXT record. The sample values in the tests are the ones in
//! `docs/reference/hakchi-ce-sync-trace.log`. Type and region detection mirrors hakchi2 CE
//! (`hakchi.cs`: `SystemCodeToConsoleType`, `ConvertVersion`, the minimum versions, and
//! `MemoryStats`).

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// A console model and region, as the device reports it through `$sftype-$sfregion`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsoleType {
    /// NES Classic Mini (USA / EUR), `nes-usa`.
    Nes,
    /// Famicom Mini, `nes-jpn`.
    Famicom,
    /// SNES Classic Mini, European model, `snes-eur`.
    SnesEur,
    /// SNES Classic, North American model, `snes-usa`.
    SnesUsa,
    /// Super Famicom Mini, `snes-jpn`.
    SuperFamicom,
    /// Famicom Mini, Shonen Jump 50th anniversary edition, `hvcj-jpn`.
    ShonenJump,
    /// Mega Drive Mini, Japan, `md-jpn`.
    MdJpn,
    /// Genesis Mini, North America, `md-usa`.
    MdUsa,
    /// Mega Drive Mini, Europe, `md-eur`.
    MdEur,
    /// Mega Drive Mini, Asia, `md-asia`.
    MdAsia,
    /// Not recognised (or not detected yet).
    #[default]
    Unknown,
}

/// Hardware family: which emulator and game format the console uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsoleFamily {
    /// NES Classic Mini, Famicom Mini and the Shonen Jump edition (`kachikachi`).
    Nes,
    /// SNES Classic Mini and Super Famicom Mini (`canoe`).
    Snes,
    /// Mega Drive / Genesis Mini (`m2engage`).
    MegaDrive,
    /// Not recognised.
    #[default]
    Unknown,
}

/// Sales region of the console.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Region {
    /// North America (and the NES Classic sold in Europe, which reports `usa`).
    Usa,
    /// Europe.
    Eur,
    /// Japan.
    Jpn,
    /// Asia (Mega Drive Mini only).
    Asia,
    /// Not recognised.
    #[default]
    Unknown,
}

/// System codes in the order hakchi2 CE lists them, with their console type.
pub const SYSTEM_CODES: [(&str, ConsoleType); 10] = [
    ("nes-usa", ConsoleType::Nes),
    ("nes-jpn", ConsoleType::Famicom),
    ("snes-eur", ConsoleType::SnesEur),
    ("snes-usa", ConsoleType::SnesUsa),
    ("snes-jpn", ConsoleType::SuperFamicom),
    ("hvcj-jpn", ConsoleType::ShonenJump),
    ("md-jpn", ConsoleType::MdJpn),
    ("md-usa", ConsoleType::MdUsa),
    ("md-eur", ConsoleType::MdEur),
    ("md-asia", ConsoleType::MdAsia),
];

impl ConsoleType {
    /// The model behind the USB product string a console in normal boot on stock firmware
    /// reports (`CLV-S-SNPY` for the European SNES). The strings come from the gadget
    /// driver of the stock kernel, which carries one per model. The Mega Drive Mini is not
    /// in that table, and the Shonen Jump edition reports as a Famicom.
    pub fn from_usb_product(product: &str) -> Option<Self> {
        match product.trim() {
            "CLV-S-NESY" => Some(Self::Nes),
            "CLV-S-HVCY" => Some(Self::Famicom),
            "CLV-S-SHVY" => Some(Self::SuperFamicom),
            "CLV-S-SNPY" => Some(Self::SnesEur),
            "CLV-S-SNSY" => Some(Self::SnesUsa),
            _ => None,
        }
    }

    /// Looks up a system code such as `snes-eur` (the output of
    /// `hakchi eval 'echo "$sftype-$sfregion"'`). Unknown codes give [`ConsoleType::Unknown`].
    pub fn from_system_code(code: &str) -> Self {
        let code = code.trim();
        SYSTEM_CODES
            .iter()
            .find(|(known, _)| *known == code)
            .map_or(Self::Unknown, |(_, console)| *console)
    }

    /// Combines the `type` and `region` values of the mDNS TXT record (`snes`, `eur`).
    pub fn from_type_and_region(console_type: &str, region: &str) -> Self {
        Self::from_system_code(&format!("{}-{}", console_type.trim(), region.trim()))
    }

    /// The system code the device uses for this console (`snes-eur`), also the name of the
    /// per-console games directory under the sync storage.
    pub const fn system_code(self) -> Option<&'static str> {
        Some(match self {
            Self::Nes => "nes-usa",
            Self::Famicom => "nes-jpn",
            Self::SnesEur => "snes-eur",
            Self::SnesUsa => "snes-usa",
            Self::SuperFamicom => "snes-jpn",
            Self::ShonenJump => "hvcj-jpn",
            Self::MdJpn => "md-jpn",
            Self::MdUsa => "md-usa",
            Self::MdEur => "md-eur",
            Self::MdAsia => "md-asia",
            Self::Unknown => return None,
        })
    }

    /// Hardware family.
    pub const fn family(self) -> ConsoleFamily {
        match self {
            Self::Nes | Self::Famicom | Self::ShonenJump => ConsoleFamily::Nes,
            Self::SnesEur | Self::SnesUsa | Self::SuperFamicom => ConsoleFamily::Snes,
            Self::MdJpn | Self::MdUsa | Self::MdEur | Self::MdAsia => ConsoleFamily::MegaDrive,
            Self::Unknown => ConsoleFamily::Unknown,
        }
    }

    /// Sales region.
    pub const fn region(self) -> Region {
        match self {
            Self::Nes | Self::SnesUsa | Self::MdUsa => Region::Usa,
            Self::SnesEur | Self::MdEur => Region::Eur,
            Self::Famicom | Self::SuperFamicom | Self::ShonenJump | Self::MdJpn => Region::Jpn,
            Self::MdAsia => Region::Asia,
            Self::Unknown => Region::Unknown,
        }
    }

    /// Human readable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nes => "NES Classic Mini (USA/EUR)",
            Self::Famicom => "Famicom Mini",
            Self::SnesEur => "SNES Classic Mini (EUR)",
            Self::SnesUsa => "SNES Classic (USA)",
            Self::SuperFamicom => "Super Famicom Mini",
            Self::ShonenJump => "Famicom Mini (Shonen Jump edition)",
            Self::MdJpn => "Mega Drive Mini (JPN)",
            Self::MdUsa => "Genesis Mini (USA)",
            Self::MdEur => "Mega Drive Mini (EUR)",
            Self::MdAsia => "Mega Drive Mini (ASIA)",
            Self::Unknown => "unknown console",
        }
    }

    /// Whether the stock UI shows pixel art next to the game list (NES Classic and Famicom).
    pub const fn has_pixel_art(self) -> bool {
        matches!(self, Self::Nes | Self::Famicom)
    }

    /// Where the original games live inside the stock firmware squashfs, when the device's
    /// `$gamepath` is not available (hakchi2 CE `GamesSquashFsPath`). Empty for Mega Drive.
    pub const fn games_squashfs_path(self) -> &'static str {
        match self.family() {
            ConsoleFamily::Nes | ConsoleFamily::Unknown => "/usr/share/games/nes/kachikachi",
            ConsoleFamily::Snes => "/usr/share/games",
            ConsoleFamily::MegaDrive => "",
        }
    }
}

impl fmt::Display for ConsoleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Version strings from `/var/version` on the console.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
    /// `bootVersion`, the u-boot / boot image version (for example `1.0.3`).
    pub boot: String,
    /// `kernelVersion`, as `uname -r` reports it (for example `3.4.113.29-madmonkey`).
    pub kernel: String,
    /// `hakchiVersion`, the device script version (for example `v1.0.4-126`).
    pub script: String,
    /// `cartloadVersion`, the version of Cartload's system (for example `0.1.0`); `None` with
    /// hakchi's own package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

impl Versions {
    /// Parses `source /var/version && echo "$bootVersion $kernelVersion $hakchiVersion
    /// $cartloadVersion"` (the last field is empty without Cartload's system).
    pub fn parse_echo(output: &str) -> Result<Self> {
        let mut fields = output.split_whitespace();
        match (fields.next(), fields.next(), fields.next()) {
            (Some(boot), Some(kernel), Some(script)) => Ok(Self {
                boot: boot.to_string(),
                kernel: kernel.to_string(),
                script: script.to_string(),
                system: fields.next().map(str::to_string),
            }),
            _ => Err(Error::Parse {
                what: "version triple",
                input: output.to_string(),
            }),
        }
    }

    /// Parses the `/var/version` file itself (`bootVersion='1.0.3'` lines), which is also what
    /// `var/version` inside `hakchi.hmod` contains.
    pub fn parse_file(contents: &str) -> Result<Self> {
        let values = parse_shell_assignments(contents);
        let get = |key: &str| {
            values.get(key).cloned().ok_or_else(|| Error::Parse {
                what: "version file",
                input: contents.to_string(),
            })
        };
        Ok(Self {
            boot: get("bootVersion")?,
            kernel: get("kernelVersion")?,
            script: get("hakchiVersion")?,
            system: values.get("cartloadVersion").cloned(),
        })
    }

    /// What hakchi2 CE assumes when `/var/version` does not exist: a kernel too old to work with.
    pub fn outdated_placeholder() -> Self {
        Self {
            boot: "1.0.0".into(),
            kernel: "3.4.112-00".into(),
            script: "v1.0.0-000".into(),
            system: None,
        }
    }

    /// Oldest boot version the host tooling supports (hakchi2 CE `MinimumBootVersion`).
    pub fn minimum_boot() -> Version {
        Version(vec![1, 0, 2])
    }

    /// Oldest kernel version the host tooling supports (hakchi2 CE `MinimumKernelVersion`).
    pub fn minimum_kernel() -> Version {
        Version(vec![3, 4, 113])
    }

    /// Oldest script version the host tooling supports (hakchi2 CE `MinimumScriptVersion`).
    pub fn minimum_script() -> Version {
        Version(vec![1, 0, 4, 118])
    }

    /// The kernel or boot image is too old: the kernel must be reinstalled.
    pub fn requires_reflash(&self) -> bool {
        Version::parse(&self.kernel) < Self::minimum_kernel()
            || Version::parse(&self.boot) < Self::minimum_boot()
    }

    /// The device script is too old: the hakchi rootfs must be updated.
    pub fn requires_rootfs_update(&self) -> bool {
        Version::parse(&self.script) < Self::minimum_script()
    }

    /// Whether the host can use this console as is (neither reflash nor rootfs update needed).
    pub fn can_interact(&self) -> bool {
        !self.requires_reflash() && !self.requires_rootfs_update()
    }
}

impl fmt::Display for Versions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(system) = &self.system {
            write!(f, "Cartload system {system}, ")?;
        }
        write!(
            f,
            "boot {}, kernel {}, script {}",
            self.boot, self.kernel, self.script
        )
    }
}

/// A dotted version number for comparisons, extracted the way hakchi2 CE's `ConvertVersion`
/// does: the first run of three or four numbers separated by `.` or `-`, so
/// `3.4.113.29-madmonkey` is 3.4.113.29 and `v1.0.4-126` is 1.0.4.126. Strings without such a
/// run compare as 0.0.0.0. Comparison is lexicographic; a shorter version that is a prefix of a
/// longer one is the smaller one (as .NET `Version` with undefined components).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Version(pub Vec<u32>);

impl Version {
    /// Extracts the version number from a free-form string.
    pub fn parse(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !bytes[i].is_ascii_digit() {
                i += 1;
                continue;
            }
            let mut parts = Vec::new();
            let mut j = i;
            loop {
                let start = j;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                parts.push(text[start..j].parse::<u32>().unwrap_or(u32::MAX));
                let separator = j < bytes.len() && matches!(bytes[j], b'.' | b'-');
                let digit_follows = j + 1 < bytes.len() && bytes[j + 1].is_ascii_digit();
                if separator && digit_follows && parts.len() < 4 {
                    j += 1;
                } else {
                    break;
                }
            }
            if parts.len() >= 3 {
                return Self(parts);
            }
            i = j;
        }
        Self(vec![0, 0, 0, 0])
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&text.join("."))
    }
}

/// Which firmware the console is running, from `hakchi currentFirmware`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Firmware {
    /// The stock firmware in NAND (`_nand_`).
    Nand,
    /// A custom firmware image loaded from the given path.
    Custom(String),
}

impl fmt::Display for Firmware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nand => f.write_str("stock NAND firmware"),
            Self::Custom(path) => write!(f, "custom firmware {path}"),
        }
    }
}

/// Parses `hakchi currentFirmware`.
pub fn parse_current_firmware(output: &str) -> Result<Firmware> {
    match output.trim() {
        "" => Err(Error::Parse {
            what: "current firmware",
            input: output.to_string(),
        }),
        "_nand_" => Ok(Firmware::Nand),
        path => Ok(Firmware::Custom(path.to_string())),
    }
}

/// Normalises `hakchi hwid` (`0461872a 87280207 9d2818c7 6c118000`) to one lowercase hex string
/// with the spaces removed, which is how hakchi2 CE identifies a console.
pub fn parse_hwid(output: &str) -> Result<String> {
    let hwid: String = output
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hwid.is_empty() || !hwid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Parse {
            what: "hardware id",
            input: output.to_string(),
        });
    }
    Ok(hwid)
}

/// Space on the storage that holds the games, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageUsage {
    /// Size of the file system.
    pub total: u64,
    /// Bytes in use.
    pub used: u64,
    /// Bytes available.
    pub free: u64,
}

impl fmt::Display for StorageUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Storage size: {}, used: {}, free: {}",
            megabytes(self.total as f64),
            megabytes(self.used as f64),
            megabytes(self.free as f64)
        )
    }
}

/// Parses the output of
/// `df "$(hakchi findGameSyncStorage)" | tail -n 1 | awk '{ print $2 " | " $3 " | " $4 }'`:
/// three 1 KiB block counts separated by ` | `.
pub fn parse_df_summary(output: &str) -> Result<StorageUsage> {
    let error = || Error::Parse {
        what: "df summary",
        input: output.to_string(),
    };
    let fields: Vec<u64> = output
        .split('|')
        .map(|field| field.trim().parse::<u64>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| error())?;
    let [total, used, free] = fields[..] else {
        return Err(error());
    };
    Ok(StorageUsage {
        total: total * 1024,
        used: used * 1024,
        free: free * 1024,
    })
}

/// Size of one directory, from `du`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirSize {
    /// Path as `du` printed it.
    pub path: String,
    /// Size in bytes (1 KiB blocks times 1024).
    pub bytes: u64,
}

impl DirSize {
    /// Last path component.
    pub fn name(&self) -> &str {
        self.path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&self.path)
    }
}

/// Parses the output of `du -d 1 "<path>" | head -n -1 | awk '{ print $2 " | " $1 }'`: one
/// `path | kilobytes` line per immediate subdirectory. Empty output is an empty list.
pub fn parse_du_summary(output: &str) -> Result<Vec<DirSize>> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let error = || Error::Parse {
                what: "du summary line",
                input: line.to_string(),
            };
            let (path, size) = line.rsplit_once(" | ").ok_or_else(error)?;
            let kib: u64 = size.trim().parse().map_err(|_| error())?;
            Ok(DirSize {
                path: path.trim().to_string(),
                bytes: kib * 1024,
            })
        })
        .collect()
}

/// How the games storage is used, split the way hakchi2 CE's `MemoryStats` does it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamesUsage {
    /// Everything under the sync storage.
    pub all: u64,
    /// Legacy single-console layouts: `.storage` and the three-digit menu directories.
    pub non_multiboot: u64,
    /// Per-console collections, by system code (`snes-eur`).
    pub collections: BTreeMap<String, u64>,
    /// Anything else (mods, configs, ...).
    pub extra: u64,
}

/// Classifies `du` entries of the sync storage (hakchi2 CE `MemoryStats.getCollectionsSize`).
pub fn classify_games_usage(dirs: &[DirSize]) -> GamesUsage {
    let mut usage = GamesUsage::default();
    for dir in dirs {
        let name = dir.name();
        let is_menu_code = name.len() == 3 && name.chars().all(|c| c.is_ascii_digit());
        if name == ".storage" || is_menu_code {
            usage.non_multiboot += dir.bytes;
        } else if ConsoleType::from_system_code(name) != ConsoleType::Unknown {
            *usage.collections.entry(name.to_string()).or_default() += dir.bytes;
        } else {
            usage.extra += dir.bytes;
        }
        usage.all += dir.bytes;
    }
    usage
}

/// The storage picture hakchi2 CE prints after connecting and after a sync.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageReport {
    /// The games sync storage (`hakchi findGameSyncStorage`).
    pub path: String,
    /// `df` of that storage.
    pub usage: StorageUsage,
    /// Bytes used by save states (`du -s` of `/var/saves`).
    pub saves: u64,
    /// Whether save states live on external storage (`/var/lib/clover/profiles` is a mount).
    pub external_saves: bool,
    /// `du` of the sync storage, classified.
    pub games: GamesUsage,
}

impl StorageReport {
    /// Bytes used by the collection of the given console (its system code directory).
    pub fn current_collection(&self, console: ConsoleType) -> u64 {
        console
            .system_code()
            .and_then(|code| self.games.collections.get(code))
            .copied()
            .unwrap_or(0)
    }

    /// Bytes used by anything that is neither games nor save states.
    pub fn other_files(&self) -> i64 {
        self.usage.used as i64 - self.games.all as i64 - self.saves as i64
    }

    /// Bytes a fresh sync of the given console's collection may use: free space plus what the
    /// current collection, legacy layouts and extra files occupy (hakchi2 CE
    /// `AvailableForGames` with separate per-console storage).
    pub fn available_for_games(&self, console: ConsoleType) -> u64 {
        self.usage.free
            + self.current_collection(console)
            + self.games.non_multiboot
            + self.games.extra
    }

    /// The eight-line summary hakchi2 CE logs, for the given console.
    pub fn summary(&self, console: ConsoleType) -> String {
        format!(
            "{}\n    Used by all games: {}\n    Used by non multi-boot games: {}\n    Used by \
             current games collection: {}\n    Used by save-states: {}\n    Used by other files \
             (mods, configs, etc.): {}\n    Reserved memory: {}\n    Available for games: {}",
            self.usage,
            megabytes(self.games.all as f64),
            megabytes(self.games.non_multiboot as f64),
            megabytes(self.current_collection(console) as f64),
            megabytes(self.saves as f64),
            megabytes(self.other_files() as f64),
            megabytes(0.0),
            megabytes(self.available_for_games(console) as f64),
        )
    }
}

/// Formats a byte count the way hakchi2 CE logs it: mebibytes with one decimal, `352.6MB`.
pub fn megabytes(bytes: f64) -> String {
    format!("{:.1}MB", bytes / 1024.0 / 1024.0)
}

/// Parses `name='value'` / `name="value"` lines (the `/var/version` and `p0000_config`
/// formats). Backslash escapes inside the quotes are undone; other lines are ignored.
pub fn parse_shell_assignments(contents: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in contents.lines() {
        let Some((name, rest)) = line.trim().split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        let Some(value) = unquote(rest.trim()) else {
            continue;
        };
        values.insert(name.to_string(), value);
    }
    values
}

/// Parses `/etc/preinit.d/p0000_config`: the `cfg_` variables with the prefix removed, as
/// hakchi2 CE's `LoadConfig` returns them.
pub fn parse_config(contents: &str) -> BTreeMap<String, String> {
    parse_shell_assignments(contents)
        .into_iter()
        .filter_map(|(name, value)| {
            name.strip_prefix("cfg_")
                .map(|key| (key.to_string(), value))
        })
        .collect()
}

fn unquote(text: &str) -> Option<String> {
    let quote = text.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let inner = text.strip_prefix(quote)?.strip_suffix(quote)?;
    let mut value = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(escaped) => value.push(escaped),
                None => value.push('\\'),
            }
        } else {
            value.push(c);
        }
    }
    Some(value)
}

/// Serialises config values the way hakchi2 CE's `SyncConfig` feeds them to `hakchi eval`:
/// one `cfg_<key>='<value>'` line per entry, single quotes in values escaped.
pub fn format_config(config: &BTreeMap<String, String>) -> String {
    config
        .iter()
        .map(|(key, value)| format!("cfg_{key}='{}'\n", value.replace('\'', "\\'")))
        .collect()
}

/// Parses `ls "<dir>" -p -1 | grep '/'`: one directory name per line with a trailing slash.
pub fn parse_dir_listing(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|line| line.trim().trim_end_matches('/'))
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parses the `YES` / `NO` answer of `[ -d "<path>" ] && echo YES || echo NO`.
pub fn parse_yes_no(output: &str) -> Option<bool> {
    match output.trim() {
        "YES" => Some(true),
        "NO" => Some(false),
        _ => None,
    }
}

/// Quotes a string for the device's shell (busybox `ash`) with single quotes.
pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Whether a string is a plain shell variable name (`[A-Za-z_][A-Za-z0-9_]*`).
pub fn is_shell_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Checks that a device path can go inside double quotes in a shell command unchanged: no
/// quotes, no `$`, backquote, backslash or control characters.
pub fn check_device_path(path: &str) -> Result<&str> {
    let bad = |c: char| matches!(c, '"' | '\'' | '$' | '`' | '\\') || c.is_control();
    if path.is_empty() || path.contains(bad) {
        return Err(Error::InvalidArgument(format!(
            "device path {path:?} is empty or contains shell metacharacters"
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    // Values from docs/reference/hakchi-ce-sync-trace.log.
    const VERSION_ECHO: &str = "1.0.3 3.4.113.29-madmonkey v1.0.4-126";
    const VERSION_FILE: &str =
        "bootVersion='1.0.3'\nhakchiVersion='v1.0.4-126'\nkernelVersion='3.4.113.29-madmonkey'\n";
    const HWID: &str = "0461872a 87280207 9d2818c7 6c118000";

    #[test]
    fn versions_with_cartload_system() {
        let echo = Versions::parse_echo("1.0.3 3.4.113.30-cartload v1.0.4-126 0.1.0\n").unwrap();
        assert_eq!(echo.system.as_deref(), Some("0.1.0"));
        assert_eq!(
            echo.to_string(),
            "Cartload system 0.1.0, boot 1.0.3, kernel 3.4.113.30-cartload, script v1.0.4-126"
        );
        let file = Versions::parse_file(
            "bootVersion='1.0.3'\nhakchiVersion='v1.0.4-126'\nkernelVersion='3.4.113.30-cartload'\ncartloadVersion='0.1.0'\n",
        )
        .unwrap();
        assert_eq!(file, echo);
        assert!(file.can_interact());
    }

    #[test]
    fn system_codes_round_trip() {
        for (code, console) in SYSTEM_CODES {
            assert_eq!(ConsoleType::from_system_code(code), console);
            assert_eq!(console.system_code(), Some(code));
            assert_ne!(console.family(), ConsoleFamily::Unknown);
            assert_ne!(console.region(), Region::Unknown);
        }
        assert_eq!(
            ConsoleType::from_system_code("snes-eur\n"),
            ConsoleType::SnesEur
        );
        assert_eq!(ConsoleType::from_system_code("-"), ConsoleType::Unknown);
        assert_eq!(ConsoleType::from_system_code(""), ConsoleType::Unknown);
        assert_eq!(ConsoleType::Unknown.system_code(), None);
    }

    #[test]
    fn trace_console_is_a_european_snes() {
        // mDNS TXT: type=snes, region=eur; shell: snes-eur.
        let console = ConsoleType::from_type_and_region("snes", "eur");
        assert_eq!(console, ConsoleType::SnesEur);
        assert_eq!(console.family(), ConsoleFamily::Snes);
        assert_eq!(console.region(), Region::Eur);
        assert_eq!(console.games_squashfs_path(), "/usr/share/games");
        assert!(!console.has_pixel_art());
        assert_eq!(console.to_string(), "SNES Classic Mini (EUR)");
    }

    #[test]
    fn family_and_region_tables() {
        assert_eq!(ConsoleType::Nes.family(), ConsoleFamily::Nes);
        assert_eq!(ConsoleType::ShonenJump.family(), ConsoleFamily::Nes);
        assert_eq!(ConsoleType::ShonenJump.region(), Region::Jpn);
        assert_eq!(ConsoleType::MdAsia.family(), ConsoleFamily::MegaDrive);
        assert_eq!(ConsoleType::MdAsia.region(), Region::Asia);
        assert_eq!(ConsoleType::MdUsa.games_squashfs_path(), "");
        assert_eq!(
            ConsoleType::Famicom.games_squashfs_path(),
            "/usr/share/games/nes/kachikachi"
        );
        assert!(ConsoleType::Famicom.has_pixel_art());
    }

    #[test]
    fn versions_parse_from_echo_and_file() {
        let versions = Versions::parse_echo(VERSION_ECHO).unwrap();
        assert_eq!(versions.boot, "1.0.3");
        assert_eq!(versions.kernel, "3.4.113.29-madmonkey");
        assert_eq!(versions.script, "v1.0.4-126");
        assert_eq!(versions.system, None);
        assert_eq!(Versions::parse_file(VERSION_FILE).unwrap(), versions);
        assert_eq!(
            versions.to_string(),
            "boot 1.0.3, kernel 3.4.113.29-madmonkey, script v1.0.4-126"
        );
        assert!(Versions::parse_echo("1.0.3 3.4.113.29-madmonkey").is_err());
        assert!(Versions::parse_file("bootVersion='1.0.3'\n").is_err());
    }

    #[test]
    fn version_extraction_matches_hakchi2_ce_convert_version() {
        assert_eq!(Version::parse("3.4.113.29-madmonkey").0, [3, 4, 113, 29]);
        assert_eq!(Version::parse("v1.0.4-126").0, [1, 0, 4, 126]);
        assert_eq!(Version::parse("1.0.3").0, [1, 0, 3]);
        assert_eq!(Version::parse("3.4.112-00").0, [3, 4, 112, 0]);
        assert_eq!(Version::parse("v1.0.0-000").0, [1, 0, 0, 0]);
        assert_eq!(Version::parse("1.2.3.4.5").0, [1, 2, 3, 4]);
        assert_eq!(Version::parse("1.2abc3.4.5").0, [3, 4, 5]);
        assert_eq!(Version::parse("garbage").0, [0, 0, 0, 0]);
        assert_eq!(Version::parse("1.2").0, [0, 0, 0, 0]);
        assert_eq!(Version::parse("3.4.113").to_string(), "3.4.113");
        assert!(Version::parse("3.4.113") < Version::parse("3.4.113.29"));
    }

    #[test]
    fn trace_versions_need_neither_reflash_nor_rootfs_update() {
        let versions = Versions::parse_echo(VERSION_ECHO).unwrap();
        assert!(!versions.requires_reflash());
        assert!(!versions.requires_rootfs_update());
        assert!(versions.can_interact());

        let outdated = Versions::outdated_placeholder();
        assert!(outdated.requires_reflash());
        assert!(outdated.requires_rootfs_update());

        let old_script = Versions {
            script: "v1.0.4-117".into(),
            ..versions.clone()
        };
        assert!(!old_script.requires_reflash());
        assert!(old_script.requires_rootfs_update());

        let old_boot = Versions {
            boot: "1.0.1".into(),
            ..versions
        };
        assert!(old_boot.requires_reflash());
    }

    #[test]
    fn firmware_parses() {
        assert_eq!(parse_current_firmware("_nand_\n").unwrap(), Firmware::Nand);
        assert_eq!(
            parse_current_firmware(
                "/var/lib/hakchi/firmware/dp-shvc-release-v2.0.13-0-g9dca6c5-usa.hsqs"
            )
            .unwrap(),
            Firmware::Custom(
                "/var/lib/hakchi/firmware/dp-shvc-release-v2.0.13-0-g9dca6c5-usa.hsqs".into()
            )
        );
        assert!(parse_current_firmware("  \n").is_err());
    }

    #[test]
    fn hwid_drops_spaces() {
        assert_eq!(
            parse_hwid(HWID).unwrap(),
            "0461872a872802079d2818c76c118000"
        );
        assert_eq!(parse_hwid("ABCD\n").unwrap(), "abcd");
        assert!(parse_hwid("").is_err());
        assert!(parse_hwid("no such function: hwid").is_err());
    }

    #[test]
    fn df_summary_reproduces_trace_storage_line() {
        let usage = parse_df_summary("361060 | 28860 | 321060\n").unwrap();
        assert_eq!(
            usage,
            StorageUsage {
                total: 361060 * 1024,
                used: 28860 * 1024,
                free: 321060 * 1024,
            }
        );
        assert_eq!(
            usage.to_string(),
            "Storage size: 352.6MB, used: 28.2MB, free: 313.5MB"
        );
        assert!(parse_df_summary("361060 | 28860").is_err());
        assert!(parse_df_summary("df: /nope: No such file or directory").is_err());
        assert!(parse_df_summary("").is_err());
    }

    #[test]
    fn du_summary_parses_and_classifies() {
        let dirs = parse_du_summary(
            "/var/lib/hakchi/games/snes-eur | 3100\n/var/lib/hakchi/games/.storage | 8\n\
             /var/lib/hakchi/games/000 | 12\n/var/lib/hakchi/games/transfer | 4\n",
        )
        .unwrap();
        assert_eq!(dirs.len(), 4);
        assert_eq!(dirs[0].name(), "snes-eur");
        assert_eq!(dirs[0].bytes, 3100 * 1024);
        let usage = classify_games_usage(&dirs);
        assert_eq!(usage.all, (3100 + 8 + 12 + 4) * 1024);
        assert_eq!(usage.non_multiboot, (8 + 12) * 1024);
        assert_eq!(usage.extra, 4 * 1024);
        assert_eq!(usage.collections.get("snes-eur"), Some(&(3100 * 1024)));
        assert!(parse_du_summary("").unwrap().is_empty());
        assert!(parse_du_summary("\n\n").unwrap().is_empty());
        assert!(parse_du_summary("/var/lib/hakchi/games/x | lots").is_err());
        assert!(parse_du_summary("no separator").is_err());
    }

    #[test]
    fn storage_report_reproduces_trace_summaries() {
        // Before the sync: nothing under the games storage yet.
        let before = StorageReport {
            path: "/var/lib/hakchi/games".into(),
            usage: parse_df_summary("361060 | 28860 | 321060").unwrap(),
            saves: 104 * 1024,
            external_saves: false,
            games: classify_games_usage(&[]),
        };
        assert_eq!(
            before.summary(ConsoleType::SnesEur),
            "Storage size: 352.6MB, used: 28.2MB, free: 313.5MB\n    \
             Used by all games: 0.0MB\n    \
             Used by non multi-boot games: 0.0MB\n    \
             Used by current games collection: 0.0MB\n    \
             Used by save-states: 0.1MB\n    \
             Used by other files (mods, configs, etc.): 28.1MB\n    \
             Reserved memory: 0.0MB\n    \
             Available for games: 313.5MB"
        );
        // After the sync: Chrono Trigger (2.7 MB) landed in snes-eur.
        let after = StorageReport {
            usage: parse_df_summary("361060 | 31960 | 317960").unwrap(),
            games: classify_games_usage(
                &parse_du_summary("/var/lib/hakchi/games/snes-eur | 3100").unwrap(),
            ),
            ..before
        };
        assert_eq!(
            after.summary(ConsoleType::SnesEur),
            "Storage size: 352.6MB, used: 31.2MB, free: 310.5MB\n    \
             Used by all games: 3.0MB\n    \
             Used by non multi-boot games: 0.0MB\n    \
             Used by current games collection: 3.0MB\n    \
             Used by save-states: 0.1MB\n    \
             Used by other files (mods, configs, etc.): 28.1MB\n    \
             Reserved memory: 0.0MB\n    \
             Available for games: 313.5MB"
        );
        assert_eq!(after.current_collection(ConsoleType::Nes), 0);
        assert_eq!(after.available_for_games(ConsoleType::Nes), 317960 * 1024);
    }

    #[test]
    fn shell_assignments_and_config() {
        let values = parse_shell_assignments(
            "# comment\nbootVersion='1.0.3'\nname=\"a \\\"quoted\\\" value\"\nbare=novalue\n\
             odd='it'\\''s'\n",
        );
        assert_eq!(values["bootVersion"], "1.0.3");
        assert_eq!(values["name"], "a \"quoted\" value");
        assert!(!values.contains_key("bare"));
        assert!(!values.contains_key("# comment"));
        let config = parse_config("cfg_cloverapp_home='no'\ncfg_mdui_region=''\nother='x'\n");
        assert_eq!(config.len(), 2);
        assert_eq!(config["cloverapp_home"], "no");
        assert_eq!(config["mdui_region"], "");
        assert_eq!(
            format_config(&config),
            "cfg_cloverapp_home='no'\ncfg_mdui_region=''\n"
        );
        let mut quoted = BTreeMap::new();
        quoted.insert("x".to_string(), "it's".to_string());
        assert_eq!(format_config(&quoted), "cfg_x='it\\'s'\n");
    }

    #[test]
    fn dir_listing_strips_slashes() {
        let games = parse_dir_listing("CLV-P-SAAAE/\nCLV-P-SAABE/\n\nCLV-P-SAAEE/\n");
        assert_eq!(games, ["CLV-P-SAAAE", "CLV-P-SAABE", "CLV-P-SAAEE"]);
        assert!(parse_dir_listing("").is_empty());
    }

    #[test]
    fn yes_no_and_quoting() {
        assert_eq!(parse_yes_no("YES\n"), Some(true));
        assert_eq!(parse_yes_no("NO"), Some(false));
        assert_eq!(parse_yes_no("maybe"), None);
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(
            sh_quote("echo \"$sftype-$sfregion\""),
            "'echo \"$sftype-$sfregion\"'"
        );
        assert!(is_shell_identifier("gamepath"));
        assert!(is_shell_identifier("_cfg_1"));
        assert!(!is_shell_identifier("1abc"));
        assert!(!is_shell_identifier("a b"));
        assert!(!is_shell_identifier(""));
        assert_eq!(
            check_device_path("/var/lib/hakchi/games").unwrap(),
            "/var/lib/hakchi/games"
        );
        assert!(check_device_path("").is_err());
        assert!(check_device_path("/tmp/$(reboot)").is_err());
        assert!(check_device_path("/tmp/a\"b").is_err());
        assert!(check_device_path("/tmp/a\nb").is_err());
    }

    #[test]
    fn megabytes_matches_hakchi2_ce_format() {
        assert_eq!(megabytes(0.0), "0.0MB");
        assert_eq!(megabytes(2_804.0 * 1024.0), "2.7MB");
        assert_eq!(megabytes(-1024.0 * 1024.0), "-1.0MB");
    }
}
