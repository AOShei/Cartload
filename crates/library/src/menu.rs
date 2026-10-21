//! The menu tree the console shows: games, folders and the automatic structures hakchi2 CE
//! offers (`NesMenuCollection.Split`, `NesMenuFolder`). Everything here is pure: a UI can show
//! the resulting tree before anything is synced.
//!
//! On the console each menu is a directory `000`, `001`, ... under the games storage. A
//! folder is an entry with code `CLV-S-<index>` whose launch command is `chmenu <index>`, so
//! the folder code is decided when the tree is exported ([`export`]).

use std::{collections::BTreeSet, str::FromStr};

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

/// Which picture a folder shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// A picture the user chose, kept in the library's `folders` directory under this stem
    /// (`<stem>.png` and `<stem>_small.png`), the same sizes as a game's cover.
    Custom(String),
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
                        image: folder.image.clone(),
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
                        image: link.image.clone(),
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

/// Position of an entry in the tree: the index of each entry on the way down from the root
/// menu. The empty path is the root menu itself.
pub type MenuPath = Vec<usize>;

/// Something to move in the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MenuItem {
    /// A library or firmware game, by code (a code appears once in the tree).
    Game {
        /// Game code.
        code: String,
    },
    /// A folder, by path.
    Folder {
        /// Path of the folder.
        path: MenuPath,
    },
}

fn back_to_parent() -> Entry {
    Entry::Back(BackLink {
        name: names::BACK.to_string(),
        position: Position::Back,
        image: FolderImage::Back,
        target: LinkTarget::Parent,
    })
}

fn no_folder(path: &[usize]) -> Error {
    Error::Invalid(format!("no folder at {path:?}"))
}

/// Editing a tree by hand (the Library page's folder tree). Every operation keeps the tree
/// valid: a game appears once, every folder keeps a back link, the root cannot move.
impl Menu {
    /// The entries of the menu at `path` (the root menu for an empty path), when `path`
    /// leads to a folder.
    pub fn entries_at(&self, path: &[usize]) -> Option<&Vec<Entry>> {
        let mut entries = &self.entries;
        for &index in path {
            match entries.get(index)? {
                Entry::Folder(folder) => entries = &folder.entries,
                _ => return None,
            }
        }
        Some(entries)
    }

    fn entries_at_mut(&mut self, path: &[usize]) -> Option<&mut Vec<Entry>> {
        let mut entries = &mut self.entries;
        for &index in path {
            match entries.get_mut(index)? {
                Entry::Folder(folder) => entries = &mut folder.entries,
                _ => return None,
            }
        }
        Some(entries)
    }

    /// The folder at `path` (never the root, which is not a folder).
    pub fn folder(&self, path: &[usize]) -> Option<&Folder> {
        let (last, parent) = path.split_last()?;
        match self.entries_at(parent)?.get(*last)? {
            Entry::Folder(folder) => Some(folder),
            _ => None,
        }
    }

    /// Where a game or firmware game sits.
    pub fn find(&self, code: &str) -> Option<MenuPath> {
        fn walk(entries: &[Entry], code: &str, path: &mut MenuPath) -> bool {
            for (index, entry) in entries.iter().enumerate() {
                path.push(index);
                match entry {
                    Entry::Game { code: c } | Entry::Original { code: c } if c == code => {
                        return true;
                    }
                    Entry::Folder(folder) if walk(&folder.entries, code, path) => return true,
                    _ => {}
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        walk(&self.entries, code, &mut path).then_some(path)
    }

    /// Number of games (library and firmware) in the menu at `path` and below.
    pub fn game_count(&self, path: &[usize]) -> usize {
        fn count(entries: &[Entry]) -> usize {
            entries
                .iter()
                .map(|e| match e {
                    Entry::Game { .. } | Entry::Original { .. } => 1,
                    Entry::Folder(folder) => count(&folder.entries),
                    Entry::Back(_) => 0,
                })
                .sum()
        }
        self.entries_at(path).map_or(0, |entries| count(entries))
    }

    fn take(&mut self, path: &[usize]) -> Option<Entry> {
        let (last, parent) = path.split_last()?;
        let entries = self.entries_at_mut(parent)?;
        (*last < entries.len()).then(|| entries.remove(*last))
    }

    /// Inserts `entry` into the menu at `folder`: at `index`, or without one after the last
    /// game or folder (before the trailing back link). Returns where it landed.
    fn insert(&mut self, folder: &[usize], index: Option<usize>, entry: Entry) -> Result<usize> {
        let entries = self
            .entries_at_mut(folder)
            .ok_or_else(|| no_folder(folder))?;
        let end = entries
            .iter()
            .rposition(|e| !matches!(e, Entry::Back(_)))
            .map_or(0, |i| i + 1);
        let at = index.map_or(end, |i| i.min(end));
        entries.insert(at, entry);
        Ok(at)
    }

    /// Moves `items` into the menu at `target` (the root for an empty path), at `index` among
    /// its entries or after the last one. A folder cannot be moved into itself; an item inside
    /// another moved folder moves with that folder. Moved items keep their tree order.
    pub fn move_items(
        &mut self,
        items: &[MenuItem],
        target: &[usize],
        index: Option<usize>,
    ) -> Result<()> {
        self.entries_at(target).ok_or_else(|| no_folder(target))?;
        let mut paths: Vec<MenuPath> = Vec::new();
        for item in items {
            let path = match item {
                MenuItem::Game { code } => self
                    .find(code)
                    .ok_or_else(|| Error::Invalid(format!("{code} is not in the menu")))?,
                MenuItem::Folder { path } => {
                    self.folder(path).ok_or_else(|| no_folder(path))?;
                    path.clone()
                }
            };
            if target.starts_with(&path) {
                return Err(Error::Invalid(
                    "a folder cannot be moved into itself".to_string(),
                ));
            }
            paths.push(path);
        }
        paths.sort();
        paths.dedup();
        let roots: Vec<MenuPath> = paths
            .iter()
            .filter(|p| !paths.iter().any(|q| q.len() < p.len() && p.starts_with(q)))
            .cloned()
            .collect();

        // Remove from the last path to the first, so the earlier indices stay valid, and keep
        // the target's coordinates in step with the removals above it.
        let mut target: MenuPath = target.to_vec();
        let mut index = index;
        let mut taken: Vec<Entry> = Vec::new();
        for path in roots.iter().rev() {
            let Some((last, parent)) = path.split_last() else {
                return Err(Error::Invalid("the root menu cannot be moved".to_string()));
            };
            let depth = path.len();
            let entry = self
                .take(path)
                .ok_or_else(|| Error::Invalid(format!("no entry at {path:?}")))?;
            taken.push(entry);
            if target.len() >= depth && target[..depth - 1] == *parent && target[depth - 1] > *last
            {
                target[depth - 1] -= 1;
            } else if target.len() == depth - 1
                && target[..] == *parent
                && let Some(i) = index
                && *last < i
            {
                index = Some(i - 1);
            }
        }
        taken.reverse();
        for (offset, entry) in taken.into_iter().enumerate() {
            self.insert(&target, index.map(|i| i + offset), entry)?;
        }
        Ok(())
    }

    /// Adds an empty folder (with its back link) to the menu at `parent`; returns its path.
    pub fn add_folder(&mut self, parent: &[usize], name: &str) -> Result<MenuPath> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("the folder needs a name".to_string()));
        }
        let folder = Folder {
            name: name.to_string(),
            position: Position::Right,
            image: FolderImage::Folder,
            entries: vec![back_to_parent()],
        };
        let at = self.insert(parent, None, Entry::Folder(folder))?;
        let mut path = parent.to_vec();
        path.push(at);
        Ok(path)
    }

    /// Renames the folder at `path`.
    pub fn rename_folder(&mut self, path: &[usize], name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("the folder needs a name".to_string()));
        }
        let (last, parent) = path.split_last().ok_or_else(|| no_folder(path))?;
        match self.entries_at_mut(parent).and_then(|e| e.get_mut(*last)) {
            Some(Entry::Folder(folder)) => {
                folder.name = name.to_string();
                Ok(())
            }
            _ => Err(no_folder(path)),
        }
    }

    /// Gives the folder at `path` a picture.
    pub fn set_folder_image(&mut self, path: &[usize], image: FolderImage) -> Result<()> {
        let (last, parent) = path.split_last().ok_or_else(|| no_folder(path))?;
        match self.entries_at_mut(parent).and_then(|e| e.get_mut(*last)) {
            Some(Entry::Folder(folder)) => {
                folder.image = image;
                Ok(())
            }
            _ => Err(no_folder(path)),
        }
    }

    /// The picture of the folder at `path`.
    pub fn folder_image(&self, path: &[usize]) -> Option<&FolderImage> {
        let (last, parent) = path.split_last()?;
        match self.entries_at(parent)?.get(*last) {
            Some(Entry::Folder(folder)) => Some(&folder.image),
            _ => None,
        }
    }

    /// Removes the folder at `path`; its games and sub-folders take its place in the parent
    /// menu.
    pub fn remove_folder(&mut self, path: &[usize]) -> Result<()> {
        self.folder(path).ok_or_else(|| no_folder(path))?;
        let (last, parent) = path.split_last().ok_or_else(|| no_folder(path))?;
        let Some(Entry::Folder(folder)) = self.take(path) else {
            return Err(no_folder(path));
        };
        let contents = folder
            .entries
            .into_iter()
            .filter(|e| !matches!(e, Entry::Back(_)));
        let entries = self
            .entries_at_mut(parent)
            .ok_or_else(|| no_folder(parent))?;
        let at = (*last).min(entries.len());
        entries.splice(at..at, contents);
        Ok(())
    }

    /// Brings the tree in line with the library: entries of games that are gone are removed
    /// and games that are not in the tree are added to the root menu. Returns whether
    /// anything changed. Folders left with nothing but a back link stay (the export drops
    /// them).
    pub fn reconcile(&mut self, games: &[GameRef]) -> bool {
        fn prune(entries: &mut Vec<Entry>, known: &BTreeSet<&str>) -> bool {
            let before = entries.len();
            entries.retain(|e| match e {
                Entry::Game { code } | Entry::Original { code } => known.contains(code.as_str()),
                _ => true,
            });
            let mut changed = entries.len() != before;
            for entry in entries.iter_mut() {
                if let Entry::Folder(folder) = entry {
                    changed |= prune(&mut folder.entries, known);
                }
            }
            changed
        }
        let known: BTreeSet<&str> = games.iter().map(|g| g.code.as_str()).collect();
        let mut changed = prune(&mut self.entries, &known);
        let present: BTreeSet<String> = self.codes().into_iter().map(str::to_string).collect();
        let mut missing: Vec<&GameRef> = games
            .iter()
            .filter(|g| !present.contains(&g.code))
            .collect();
        missing.sort_by(|a, b| a.sort_title.cmp(&b.sort_title).then(a.code.cmp(&b.code)));
        for game in missing {
            if self.insert(&[], None, game_entry(game)).is_ok() {
                changed = true;
            }
        }
        changed
    }

    /// Gives every folder without a back link one that leads to its parent menu.
    pub fn ensure_back_links(&mut self) {
        fn walk(entries: &mut [Entry]) {
            for entry in entries {
                if let Entry::Folder(folder) = entry {
                    if !folder.entries.iter().any(|e| matches!(e, Entry::Back(_))) {
                        folder.entries.push(back_to_parent());
                    }
                    walk(&mut folder.entries);
                }
            }
        }
        walk(&mut self.entries);
    }
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

    fn edited_sample() -> Menu {
        let games: Vec<GameRef> = ["Aladdin", "Axelay", "Batman", "Contra", "Cybernator"]
            .iter()
            .enumerate()
            .map(|(i, title)| GameRef {
                code: format!("CLV-U-{:05}", i),
                sort_title: title.to_lowercase(),
                original: false,
            })
            .collect();
        build(
            &games,
            &MenuOptions {
                structure: Structure::Letters,
                originals_at_root: false,
            },
        )
    }

    fn names_at(menu: &Menu, path: &[usize]) -> Vec<String> {
        menu.entries_at(path)
            .expect("folder")
            .iter()
            .map(|e| match e {
                Entry::Game { code } | Entry::Original { code } => code.clone(),
                Entry::Folder(f) => format!("[{}]", f.name),
                Entry::Back(b) => format!("<{}", b.name),
            })
            .collect()
    }

    #[test]
    fn paths_find_and_count() {
        let menu = edited_sample();
        assert_eq!(names_at(&menu, &[]), ["[A]", "[B]", "[C]"]);
        assert_eq!(
            names_at(&menu, &[0]),
            ["CLV-U-00000", "CLV-U-00001", "<Back"]
        );
        assert_eq!(menu.find("CLV-U-00003"), Some(vec![2, 0]));
        assert_eq!(menu.find("CLV-U-99999"), None);
        assert_eq!(menu.folder(&[1]).map(|f| f.name.as_str()), Some("B"));
        assert!(menu.folder(&[]).is_none());
        assert!(menu.folder(&[0, 0]).is_none(), "a game is not a folder");
        assert!(menu.entries_at(&[0, 0]).is_none());
        assert_eq!(menu.game_count(&[]), 5);
        assert_eq!(menu.game_count(&[2]), 2);
    }

    #[test]
    fn move_games_between_folders_and_to_the_root() {
        let mut menu = edited_sample();
        menu.move_items(
            &[MenuItem::Game {
                code: "CLV-U-00000".into(),
            }],
            &[1],
            None,
        )
        .unwrap();
        assert_eq!(names_at(&menu, &[0]), ["CLV-U-00001", "<Back"]);
        assert_eq!(
            names_at(&menu, &[1]),
            ["CLV-U-00002", "CLV-U-00000", "<Back"],
            "lands before the back link"
        );

        // To the root, at a given index, two at once in their original order.
        menu.move_items(
            &[
                MenuItem::Game {
                    code: "CLV-U-00004".into(),
                },
                MenuItem::Game {
                    code: "CLV-U-00003".into(),
                },
            ],
            &[],
            Some(1),
        )
        .unwrap();
        assert_eq!(
            names_at(&menu, &[]),
            ["[A]", "CLV-U-00003", "CLV-U-00004", "[B]", "[C]"],
            "moved items keep their tree order"
        );
        assert_eq!(names_at(&menu, &[4]), ["<Back"]);
        assert_eq!(export(&menu).len(), 3, "the emptied folder is not exported");
    }

    #[test]
    fn move_folders_and_refuse_cycles() {
        let mut menu = edited_sample();
        // C into A: the target's index shifts because C was after A... it was not; B into A:
        menu.move_items(&[MenuItem::Folder { path: vec![1] }], &[0], None)
            .unwrap();
        assert_eq!(names_at(&menu, &[]), ["[A]", "[C]"]);
        assert_eq!(
            names_at(&menu, &[0]),
            ["CLV-U-00000", "CLV-U-00001", "[B]", "<Back"]
        );
        // A folder before the target shifts the target's index: move A into C (C is now [1]).
        menu.move_items(&[MenuItem::Folder { path: vec![0] }], &[1], Some(0))
            .unwrap();
        assert_eq!(names_at(&menu, &[]), ["[C]"]);
        assert_eq!(
            names_at(&menu, &[0]),
            ["[A]", "CLV-U-00003", "CLV-U-00004", "<Back"]
        );
        // Into itself or into its own child: refused, nothing changes.
        let before = menu.clone();
        assert!(matches!(
            menu.move_items(&[MenuItem::Folder { path: vec![0] }], &[0], None),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            menu.move_items(&[MenuItem::Folder { path: vec![0] }], &[0, 0], None),
            Err(Error::Invalid(_))
        ));
        assert!(
            matches!(
                menu.move_items(&[MenuItem::Folder { path: vec![0, 1] }], &[], None),
                Err(Error::Invalid(_))
            ),
            "a game is not a folder"
        );
        assert_eq!(menu, before);
        // A folder and a game inside it: the game moves with the folder, not twice.
        menu.move_items(
            &[
                MenuItem::Folder { path: vec![0, 0] },
                MenuItem::Game {
                    code: "CLV-U-00001".into(),
                },
            ],
            &[],
            Some(0),
        )
        .unwrap();
        assert_eq!(names_at(&menu, &[]), ["[A]", "[C]"]);
        assert_eq!(
            names_at(&menu, &[0]),
            ["CLV-U-00000", "CLV-U-00001", "[B]", "<Back"]
        );
    }

    #[test]
    fn reorder_within_the_same_menu() {
        let mut menu = Menu {
            entries: vec![
                Entry::Game { code: "a".into() },
                Entry::Game { code: "b".into() },
                Entry::Game { code: "c".into() },
            ],
        };
        menu.move_items(&[MenuItem::Game { code: "a".into() }], &[], Some(2))
            .unwrap();
        assert_eq!(names_at(&menu, &[]), ["b", "a", "c"]);
        menu.move_items(&[MenuItem::Game { code: "c".into() }], &[], Some(0))
            .unwrap();
        assert_eq!(names_at(&menu, &[]), ["c", "b", "a"]);
    }

    #[test]
    fn folders_are_added_renamed_and_removed() {
        let mut menu = edited_sample();
        let path = menu.add_folder(&[], "  Favourites ").unwrap();
        assert_eq!(path, vec![3]);
        assert_eq!(names_at(&menu, &[3]), ["<Back"]);
        let nested = menu.add_folder(&[3], "RPG").unwrap();
        assert_eq!(nested, vec![3, 0]);
        assert_eq!(names_at(&menu, &[3]), ["[RPG]", "<Back"]);
        assert!(matches!(menu.add_folder(&[], "  "), Err(Error::Invalid(_))));
        menu.rename_folder(&[3], "Best").unwrap();
        assert_eq!(menu.folder(&[3]).unwrap().name, "Best");
        assert!(matches!(
            menu.rename_folder(&[0, 0], "x"),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            menu.rename_folder(&[], "x"),
            Err(Error::Invalid(_))
        ));
        // Removing C puts its games where the folder was.
        menu.remove_folder(&[2]).unwrap();
        assert_eq!(
            names_at(&menu, &[]),
            ["[A]", "[B]", "CLV-U-00003", "CLV-U-00004", "[Best]"]
        );
        assert!(matches!(menu.remove_folder(&[2]), Err(Error::Invalid(_))));
    }

    #[test]
    fn reconcile_adds_new_games_and_drops_missing_ones() {
        let mut menu = edited_sample();
        let games: Vec<GameRef> = ["CLV-U-00000", "CLV-U-00002", "CLV-U-00009", "CLV-P-SAAAE"]
            .iter()
            .map(|code| GameRef {
                code: (*code).to_string(),
                sort_title: code.to_lowercase(),
                original: code.starts_with("CLV-P"),
            })
            .collect();
        assert!(menu.reconcile(&games));
        assert_eq!(
            names_at(&menu, &[]),
            ["[A]", "[B]", "[C]", "CLV-P-SAAAE", "CLV-U-00009"]
        );
        assert_eq!(names_at(&menu, &[0]), ["CLV-U-00000", "<Back"]);
        assert_eq!(names_at(&menu, &[2]), ["<Back"]);
        assert!(matches!(menu.entries[3], Entry::Original { .. }));
        assert!(!menu.reconcile(&games), "a second pass changes nothing");
    }

    #[test]
    fn back_links_are_added_where_missing() {
        let mut menu = Menu {
            entries: vec![Entry::Folder(Folder {
                name: "X".into(),
                position: Position::Right,
                image: FolderImage::Folder,
                entries: vec![
                    Entry::Game { code: "a".into() },
                    Entry::Folder(Folder {
                        name: "Y".into(),
                        position: Position::Right,
                        image: FolderImage::Folder,
                        entries: vec![Entry::Game { code: "b".into() }],
                    }),
                ],
            })],
        };
        menu.ensure_back_links();
        assert_eq!(names_at(&menu, &[0]), ["a", "[Y]", "<Back"]);
        assert_eq!(names_at(&menu, &[0, 1]), ["b", "<Back"]);
        let again = menu.clone();
        menu.ensure_back_links();
        assert_eq!(menu, again);
    }
}
