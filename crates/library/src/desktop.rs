//! `.desktop` launch entries, the files the console's menu reads for every game and folder.
//!
//! Two layouts exist. The stock firmware ships entries with `[Desktop Entry]` and
//! `[X-CLOVER Game]` only ([`Format::Stock`], reproduced byte for byte from
//! `data/desktop_entries/` in hakchi2 CE). hakchi2 CE writes a superset with a `CePrefix` line
//! and `[m2engage]` / `[Description]` sections ([`Format::Hakchi`], hakchi2 CE
//! `DesktopFile.GetConfig`); that is what every synced console carries, so it is what the
//! library uploads. The menu ignores the extra keys.

use serde::{Deserialize, Serialize};

/// One launch entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopEntry {
    /// `Exec`: the command line.
    pub exec: String,
    /// `Path`: the profile (save) directory, verbatim.
    pub path: String,
    /// `Name`: the title shown in the menu.
    pub name: String,
    /// `CePrefix`: hakchi2 CE's own grouping prefix (unused by the console).
    pub ce_prefix: String,
    /// `Icon`: path of the cover image.
    pub icon: String,
    /// `Code`: the game or folder code.
    pub code: String,
    /// `TestID`: 0 for games, 777 for folders.
    pub test_id: u32,
    /// `Status`: present in stock SNES entries (`Completing-3`).
    pub status: Option<String>,
    /// `Players`.
    pub players: u8,
    /// `Simultaneous`.
    pub simultaneous: bool,
    /// `ReleaseDate` (`1995-03-11`).
    pub release_date: String,
    /// `SaveCount`: save slots offered.
    pub save_count: u8,
    /// `SortRawTitle`: the key the menu sorts by.
    pub sort_title: String,
    /// `SortRawPublisher`.
    pub publisher: String,
    /// `Copyright`.
    pub copyright: String,
    /// `MyPlayDemoTime`: present in stock SNES entries (45).
    pub demo_time: Option<u32>,
    /// `[m2engage] regionTag` (Mega Drive).
    pub region_tag: String,
    /// `[m2engage] sortRawGenre`.
    pub genre: String,
    /// `[m2engage] index`.
    pub index: String,
    /// `[m2engage] demo_time`.
    pub m2_demo_time: String,
    /// `[m2engage] country`.
    pub country: String,
    /// `[Description]` text.
    pub description: String,
}

/// Which keys to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// The stock firmware layout.
    Stock,
    /// hakchi2 CE's layout, what a synced console carries.
    Hakchi,
}

impl DesktopEntry {
    /// Defaults hakchi2 CE starts a new entry with.
    pub fn new() -> Self {
        Self {
            players: 1,
            release_date: "1900-01-01".to_string(),
            publisher: "UNKNOWN".to_string(),
            ..Self::default()
        }
    }

    /// Reads an entry (hakchi2 CE `DesktopFile.Load`): keys are matched case-insensitively,
    /// unknown keys are ignored, everything after `[Description]` / `Text = ` is the
    /// description.
    pub fn parse(text: &str) -> Self {
        let mut entry = Self::new();
        let mut description = Vec::new();
        let mut in_description = false;
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if in_description {
                description.push(line);
                continue;
            }
            if line == "[Description]" {
                in_description = true;
                if let Some(next) = lines.next()
                    && next != "Text = "
                {
                    description.push(next);
                }
                continue;
            }
            let Some(pos) = line.find('=') else { continue };
            if pos == 0 {
                continue;
            }
            let key = line[..pos].trim().to_ascii_lowercase();
            // Leading blanks go, trailing ones stay (stock NES entries end `Exec` with a space).
            let value = line[pos + 1..].trim_start().to_string();
            match key.as_str() {
                "exec" => entry.exec = value,
                "path" => entry.path = value,
                "name" => entry.name = value,
                "ceprefix" => entry.ce_prefix = value,
                "icon" => entry.icon = value,
                "code" => entry.code = value,
                "testid" => entry.test_id = value.parse().unwrap_or(0),
                "status" => entry.status = Some(value),
                "players" => entry.players = value.parse().unwrap_or(1),
                "simultaneous" => entry.simultaneous = value.parse::<i64>().unwrap_or(0) != 0,
                "releasedate" => entry.release_date = value,
                "savecount" => entry.save_count = value.parse().unwrap_or(0),
                "sortrawtitle" => entry.sort_title = value,
                "sortrawpublisher" => entry.publisher = value,
                "copyright" => entry.copyright = value,
                "myplaydemotime" => entry.demo_time = value.parse().ok(),
                "regiontag" => entry.region_tag = value,
                "sortrawgenre" => entry.genre = value,
                "index" => entry.index = value,
                "demo_time" => entry.m2_demo_time = value,
                "country" => entry.country = value,
                _ => {}
            }
        }
        entry.description = description.join("\n");
        entry
    }

    /// Writes the entry.
    pub fn render(&self, format: Format) -> String {
        let mut out = String::with_capacity(1024);
        out.push_str("[Desktop Entry]\nType=Application\n");
        out.push_str(&format!("Exec={}\n", self.exec));
        out.push_str(&format!("Path={}\n", self.path));
        out.push_str(&format!("Name={}\n", self.name));
        if format == Format::Hakchi {
            out.push_str(&format!("CePrefix={}\n", self.ce_prefix));
        }
        out.push_str(&format!("Icon={}\n", self.icon));
        out.push_str("\n[X-CLOVER Game]\n");
        out.push_str(&format!("Code={}\n", self.code));
        out.push_str(&format!("TestID={}\n", self.test_id));
        if format == Format::Stock
            && let Some(status) = &self.status
        {
            out.push_str(&format!("Status={status}\n"));
        }
        out.push_str("ID=0\n");
        out.push_str(&format!("Players={}\n", self.players));
        out.push_str(&format!("Simultaneous={}\n", u8::from(self.simultaneous)));
        out.push_str(&format!("ReleaseDate={}\n", self.release_date));
        out.push_str(&format!("SaveCount={}\n", self.save_count));
        out.push_str(&format!("SortRawTitle={}\n", self.sort_title));
        match format {
            Format::Stock => out.push_str(&format!("SortRawPublisher={}\n", self.publisher)),
            Format::Hakchi => out.push_str(&format!(
                "SortRawPublisher={}\n",
                self.publisher.to_uppercase()
            )),
        }
        out.push_str(&format!("Copyright={}\n", self.copyright));
        if format == Format::Stock {
            if let Some(demo_time) = self.demo_time {
                out.push_str(&format!("MyPlayDemoTime={demo_time}\n"));
            }
            return out;
        }
        out.push_str("\n[m2engage]\n");
        out.push_str(&format!("regionTag={}\n", self.region_tag));
        out.push_str(&format!("sortRawGenre={}\n", self.genre));
        out.push_str(&format!("index={}\n", self.index));
        out.push_str(&format!("demo_time={}\n", self.m2_demo_time));
        out.push_str(&format!("country={}\n", self.country));
        out.push_str("\n[Description]\nText = \n");
        out.push_str(self.description.replace("\r\n", "\n").trim());
        out
    }

    /// The executable part of `Exec`: the first word, plus ` -rom` when the emulator takes the
    /// ROM through that switch (hakchi2 CE `DesktopFile.Bin`).
    pub fn binary(&self) -> String {
        let mut words = self.exec.split_whitespace();
        let Some(first) = words.next() else {
            return String::new();
        };
        match words.next() {
            Some("-rom") => format!("{first} -rom"),
            _ => first.to_string(),
        }
    }

    /// The words of `Exec` after [`DesktopEntry::binary`].
    pub fn args(&self) -> Vec<String> {
        let binary_words = self.binary().split_whitespace().count();
        self.exec
            .split_whitespace()
            .skip(binary_words)
            .map(str::to_string)
            .collect()
    }

    /// Rewrites every word of `Exec` and the icon that starts with `from` to start with `to`.
    pub fn relocate(&mut self, from: &str, to: &str) {
        let relocated: Vec<String> = self
            .exec
            .split(' ')
            .map(|word| match word.strip_prefix(from) {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{to}{rest}"),
                _ => word.to_string(),
            })
            .collect();
        self.exec = relocated.join(" ");
        if let Some(rest) = self.icon.strip_prefix(from)
            && (rest.is_empty() || rest.starts_with('/'))
        {
            self.icon = format!("{to}{rest}");
        }
    }

    /// Replaces the executable (first word of `Exec`).
    pub fn set_binary(&mut self, binary: &str) {
        let mut words = self.exec.splitn(2, ' ');
        let _ = words.next();
        self.exec = match words.next() {
            Some(rest) => format!("{binary} {rest}"),
            None => binary.to_string(),
        };
    }
}

/// hakchi2 CE's name fix (`NesApplication.Save`): an apostrophe followed by a digit crashes the
/// console's UI, so it becomes a backtick.
pub fn fix_apostrophes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, c) in chars.iter().enumerate() {
        if *c == '\'' && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
            out.push('`');
        } else {
            out.push(*c);
        }
    }
    out
}

/// The sort key hakchi2 CE derives from a title: lower case, without a leading "the ".
pub fn sort_title(name: &str) -> String {
    let lower = fix_apostrophes(name).to_lowercase();
    match lower.strip_prefix("the ") {
        Some(rest) => rest.to_string(),
        None => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stock entry of the European SNES Classic's Super Mario World
    /// (`data/desktop_entries/snes-eur/CLV-P-SAAAE.desktop` in hakchi2 CE).
    const STOCK_SNES: &str = include_str!("../tests/fixtures/stock-snes-eur-CLV-P-SAAAE.desktop");
    /// Stock entry of the NES Classic's Super Mario Bros.
    /// (`data/desktop_entries/nes-usa/CLV-P-NAAAE.desktop`).
    const STOCK_NES: &str = include_str!("../tests/fixtures/stock-nes-usa-CLV-P-NAAAE.desktop");
    /// What hakchi2 CE uploaded for the same original game (read from the console after the
    /// sync in docs/reference/hakchi-ce-sync-trace.log).
    const CE_ORIGINAL: &str = include_str!("../tests/fixtures/ce-000-CLV-P-SAAAE.desktop");

    #[test]
    fn stock_snes_entry_round_trips_byte_for_byte() {
        let entry = DesktopEntry::parse(STOCK_SNES);
        assert_eq!(entry.code, "CLV-P-SAAAE");
        assert_eq!(entry.name, "Super Mario World");
        assert_eq!(entry.status.as_deref(), Some("Completing-3"));
        assert_eq!(entry.demo_time, Some(45));
        assert_eq!(entry.players, 2);
        assert!(!entry.simultaneous);
        assert_eq!(entry.release_date, "1991-08-18");
        assert_eq!(entry.save_count, 3);
        assert_eq!(entry.publisher, "NINTENDO");
        assert_eq!(entry.copyright, "Super Mario World™ ©1990-1991 Nintendo");
        assert_eq!(entry.binary(), "/bin/clover-canoe-shvc-wr -rom");
        assert_eq!(
            entry.args()[0],
            "/var/squashfs/usr/share/games/CLV-P-SAAAE/CLV-P-SAAAE.sfrom"
        );
        assert_eq!(entry.render(Format::Stock), STOCK_SNES);
    }

    #[test]
    fn stock_nes_entry_round_trips_byte_for_byte() {
        let entry = DesktopEntry::parse(STOCK_NES);
        assert_eq!(entry.code, "CLV-P-NAAAE");
        assert_eq!(entry.test_id, 452);
        assert_eq!(entry.status, None);
        assert_eq!(entry.demo_time, None);
        assert_eq!(entry.binary(), "/bin/clover-kachikachi-wr");
        assert_eq!(entry.render(Format::Stock), STOCK_NES);
    }

    #[test]
    fn hakchi_format_matches_what_ce_uploads_for_an_original_game() {
        let mut entry = DesktopEntry::parse(STOCK_SNES);
        entry.path = "/var/saves/CLV-P-SAAAE".to_string();
        assert_eq!(entry.render(Format::Hakchi), CE_ORIGINAL);
        let reparsed = DesktopEntry::parse(CE_ORIGINAL);
        assert_eq!(reparsed.render(Format::Hakchi), CE_ORIGINAL);
        assert_eq!(reparsed.description, "");
    }

    #[test]
    fn relocation_and_binary_replacement() {
        let mut entry = DesktopEntry::new();
        entry.exec =
            "/usr/bin/clover-canoe-shvc -rom /usr/share/games/CLV-P-SAAAE/CLV-P-SAAAE.sfrom --volume 100"
                .to_string();
        entry.icon = "/usr/share/games/CLV-P-SAAAE/CLV-P-SAAAE.png".to_string();
        entry.relocate("/usr/share/games", "/var/squashfs/usr/share/games");
        entry.set_binary("/bin/clover-canoe-shvc-wr");
        assert_eq!(
            entry.exec,
            "/bin/clover-canoe-shvc-wr -rom /var/squashfs/usr/share/games/CLV-P-SAAAE/CLV-P-SAAAE.sfrom --volume 100"
        );
        assert_eq!(
            entry.icon,
            "/var/squashfs/usr/share/games/CLV-P-SAAAE/CLV-P-SAAAE.png"
        );
        entry.relocate("/usr/share/gamesX", "/nope");
        assert!(entry.exec.contains("/var/squashfs/usr/share/games/"));
    }

    #[test]
    fn description_and_defaults() {
        let text = "[Desktop Entry]\nExec=/bin/x\n\n[Description]\nText = \nline one\nline two";
        let entry = DesktopEntry::parse(text);
        assert_eq!(entry.description, "line one\nline two");
        assert_eq!(entry.players, 1);
        assert_eq!(entry.publisher, "UNKNOWN");
        assert_eq!(entry.release_date, "1900-01-01");
        assert!(
            entry
                .render(Format::Hakchi)
                .ends_with("Text = \nline one\nline two")
        );
        let empty = DesktopEntry::parse("[Description]\nText = ");
        assert_eq!(empty.description, "");
    }

    #[test]
    fn apostrophes_and_sort_titles() {
        assert_eq!(
            fix_apostrophes("Kirby's Dream Land '94"),
            "Kirby's Dream Land `94"
        );
        assert_eq!(sort_title("The Legend of Zelda"), "legend of zelda");
        assert_eq!(sort_title("Chrono Trigger"), "chrono trigger");
        assert_eq!(sort_title("Theme Park"), "theme park");
    }
}
