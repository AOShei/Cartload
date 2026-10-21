//! The menu tree the console shows: games, folders and the automatic structures hakchi2 CE
//! offers (`NesMenuCollection.Split`, `NesMenuFolder`). Everything here is pure: a UI can show
//! the resulting tree before anything is synced.
//!
//! On the console each menu is a directory `000`, `001`, ... under the games storage. A
//! folder is an entry with code `CLV-S-<index>` whose launch command is `chmenu <index>`, so
//! the folder code is decided when the tree is exported ([`export`]).

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Error, Result, codes, desktop::DesktopEntry};

/// Where a folder sorts in its menu (hakchi2 CE `NesMenuFolder.Priority`). The menu sorts by
/// title, release date, publisher or player count; the folder's fields are set so that it
/// lands at the chosen edge in every sort order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    /// Before everything, used for "back" folders placed on the left.
    LeftBack,
    /// Left edge.
    Leftmost,
    /// Left of the games.
    Left,
    /// Right of the games.
    #[default]
    Right,
    /// Right edge.
    Rightmost,
    /// After everything, the default for "back" folders.
    Back,
}

impl Position {
    /// The sort-title prefix, players, simultaneous flag, release date and publisher the
    /// folder entry gets.
    pub fn sort_fields(self) -> (char, u8, bool, &'static str, String) {
        match self {
            Self::LeftBack => ('\u{1}', 2, true, "0000-00-00", "\u{1}".repeat(10)),
            Self::Leftmost => ('\u{2}', 2, true, "0001-11-11", "\u{2}".repeat(10)),
            Self::Left => ('\u{3}', 2, true, "0002-22-22", "\u{3}".repeat(10)),
            Self::Right => ('Э', 1, false, "7777-77-77", "ZZZZZZZZZX".to_string()),
            Self::Rightmost => ('Ю', 1, false, "8888-88-88", "ZZZZZZZZZY".to_string()),
            Self::Back => ('Я', 1, false, "9999-99-99", "ZZZZZZZZZZ".to_string()),
        }
    }
}

/// Which built-in picture a folder shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderImage {
    /// Plain folder.
    #[default]
    Folder,
    /// Back arrow.
    Back,
    /// Home.
    Home,
    /// `#`, for titles that do not start with a letter.
    Number,
    /// One letter.
    Letter(char),
}

/// A folder with its own sub-menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Title shown in the menu.
    pub name: String,
    /// Sort position.
    #[serde(default)]
    pub position: Position,
    /// Picture.
    #[serde(default)]
    pub image: FolderImage,
    /// What the sub-menu contains.
    #[serde(default)]
    pub entries: Vec<Entry>,
}

/// Where a back link leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkTarget {
    /// The menu that contains the folder this link is in.
    Parent,
    /// The top-level menu.
    Root,
}

/// A folder that leads back to an existing menu instead of opening a new one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackLink {
    /// Title ("Back", "Original games", "Home").
    pub name: String,
    /// Sort position.
    #[serde(default)]
    pub position: Position,
    /// Picture.
    #[serde(default)]
    pub image: FolderImage,
    /// Destination.
    pub target: LinkTarget,
}

/// One item of a menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Entry {
    /// A game from the library, by code.
    Game {
        /// `CLV-U-...` or `CLV-H-...`.
        code: String,
    },
    /// A game built into the firmware, by code.
    Original {
        /// `CLV-P-...`.
        code: String,
    },
    /// A folder opening a sub-menu.
    Folder(Folder),
    /// A link back to the parent or root menu.
    Back(BackLink),
}

/// The whole tree; the root menu is `000` on the console.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Menu {
    /// Entries of the root menu.
    pub entries: Vec<Entry>,
}

impl Menu {
    /// Every game and original code in the tree, in menu order.
    pub fn codes(&self) -> Vec<&str> {
        fn walk<'a>(entries: &'a [Entry], out: &mut Vec<&'a str>) {
            for entry in entries {
                match entry {
                    Entry::Game { code } | Entry::Original { code } => out.push(code),
                    Entry::Folder(folder) => walk(&folder.entries, out),
                    Entry::Back(_) => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.entries, &mut out);
        out
    }
}

/// Default size limit of a menu: hakchi2 CE's `MaxGamesPerFolder`.
pub const DEFAULT_MAX_PER_FOLDER: usize = 30;

/// An automatic structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Structure {
    /// Every game in one menu.
    Flat,
    /// One folder per initial letter (`A`..`Z` and `#`), hakchi2 CE's "folders, alphabetic".
    Letters,
    /// Folders of at most `max` games, named after the first and last title they contain,
    /// hakchi2 CE's "folders, equal". Stays flat while the games fit in one menu.
    Count {
        /// Games per folder.
        max: usize,
    },
}

impl Default for Structure {
    fn default() -> Self {
        Self::Count {
            max: DEFAULT_MAX_PER_FOLDER,
        }
    }
}

impl FromStr for Structure {
    type Err = Error;

    /// `flat`, `letter` (or `letters`), `count:N` (or `count`, 30 per folder).
    fn from_str(text: &str) -> Result<Self> {
        let text = text.trim().to_ascii_lowercase();
        match text.as_str() {
            "flat" => Ok(Self::Flat),
            "letter" | "letters" => Ok(Self::Letters),
            "count" => Ok(Self::default()),
            _ => match text.strip_prefix("count:") {
                Some(n) => n
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n > 0)
                    .map(|max| Self::Count { max })
                    .ok_or_else(|| {
                        Error::InvalidStructure(format!("{text:?}: count needs a positive number"))
                    }),
                None => Err(Error::InvalidStructure(format!(
                    "{text:?}: expected flat, letter or count:N"
                ))),
            },
        }
    }
}

impl std::fmt::Display for Structure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flat => f.write_str("flat"),
            Self::Letters => f.write_str("letter"),
            Self::Count { max } => write!(f, "count:{max}"),
        }
    }
}

/// How to build the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuOptions {
    /// Structure of the games.
    #[serde(default)]
    pub structure: Structure,
    /// Keep the firmware's games in the root menu and put the library's games in a
    /// "More games..." folder (hakchi2 CE's `Original_*` modes). Only applies when there are
    /// originals.
    #[serde(default = "default_true")]
    pub originals_at_root: bool,
}

fn default_true() -> bool {
    true
}

impl Default for MenuOptions {
    fn default() -> Self {
        Self {
            structure: Structure::default(),
            originals_at_root: true,
        }
    }
}

/// What the structuring needs to know about a game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRef {
    /// Game code.
    pub code: String,
    /// The key the menu sorts by.
    pub sort_title: String,
    /// Whether the game is built into the firmware.
    pub original: bool,
}

/// Folder titles hakchi2 CE uses.
pub mod names {
    /// The folder holding the library's games when originals stay in the root menu.
    pub const MORE_GAMES: &str = "More games...";
    /// The link back to the root menu from "More games...".
    pub const ORIGINAL_GAMES: &str = "Original games";
    /// The link back to the previous menu.
    pub const BACK: &str = "Back";
}

/// Builds the tree for `games` (library games and originals mixed) with `options`.
pub fn build(games: &[GameRef], options: &MenuOptions) -> Menu {
    let mut originals: Vec<&GameRef> = games.iter().filter(|g| g.original).collect();
    let mut user: Vec<&GameRef> = games.iter().filter(|g| !g.original).collect();
    originals.sort_by(|a, b| a.sort_title.cmp(&b.sort_title).then(a.code.cmp(&b.code)));
    user.sort_by(|a, b| a.sort_title.cmp(&b.sort_title).then(a.code.cmp(&b.code)));

    if options.originals_at_root && !originals.is_empty() {
        let mut root: Vec<Entry> = originals
            .iter()
            .map(|g| Entry::Original {
                code: g.code.clone(),
            })
            .collect();
        if !user.is_empty() {
            let back = BackLink {
                name: names::ORIGINAL_GAMES.to_string(),
                position: Position::Back,
                image: FolderImage::Back,
                target: LinkTarget::Root,
            };
            let mut more = structure(&user, options.structure, back);
            more.push(Entry::Back(BackLink {
                name: names::ORIGINAL_GAMES.to_string(),
                position: Position::Back,
                image: FolderImage::Back,
                target: LinkTarget::Root,
            }));
            root.push(Entry::Folder(Folder {
                name: names::MORE_GAMES.to_string(),
                position: Position::Rightmost,
                image: FolderImage::Folder,
                entries: more,
            }));
        }
        return Menu { entries: root };
    }

    let mut all: Vec<&GameRef> = originals;
    all.extend(user);
    all.sort_by(|a, b| a.sort_title.cmp(&b.sort_title).then(a.code.cmp(&b.code)));
    let back = BackLink {
        name: names::BACK.to_string(),
        position: Position::Back,
        image: FolderImage::Back,
        target: LinkTarget::Parent,
    };
    Menu {
        entries: structure(&all, options.structure, back),
    }
}

fn game_entry(game: &GameRef) -> Entry {
    if game.original {
        Entry::Original {
            code: game.code.clone(),
        }
    } else {
        Entry::Game {
            code: game.code.clone(),
        }
    }
}

/// Applies a structure to a sorted list of games; sub-menus get a copy of `back`.
fn structure(sorted: &[&GameRef], structure: Structure, back: BackLink) -> Vec<Entry> {
    match structure {
        Structure::Flat => sorted.iter().map(|g| game_entry(g)).collect(),
        Structure::Letters => split_letters(sorted, &back),
        Structure::Count { max } => split_count(sorted, max.max(1), &back),
    }
}

fn split_letters(sorted: &[&GameRef], back: &BackLink) -> Vec<Entry> {
    let mut buckets: Vec<(char, Vec<Entry>)> = ('A'..='Z').map(|c| (c, Vec::new())).collect();
    buckets.push(('#', Vec::new()));
    for game in sorted {
        let initial = game
            .sort_title
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
            .filter(char::is_ascii_uppercase)
            .unwrap_or('#');
        if let Some((_, bucket)) = buckets.iter_mut().find(|(c, _)| *c == initial) {
            bucket.push(game_entry(game));
        }
    }
    buckets
        .into_iter()
        .filter(|(_, entries)| !entries.is_empty())
        .map(|(letter, mut entries)| {
            entries.push(Entry::Back(back.clone()));
            Entry::Folder(Folder {
                name: letter.to_string(),
                position: Position::Right,
                image: if letter == '#' {
                    FolderImage::Number
                } else {
                    FolderImage::Letter(letter)
                },
                entries,
            })
        })
        .collect()
}

fn split_count(sorted: &[&GameRef], max: usize, back: &BackLink) -> Vec<Entry> {
    if sorted.len() <= max {
        return sorted.iter().map(|g| game_entry(g)).collect();
    }
    let parts = sorted.len().div_ceil(max);
    let per_part = sorted.len().div_ceil(parts);
    let mut folders: Vec<(Vec<char>, Vec<char>, Vec<Entry>)> = Vec::new();
    for chunk in sorted.chunks(per_part) {
        let first = folder_name_part(&chunk[0].sort_title);
        let last = folder_name_part(&chunk[chunk.len() - 1].sort_title);
        let mut entries: Vec<Entry> = chunk.iter().map(|g| game_entry(g)).collect();
        entries.push(Entry::Back(back.clone()));
        folders.push((first, last, entries));
    }
    trim_folder_names(&mut folders);
    folders
        .into_iter()
        .map(|(first, last, entries)| {
            Entry::Folder(Folder {
                name: format!(
                    "{} - {}",
                    first.iter().collect::<String>(),
                    last.iter().collect::<String>()
                ),
                position: Position::Right,
                image: FolderImage::Folder,
                entries,
            })
        })
        .collect()
}

/// Upper-case letters and digits of a title (hakchi2 CE strips `[^\p{L}\p{Nd}0-9]`).
fn folder_name_part(title: &str) -> Vec<char> {
    title
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Shortens the first/last names of consecutive folders to the shortest prefixes that still
/// tell them apart (hakchi2 CE `TrimFolderNames`: between 3 and 8 characters).
fn trim_folder_names(folders: &mut [(Vec<char>, Vec<char>, Vec<Entry>)]) {
    const MIN_CHARS: usize = 3;
    const MAX_CHARS: usize = 8;
    let prefix = |chars: &[char], len: usize| chars[..len.min(chars.len())].to_vec();
    for i in 1..folders.len() {
        let mut name_a = folders[i - 1].1.clone();
        let mut name_b = folders[i].0.clone();
        let mut l = (MAX_CHARS - 1).min(name_a.len().max(name_b.len()));
        while prefix(&name_a, l) != prefix(&name_b, l) && l >= MIN_CHARS {
            l -= 1;
        }
        name_a = prefix(&name_a, l + 1);
        name_b = prefix(&name_b, l + 1);
        if name_a == name_b {
            name_a = prefix(&name_a, MIN_CHARS);
            name_b = name_a.clone();
        }
        folders[i - 1].1 = name_a;
        folders[i].0 = name_b;
    }
    if let Some(first) = folders.first_mut() {
        first.0 = prefix(&first.0, MIN_CHARS);
    }
    if let Some(last) = folders.last_mut() {
        last.1 = prefix(&last.1, MIN_CHARS);
    }
}

/// A folder as exported: its code is known.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportedFolder {
    /// `CLV-S-<target index>`.
    pub code: String,
    /// Title.
    pub name: String,
    /// Sort position.
    pub position: Position,
    /// Picture.
    pub image: FolderImage,
    /// Index of the menu the folder opens.
    pub target: usize,
}

impl ExportedFolder {
    /// The launch entry hakchi2 CE writes for a folder (`NesMenuFolder.getAdjustedDesktopFile`).
    pub fn desktop_entry(&self, games_path: &str, profile_path: &str) -> DesktopEntry {
        let (prefix, players, simultaneous, release_date, publisher) = self.position.sort_fields();
        DesktopEntry {
            exec: format!("/bin/chmenu {:03} {games_path}", self.target),
            path: format!("{profile_path}/FOLDER"),
            name: self.name.clone(),
            icon: format!("{games_path}/{}/{}.png", self.code, self.code),
            code: self.code.clone(),
            test_id: 777,
            players,
            simultaneous,
            release_date: release_date.to_string(),
            save_count: 0,
            sort_title: format!("{prefix}{}", self.name.to_lowercase()),
            publisher,
            ..DesktopEntry::new()
        }
    }
}

/// One item of an exported menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExportedEntry {
    /// A library game.
    Game {
        /// Game code.
        code: String,
    },
    /// A firmware game.
    Original {
        /// Game code.
        code: String,
    },
    /// A folder (including back links, which are folders opening an existing menu).
    Folder(ExportedFolder),
}

/// One menu directory on the console.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportedMenu {
    /// `000` for the root, then in depth-first order.
    pub index: usize,
    /// Its entries.
    pub entries: Vec<ExportedEntry>,
}

impl ExportedMenu {
    /// The directory name on the console (`001`).
    pub fn dir_name(&self) -> String {
        format!("{:03}", self.index)
    }
}

/// Assigns menu indices (root first, then depth-first in entry order, as hakchi2 CE's
/// `SyncTask.AddMenu` does) and folder codes. Folders whose sub-menu holds nothing but back
/// links are dropped.
pub fn export(menu: &Menu) -> Vec<ExportedMenu> {
    fn has_content(entries: &[Entry]) -> bool {
        entries.iter().any(|e| !matches!(e, Entry::Back(_)))
    }

    fn walk(
        entries: &[Entry],
        index: usize,
        parent: usize,
        menus: &mut Vec<ExportedMenu>,
        next: &mut usize,
    ) {
        let mut exported = Vec::new();
        let mut children: Vec<(usize, &Folder)> = Vec::new();
        for entry in entries {
            match entry {
                Entry::Game { code } => exported.push(ExportedEntry::Game { code: code.clone() }),
                Entry::Original { code } => {
                    exported.push(ExportedEntry::Original { code: code.clone() });
                }
                Entry::Folder(folder) => {
                    if !has_content(&folder.entries) {
                        continue;
                    }
                    let target = *next;
                    *next += 1;
                    exported.push(ExportedEntry::Folder(ExportedFolder {
                        code: codes::folder_code(target),
                        name: folder.name.clone(),
                        position: folder.position,
                        image: folder.image,
                        target,
                    }));
                    children.push((target, folder));
                }
                Entry::Back(link) => {
                    let target = match link.target {
                        LinkTarget::Parent => parent,
                        LinkTarget::Root => 0,
                    };
                    exported.push(ExportedEntry::Folder(ExportedFolder {
                        code: codes::folder_code(target),
                        name: link.name.clone(),
                        position: link.position,
                        image: link.image,
                        target,
                    }));
                }
            }
        }
        menus.push(ExportedMenu {
            index,
            entries: exported,
        });
        for (target, folder) in children {
            walk(&folder.entries, target, index, menus, next);
        }
    }

    let mut menus = Vec::new();
    let mut next = 1;
    walk(&menu.entries, 0, 0, &mut menus, &mut next);
    menus.sort_by_key(|m| m.index);
    menus
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn game(code: &str, title: &str) -> GameRef {
        GameRef {
            code: code.to_string(),
            sort_title: crate::desktop::sort_title(title),
            original: false,
        }
    }

    fn original(code: &str, title: &str) -> GameRef {
        GameRef {
            original: true,
            ..game(code, title)
        }
    }

    fn folder_names(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|e| match e {
                Entry::Folder(f) => Some(f.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn structures_parse_and_print() {
        assert_eq!("flat".parse::<Structure>().unwrap(), Structure::Flat);
        assert_eq!("Letter".parse::<Structure>().unwrap(), Structure::Letters);
        assert_eq!(
            "count:12".parse::<Structure>().unwrap(),
            Structure::Count { max: 12 }
        );
        assert_eq!(
            "count".parse::<Structure>().unwrap(),
            Structure::Count { max: 30 }
        );
        assert!("count:0".parse::<Structure>().is_err());
        assert!("pages".parse::<Structure>().is_err());
        assert_eq!(Structure::Count { max: 7 }.to_string(), "count:7");
    }

    #[test]
    fn trace_layout_originals_at_root_and_more_games_folder() {
        // The menu of docs/reference/hakchi-ce-sync-trace.log: 21 originals plus Chrono Trigger.
        let mut games: Vec<GameRef> = (0..21)
            .map(|i| original(&format!("CLV-P-SAA{i:02}"), &format!("Original {i:02}")))
            .collect();
        games.push(game("CLV-U-ZJSQU", "Chrono Trigger"));
        let menu = build(&games, &MenuOptions::default());
        assert_eq!(menu.entries.len(), 22);
        assert!(matches!(&menu.entries[0], Entry::Original { code } if code == "CLV-P-SAA00"));
        let Entry::Folder(more) = &menu.entries[21] else {
            panic!("last root entry is the More games folder");
        };
        assert_eq!(more.name, names::MORE_GAMES);
        assert_eq!(more.position, Position::Rightmost);
        assert_eq!(more.entries.len(), 2);
        assert!(matches!(&more.entries[0], Entry::Game { code } if code == "CLV-U-ZJSQU"));
        assert!(
            matches!(&more.entries[1], Entry::Back(link) if link.name == names::ORIGINAL_GAMES)
        );

        let exported = export(&menu);
        assert_eq!(exported.len(), 2);
        assert_eq!(exported[0].index, 0);
        assert_eq!(exported[0].entries.len(), 22);
        let ExportedEntry::Folder(more) = &exported[0].entries[21] else {
            panic!("folder");
        };
        assert_eq!(more.code, "CLV-S-00001");
        assert_eq!(more.target, 1);
        assert_eq!(exported[1].index, 1);
        assert_eq!(exported[1].dir_name(), "001");
        let ExportedEntry::Folder(back) = &exported[1].entries[1] else {
            panic!("back");
        };
        assert_eq!(back.code, "CLV-S-00000");
        assert_eq!(back.target, 0);
        assert_eq!(back.name, names::ORIGINAL_GAMES);
        assert_eq!(menu.codes().len(), 22);
    }

    #[test]
    fn folder_entries_match_what_ce_uploaded() {
        let more = ExportedFolder {
            code: "CLV-S-00001".into(),
            name: names::MORE_GAMES.into(),
            position: Position::Rightmost,
            image: FolderImage::Folder,
            target: 1,
        };
        let text = more
            .desktop_entry("/var/games", "/var/saves")
            .render(crate::desktop::Format::Hakchi);
        assert_eq!(
            text,
            include_str!("../tests/fixtures/ce-000-CLV-S-00001.desktop")
        );

        let back = ExportedFolder {
            code: "CLV-S-00000".into(),
            name: names::ORIGINAL_GAMES.into(),
            position: Position::Back,
            image: FolderImage::Back,
            target: 0,
        };
        let text = back
            .desktop_entry("/var/games", "/var/saves")
            .render(crate::desktop::Format::Hakchi);
        assert_eq!(
            text,
            include_str!("../tests/fixtures/ce-001-CLV-S-00000.desktop")
        );
    }

    #[test]
    fn flat_without_originals_at_root_mixes_everything_sorted() {
        let games = vec![
            game("CLV-U-AAAAB", "Zelda"),
            original("CLV-P-AAAAA", "Mario"),
            game("CLV-U-AAAAC", "The Addams Family"),
        ];
        let options = MenuOptions {
            structure: Structure::Flat,
            originals_at_root: false,
        };
        let menu = build(&games, &options);
        let codes = menu.codes();
        assert_eq!(codes, ["CLV-U-AAAAC", "CLV-P-AAAAA", "CLV-U-AAAAB"]);
        assert_eq!(export(&menu).len(), 1);
    }

    #[test]
    fn letters_make_one_folder_per_initial_with_back_links() {
        let games = vec![
            game("CLV-U-AAAAA", "Aladdin"),
            game("CLV-U-AAAAB", "Actraiser"),
            game("CLV-U-AAAAC", "Zelda"),
            game("CLV-U-AAAAD", "3 Ninjas Kick Back"),
            game("CLV-U-AAAAE", "The Lion King"),
        ];
        let options = MenuOptions {
            structure: Structure::Letters,
            originals_at_root: false,
        };
        let menu = build(&games, &options);
        assert_eq!(folder_names(&menu.entries), ["A", "L", "Z", "#"]);
        let Entry::Folder(a) = &menu.entries[0] else {
            panic!()
        };
        assert_eq!(a.image, FolderImage::Letter('A'));
        assert_eq!(a.entries.len(), 3, "two games and a back link");
        assert!(matches!(&a.entries[0], Entry::Game { code } if code == "CLV-U-AAAAB"));
        assert!(matches!(&a.entries[2], Entry::Back(link) if link.target == LinkTarget::Parent));
        let Entry::Folder(number) = &menu.entries[3] else {
            panic!()
        };
        assert_eq!(number.image, FolderImage::Number);

        let exported = export(&menu);
        assert_eq!(exported.len(), 5);
        assert_eq!(exported[1].entries.len(), 3);
        let ExportedEntry::Folder(back) = &exported[1].entries[2] else {
            panic!()
        };
        assert_eq!(back.target, 0);
        assert_eq!(back.code, "CLV-S-00000");
        let ExportedEntry::Folder(a) = &exported[0].entries[0] else {
            panic!()
        };
        assert_eq!((a.code.as_str(), a.target), ("CLV-S-00001", 1));
    }

    #[test]
    fn count_splits_equally_and_trims_folder_names() {
        let titles = [
            "Actraiser",
            "Aladdin",
            "Axelay",
            "Batman",
            "Breath of Fire",
            "Castlevania",
            "Chrono Trigger",
            "Contra",
            "Donkey Kong",
            "Earthbound",
            "F-Zero",
            "Final Fantasy",
        ];
        let games: Vec<GameRef> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| game(&format!("CLV-U-AAA{i:02}"), t))
            .collect();
        let options = MenuOptions {
            structure: Structure::Count { max: 5 },
            originals_at_root: false,
        };
        let menu = build(&games, &options);
        // 12 games, 5 per folder: 3 folders of 4.
        let names = folder_names(&menu.entries);
        assert_eq!(names.len(), 3);
        assert_eq!(names[0], "ACT - BAT");
        assert_eq!(names[1], "BRE - CON");
        assert_eq!(names[2], "DON - FIN");
        for entry in &menu.entries {
            let Entry::Folder(folder) = entry else {
                panic!()
            };
            assert_eq!(folder.entries.len(), 5, "four games and a back link");
        }

        let few = MenuOptions {
            structure: Structure::Count { max: 30 },
            originals_at_root: false,
        };
        assert!(
            folder_names(&build(&games, &few).entries).is_empty(),
            "fits in one menu"
        );
    }

    #[test]
    fn count_folder_names_grow_to_tell_neighbours_apart() {
        let titles = [
            "Super Bomberman",
            "Super Castlevania IV",
            "Super Ghouls",
            "Super Mario Kart",
            "Super Mario World",
            "Super Metroid",
        ];
        let games: Vec<GameRef> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| game(&format!("CLV-U-AAA{i:02}"), t))
            .collect();
        let options = MenuOptions {
            structure: Structure::Count { max: 3 },
            originals_at_root: false,
        };
        let names = folder_names(&build(&games, &options).entries);
        assert_eq!(names, ["SUP - SUPERG", "SUPERM - SUP"]);
    }

    #[test]
    fn empty_folders_are_dropped_on_export() {
        let menu = Menu {
            entries: vec![
                Entry::Game {
                    code: "CLV-U-AAAAA".into(),
                },
                Entry::Folder(Folder {
                    name: "Empty".into(),
                    position: Position::Right,
                    image: FolderImage::Folder,
                    entries: vec![Entry::Back(BackLink {
                        name: "Back".into(),
                        position: Position::Back,
                        image: FolderImage::Back,
                        target: LinkTarget::Parent,
                    })],
                }),
                Entry::Folder(Folder {
                    name: "Nested".into(),
                    position: Position::Right,
                    image: FolderImage::Folder,
                    entries: vec![Entry::Folder(Folder {
                        name: "Inner".into(),
                        position: Position::Right,
                        image: FolderImage::Folder,
                        entries: vec![Entry::Game {
                            code: "CLV-U-AAAAB".into(),
                        }],
                    })],
                }),
            ],
        };
        let exported = export(&menu);
        assert_eq!(exported.len(), 3);
        assert_eq!(exported[0].entries.len(), 2, "the empty folder is gone");
        let ExportedEntry::Folder(nested) = &exported[0].entries[1] else {
            panic!()
        };
        assert_eq!(nested.target, 1);
        let ExportedEntry::Folder(inner) = &exported[1].entries[0] else {
            panic!()
        };
        assert_eq!((inner.target, inner.code.as_str()), (2, "CLV-S-00002"));
        assert!(
            matches!(&exported[2].entries[0], ExportedEntry::Game { code } if code == "CLV-U-AAAAB")
        );
    }

    #[test]
    fn positions_give_ce_sort_fields() {
        assert_eq!(Position::Right.sort_fields().0, 'Э');
        assert_eq!(Position::Back.sort_fields().3, "9999-99-99");
        assert_eq!(Position::LeftBack.sort_fields().4, "\u{1}".repeat(10));
    }
}
