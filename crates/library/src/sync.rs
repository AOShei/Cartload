//! Building the files for the console, diffing them against what is there, and running the
//! sync in the order hakchi2 CE runs it (`Tasks/SyncTask.cs`, verified in
//! `docs/reference/hakchi-ce-sync-trace.log`): stop the UI, unmount the games, diff, FTP
//! upload, cleanup, link script, config, `overmount_games` and `uistart`.
//!
//! Layout on the console (hakchi2 CE's default "linked" sync, read back from a console CE had
//! synced): under `<sync storage>/<system code>/`, `.storage/<game code>/` holds each game's
//! ROM and art once, and every menu directory `000`, `001`, ... holds one directory per entry
//! with its launch entry (plus the icons for folders). Firmware games get a launch entry
//! pointing into the squashfs and a symlink to their `autoplay` directory, made by the link
//! script.
//!
//! Every change to the console is explicit: [`SyncPlan`] names each file to upload, each file
//! and stale symlink to remove and each directory that ends up empty, so a UI can show the
//! whole plan before anything runs. [`collect`] and [`sync_plan`] are pure; [`sync`] executes
//! a plan over a [`Transport`].

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::PathBuf,
};

use cartload_console::{
    ConsoleInfo, ConsoleType, ExecOutput, Session, Shell,
    parse::{check_device_path, sh_quote},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::menu::FolderImage;
use crate::{
    Error, Result,
    desktop::{DesktopEntry, Format},
    game::{GAMES_PATH, PROFILE_PATH},
    images,
    library::Library,
    menu::{ExportedEntry, ExportedMenu},
    rom::crc32,
};

/// Block size of the console's data partition, used to estimate space.
pub const BLOCK_SIZE: u64 = 4096;
/// Modification times within this many seconds count as equal (FAT precision, as in hakchi2 CE).
pub const MTIME_TOLERANCE: i64 = 3;

/// Device-side paths the sync works with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePaths {
    /// `hakchi findGameSyncStorage`: `/var/lib/hakchi/games` on NAND.
    pub sync_root: String,
    /// `snes-eur`: the per-console directory under the sync root.
    pub system_code: String,
    /// Where the firmware's games are, inside the mounted squashfs
    /// (`/var/squashfs/usr/share/games`).
    pub squashfs_games: String,
    /// The firmware's own games path (`/usr/share/games`), as its launch entries refer to it.
    pub stock_games: String,
    /// Whether the console shows pixel art (NES Classic, Famicom): links `pixelart` too.
    pub has_pixel_art: bool,
}

impl DevicePaths {
    /// Paths of a connected console.
    pub fn from_info(info: &ConsoleInfo) -> Result<Self> {
        let system_code = info
            .system_code
            .clone()
            .filter(|_| info.console != ConsoleType::Unknown)
            .ok_or(Error::UnknownConsole)?;
        let sync_root = info
            .game_sync_path
            .clone()
            .ok_or_else(|| Error::Invalid("the console reported no games storage".into()))?;
        let squashfs = info
            .squashfs_path
            .clone()
            .unwrap_or_else(|| "/var/squashfs".into());
        let stock_games = info
            .original_games_path
            .clone()
            .unwrap_or_else(|| info.console.games_squashfs_path().to_string());
        let paths = Self {
            sync_root,
            system_code,
            squashfs_games: format!("{squashfs}{stock_games}"),
            stock_games,
            has_pixel_art: info.console.has_pixel_art(),
        };
        paths.check()?;
        Ok(paths)
    }

    /// The paths of a console type when none is connected (NAND storage, stock firmware).
    pub fn for_console(console: ConsoleType) -> Result<Self> {
        let system_code = console.system_code().ok_or(Error::UnknownConsole)?;
        let stock_games = console.games_squashfs_path().to_string();
        Ok(Self {
            sync_root: "/var/lib/hakchi/games".to_string(),
            system_code: system_code.to_string(),
            squashfs_games: format!("/var/squashfs{stock_games}"),
            stock_games,
            has_pixel_art: console.has_pixel_art(),
        })
    }

    fn check(&self) -> Result<()> {
        check_device_path(&self.sync_root)?;
        check_device_path(&self.system_code)?;
        check_device_path(&self.squashfs_games)?;
        Ok(())
    }

    /// The directory the files go to (`/var/lib/hakchi/games/snes-eur`).
    pub fn upload_path(&self) -> String {
        format!("{}/{}", self.sync_root, self.system_code)
    }

    /// Where the games' own files live (`<upload path>/.storage`).
    pub fn storage_path(&self) -> String {
        format!("{}/.storage", self.upload_path())
    }
}

/// One file on the console, from the `find`/`stat` listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFile {
    /// Path relative to the upload directory, `./` first.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Modification time as seconds since the epoch, when the line had one.
    pub mtime: Option<i64>,
}

/// Parses the output of the listing command: one `<path> <size> <yyyy-mm-dd> <hh:mm:ss[.frac]>`
/// line per file (busybox `stat -c "%n %s %y"`, which prints no time zone; a trailing
/// `+hhmm` is accepted and ignored). Paths may contain spaces. Lines that do not fit are
/// skipped.
pub fn parse_listing(text: &str) -> Vec<RemoteFile> {
    text.lines().filter_map(parse_listing_line).collect()
}

fn parse_listing_line(line: &str) -> Option<RemoteFile> {
    let line = line.trim_end();
    if line.is_empty() {
        return None;
    }
    let mut words: Vec<&str> = line.rsplitn(5, ' ').collect();
    words.reverse();
    // Drop a time zone word if the server printed one.
    if words.len() == 5 && is_zone(words[4]) {
        words.pop();
    } else if words.len() == 5 {
        // The path itself contained spaces: fold the extra word back into the path.
        let path = format!("{} {}", words[0], words[1]);
        words = vec![
            Box::leak(path.into_boxed_str()),
            words[2],
            words[3],
            words[4],
        ];
    }
    match words.as_slice() {
        [path, size, date, time] => {
            let size = size.parse().ok()?;
            let mtime = parse_stat_time(date, time);
            if mtime.is_none() && !looks_like_date(date) {
                // No time at all: then the last two words were part of the path and size.
                return parse_path_size(line);
            }
            Some(RemoteFile {
                path: (*path).to_string(),
                size,
                mtime,
            })
        }
        _ => parse_path_size(line),
    }
}

fn parse_path_size(line: &str) -> Option<RemoteFile> {
    let (path, size) = line.rsplit_once(' ')?;
    Some(RemoteFile {
        path: path.to_string(),
        size: size.parse().ok()?,
        mtime: None,
    })
}

fn is_zone(word: &str) -> bool {
    word.len() == 5
        && (word.starts_with('+') || word.starts_with('-'))
        && word[1..].chars().all(|c| c.is_ascii_digit())
}

fn looks_like_date(word: &str) -> bool {
    word.len() == 10 && word.as_bytes()[4] == b'-' && word.as_bytes()[7] == b'-'
}

/// Parses the symlink listing (`find -type l`): one path per line.
pub fn parse_link_listing(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_end)
        .filter(|l| l.starts_with("./"))
        .map(str::to_string)
        .collect()
}

/// Where a local file's bytes come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A file in the library.
    Path(PathBuf),
    /// Generated content (launch entries, folder icons).
    Bytes(Vec<u8>),
}

/// One file to have on the console.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFile {
    /// Path relative to the upload directory, `./` first (`./001/CLV-U-ZJSQU/CLV-U-ZJSQU.desktop`).
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Modification time to set on the console, seconds since the epoch.
    pub mtime: i64,
    /// Content.
    pub source: Source,
}

impl LocalFile {
    fn generated(path: String, bytes: Vec<u8>) -> Self {
        Self {
            size: bytes.len() as u64,
            mtime: content_mtime(&bytes),
            path,
            source: Source::Bytes(bytes),
        }
    }

    /// Whether the remote file is the same file: same path and size, modification time within
    /// [`MTIME_TOLERANCE`] (hakchi2 CE `ApplicationFileInfo.Equals`).
    pub fn matches(&self, remote: &RemoteFile) -> bool {
        self.path == remote.path
            && self.size == remote.size
            && remote
                .mtime
                .is_some_and(|m| (m - self.mtime).abs() < MTIME_TOLERANCE)
    }

    /// Absolute path on the console.
    pub fn remote_path(&self, upload_path: &str) -> String {
        format!("{upload_path}/{}", self.path.trim_start_matches("./"))
    }
}

/// A timestamp derived from content (between 2000 and 2020), so generated files keep the
/// same time while their bytes stay the same and change it when they change.
pub fn content_mtime(bytes: &[u8]) -> i64 {
    const YEAR_2000: i64 = 946_684_800;
    const TWENTY_YEARS: i64 = 631_152_000;
    YEAR_2000 + i64::from(crc32(bytes)) % TWENTY_YEARS
}

/// Everything the console should hold for a menu tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Collection {
    /// The files, in menu order.
    pub files: Vec<LocalFile>,
    /// The script that links the firmware games' `autoplay` (and `pixelart`) directories.
    pub link_script: String,
    /// Number of library games.
    pub games: usize,
    /// Number of firmware games.
    pub originals: usize,
    /// Number of folders (including back links).
    pub folders: usize,
    /// Number of library games whose ROM goes up as a 7z archive (they need `tiny7zx` on the
    /// console).
    pub compressed: usize,
}

impl Collection {
    /// Space the files take in 4 KiB blocks.
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| pad(f.size)).sum()
    }
}

fn pad(size: u64) -> u64 {
    size.div_ceil(BLOCK_SIZE).max(1) * BLOCK_SIZE
}

/// Builds the file set for the exported menus.
pub fn collect(
    library: &Library,
    menus: &[ExportedMenu],
    paths: &DevicePaths,
) -> Result<Collection> {
    let storage = paths.storage_path();
    let upload_path = paths.upload_path();
    let originals: BTreeMap<String, (DesktopEntry, i64)> = library
        .originals(&paths.system_code)?
        .into_iter()
        .map(|(code, entry, mtime)| (code, (entry, mtime)))
        .collect();
    let mut collection = Collection::default();
    let mut link_script = "#!/bin/sh\ncd \"/tmp\"\n".to_string();
    let mut stored: BTreeSet<String> = BTreeSet::new();
    for menu in menus {
        let dir = menu.dir_name();
        for entry in &menu.entries {
            match entry {
                ExportedEntry::Game { code } => {
                    let game = library.game(code)?;
                    if stored.insert(code.clone()) {
                        for file in library.game_files(&game)? {
                            collection.files.push(LocalFile {
                                path: format!("./.storage/{code}/{}", file.name),
                                size: file.size,
                                mtime: file.mtime,
                                source: Source::Path(file.path),
                            });
                        }
                        collection.games += 1;
                        if game.compress {
                            collection.compressed += 1;
                        }
                    }
                    let text = game.desktop_entry(&storage).render(Format::Hakchi);
                    collection.files.push(LocalFile::generated(
                        format!("./{dir}/{code}/{}", game.desktop_file()),
                        text.into_bytes(),
                    ));
                }
                ExportedEntry::Original { code } => {
                    let Some((entry, _)) = originals.get(code) else {
                        return Err(Error::Invalid(format!(
                            "no launch entry is cached for the firmware game {code}; connect the \
                             console once so it can be read"
                        )));
                    };
                    let text = original_entry(entry, paths).render(Format::Hakchi);
                    collection.files.push(LocalFile::generated(
                        format!("./{dir}/{code}/{code}.desktop"),
                        text.into_bytes(),
                    ));
                    link_script.push_str(&link_line(paths, &upload_path, &dir, code));
                    collection.originals += 1;
                }
                ExportedEntry::Folder(folder) => {
                    let code = &folder.code;
                    let text = folder
                        .desktop_entry(GAMES_PATH, PROFILE_PATH)
                        .render(Format::Hakchi);
                    collection.files.push(LocalFile::generated(
                        format!("./{dir}/{code}/{code}.desktop"),
                        text.into_bytes(),
                    ));
                    let custom = match &folder.image {
                        FolderImage::Custom(stem) => {
                            let (cover, thumbnail) = library.folder_image_files(stem);
                            match (std::fs::read(&cover), std::fs::read(&thumbnail)) {
                                (Ok(cover), Ok(thumbnail)) => Some((cover, thumbnail)),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    let (icon, thumbnail) = match custom {
                        Some(files) => files,
                        None => (
                            images::folder_icon(&folder.image)?,
                            images::folder_thumbnail(&folder.image)?,
                        ),
                    };
                    collection.files.push(LocalFile::generated(
                        format!("./{dir}/{code}/{code}.png"),
                        icon,
                    ));
                    collection.files.push(LocalFile::generated(
                        format!("./{dir}/{code}/{code}_small.png"),
                        thumbnail,
                    ));
                    collection.folders += 1;
                }
            }
        }
    }
    collection.link_script = link_script;
    Ok(collection)
}

/// The launch entry of a firmware game as uploaded: the hakchi wrapper as emulator, ROM and
/// icon inside the squashfs, the hakchi save directory.
pub fn original_entry(stock: &DesktopEntry, paths: &DevicePaths) -> DesktopEntry {
    let mut entry = stock.clone();
    let binary = entry.binary();
    let first = binary.split(' ').next().unwrap_or("");
    if let Some(name) = first.strip_prefix("/usr/bin/clover-") {
        entry.set_binary(&format!("/bin/clover-{name}-wr"));
    }
    entry.relocate(&paths.stock_games, &paths.squashfs_games);
    entry.path = format!("{PROFILE_PATH}/{}", entry.code);
    entry
}

/// One line of the link script (hakchi2 CE `SyncTask.AddSymLinks`).
fn link_line(paths: &DevicePaths, upload_path: &str, dir: &str, code: &str) -> String {
    let mut line = format!(
        "src=\"{}/{code}\" && dst=\"{upload_path}/{dir}/{code}\" && mkdir -p \"$dst\" && \
         rm -rf \"$dst/autoplay\" && ln -s \"$src/autoplay\" \"$dst/\" ",
        paths.squashfs_games
    );
    if paths.has_pixel_art {
        line.push_str("&& rm -rf \"$dst/pixelart\" && ln -s \"$src/pixelart\" \"$dst/\" ");
    }
    line.push('\n');
    line
}

/// The exact command strings sent to the console.
pub mod commands {
    /// Exit code 0 when the console can extract compressed games: the emulator wrappers call
    /// `tiny7zx` through `PATH`, so this is the check that matters.
    pub const WHICH_EXTRACTOR: &str = "which tiny7zx";
    /// Stops the stock UI (the console's own `uistop` script).
    pub const UI_STOP: &str = "uistop";
    /// Unmounts the games overlay; exits 1 when nothing was mounted.
    pub const UMOUNT_GAMES: &str = r#"hakchi eval 'umount "$gamepath"'"#;
    /// Lists every file under the upload directory with size and modification time.
    pub fn list_files(upload_path: &str) -> String {
        format!(
            r#"mkdir -p "{upload_path}"; cd "{upload_path}"; find . -type f -exec sh -c "stat \"{{}}\" -c \"%n %s %y\"" \;"#
        )
    }
    /// Lists every symlink under the upload directory.
    pub fn list_links(upload_path: &str) -> String {
        format!(r#"cd "{upload_path}" && find . -type l"#)
    }
    /// Writes a script to `/tmp`.
    pub fn write_script(name: &str) -> String {
        format!("cat > /tmp/{name}")
    }
    /// Runs a script in `/tmp`.
    pub fn run_script(name: &str) -> String {
        format!("chmod +x /tmp/{name} && /tmp/{name}")
    }
    /// Removes a script from `/tmp`.
    pub fn remove_script(name: &str) -> String {
        format!("rm /tmp/{name}")
    }
    /// Evaluates `cfg_` assignments from stdin in the hakchi environment.
    pub const EVAL_STDIN: &str = "hakchi eval";
    /// Mounts the games over the stock directory and restarts the UI.
    pub const FINISH: &str = "hakchi overmount_games; uistart";
    /// Name of the deletion script.
    pub const CLEANUP_SCRIPT: &str = "cleanup.sh";
    /// Name of the modification time script.
    pub const MODTIME_SCRIPT: &str = "modtime.sh";
    /// Name of the link script.
    pub const LINK_SCRIPT: &str = "linkgamessync.sh";
    /// A file is uploaded under its name plus this suffix and renamed when complete, so an
    /// interrupted transfer never leaves a truncated game under its final name. Leftovers
    /// show up as stale files and are removed by the next sync.
    pub const PART_SUFFIX: &str = ".cartload-part";
}

/// What a sync will do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPlan {
    /// Device paths.
    pub paths: DevicePaths,
    /// The complete local file set.
    pub collection: Collection,
    /// The remote files the plan was computed against.
    pub remote: Vec<RemoteFile>,
    /// Files to upload (local files with no identical remote file).
    pub uploads: Vec<LocalFile>,
    /// Remote files to remove (not in the collection), relative paths.
    pub deletions: Vec<String>,
    /// Remote symlinks to remove (the link script recreates the needed ones), relative paths.
    pub link_deletions: Vec<String>,
    /// Remote directories that hold nothing once the deletions are done and get no upload,
    /// deepest first.
    pub empty_dirs: Vec<String>,
    /// Local files already on the console.
    pub unchanged: usize,
    /// Bytes available for games on the console, when known.
    pub available: Option<u64>,
    /// `cfg_` variables to write (none by default; the step still runs, as in hakchi2 CE).
    pub config: BTreeMap<String, String>,
}

/// Diffs the collection against the console's listings.
pub fn sync_plan(
    collection: Collection,
    remote: &[RemoteFile],
    remote_links: &[String],
    paths: DevicePaths,
    available: Option<u64>,
) -> SyncPlan {
    let remote_by_path: BTreeMap<&str, &RemoteFile> =
        remote.iter().map(|r| (r.path.as_str(), r)).collect();
    let local_paths: BTreeSet<&str> = collection.files.iter().map(|f| f.path.as_str()).collect();
    let mut uploads = Vec::new();
    let mut unchanged = 0;
    for file in &collection.files {
        match remote_by_path.get(file.path.as_str()) {
            Some(remote) if file.matches(remote) => unchanged += 1,
            _ => uploads.push(file.clone()),
        }
    }
    let deletions: Vec<String> = remote
        .iter()
        .filter(|r| !local_paths.contains(r.path.as_str()))
        .map(|r| r.path.clone())
        .collect();
    let link_deletions: Vec<String> = remote_links.to_vec();

    // Directories known from the listings that keep no file and get none.
    let mut remote_dirs: BTreeSet<String> = BTreeSet::new();
    for path in remote
        .iter()
        .map(|r| r.path.as_str())
        .chain(remote_links.iter().map(String::as_str))
    {
        for dir in parent_dirs(path) {
            remote_dirs.insert(dir);
        }
    }
    let mut kept_dirs: BTreeSet<String> = BTreeSet::new();
    for path in collection.files.iter().map(|f| f.path.as_str()) {
        for dir in parent_dirs(path) {
            kept_dirs.insert(dir);
        }
    }
    let mut empty_dirs: Vec<String> = remote_dirs
        .into_iter()
        .filter(|dir| dir != "." && !kept_dirs.contains(dir))
        .collect();
    // Deepest first so that each removal finds an empty directory.
    empty_dirs.sort_by(|a, b| {
        b.matches('/')
            .count()
            .cmp(&a.matches('/').count())
            .then(a.cmp(b))
    });

    SyncPlan {
        paths,
        collection,
        remote: remote.to_vec(),
        uploads,
        deletions,
        link_deletions,
        empty_dirs,
        unchanged,
        available,
        config: BTreeMap::new(),
    }
}

/// `./a/b/c.ext` gives `./a/b` and `./a`.
fn parent_dirs(path: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut current = path;
    while let Some((parent, _)) = current.rsplit_once('/') {
        if parent == "." || parent.is_empty() {
            break;
        }
        dirs.push(parent.to_string());
        current = parent;
    }
    dirs
}

impl SyncPlan {
    /// Bytes to transfer.
    pub fn upload_size(&self) -> u64 {
        self.uploads.iter().map(|f| f.size).sum()
    }

    /// Space the whole collection takes on the console.
    pub fn total_size(&self) -> u64 {
        self.collection.total_size()
    }

    /// Whether the collection fits in the known free space (true when unknown).
    pub fn fits(&self) -> bool {
        self.available
            .is_none_or(|available| self.total_size() <= available)
    }

    /// Whether the plan has compressed games, which need `tiny7zx` on the console.
    pub fn needs_extractor(&self) -> bool {
        self.collection.compressed > 0
    }

    /// Whether the console already holds everything.
    pub fn is_noop(&self) -> bool {
        self.uploads.is_empty()
            && self.deletions.is_empty()
            && self.link_deletions.is_empty()
            && self.empty_dirs.is_empty()
    }

    /// Remote directories to create before the upload, parents first.
    pub fn directories(&self) -> Vec<String> {
        let upload_path = self.paths.upload_path();
        let mut dirs = BTreeSet::new();
        for file in &self.uploads {
            for dir in parent_dirs(&file.path) {
                dirs.insert(format!("{upload_path}/{}", dir.trim_start_matches("./")));
            }
        }
        dirs.into_iter().collect()
    }

    /// The script removing the stale files, symlinks and directories, by name.
    pub fn cleanup_script(&self) -> String {
        let mut script = format!("#!/bin/sh\ncd \"{}\"\n", self.paths.upload_path());
        for path in self.deletions.iter().chain(&self.link_deletions) {
            script.push_str(&format!("rm {}\n", sh_quote(path)));
        }
        for dir in &self.empty_dirs {
            script.push_str(&format!("rmdir {}\n", sh_quote(dir)));
        }
        script
    }

    /// The script setting the uploaded files' modification times (hakchi2 CE's `modtime.sh`),
    /// so the next diff recognises them.
    pub fn modtime_script(&self) -> String {
        let mut script = format!("#!/bin/sh\ncd \"{}\"\n", self.paths.upload_path());
        for file in &self.uploads {
            script.push_str(&format!(
                "touch -m -t {} {}\n",
                format_touch_time(file.mtime),
                sh_quote(&file.path)
            ));
        }
        script
    }

    /// The `cfg_` assignments for `hakchi eval`.
    pub fn config_script(&self) -> String {
        self.config
            .iter()
            .map(|(key, value)| format!("cfg_{key}='{}'\n", value.replace('\'', "\\'")))
            .collect()
    }

    /// A few lines describing the plan.
    pub fn summary(&self) -> String {
        let mut out = format!(
            "{} games ({} compressed), {} firmware games, {} folders in {} files ({} bytes on the console)\n",
            self.collection.games,
            self.collection.compressed,
            self.collection.originals,
            self.collection.folders,
            self.collection.files.len(),
            self.total_size()
        );
        out.push_str(&format!(
            "upload {} files ({} bytes), keep {}, remove {} files, {} symlinks, {} directories\n",
            self.uploads.len(),
            self.upload_size(),
            self.unchanged,
            self.deletions.len(),
            self.link_deletions.len(),
            self.empty_dirs.len()
        ));
        match self.available {
            Some(available) if self.fits() => {
                out.push_str(&format!("{available} bytes available for games\n"));
            }
            Some(available) => out.push_str(&format!(
                "NOT ENOUGH SPACE: {available} bytes available for games\n"
            )),
            None => out.push_str("free space unknown\n"),
        }
        out
    }

    /// Recomputes the diff against fresh listings.
    pub fn refresh(&self, remote: &[RemoteFile], remote_links: &[String]) -> Self {
        let mut plan = sync_plan(
            self.collection.clone(),
            remote,
            remote_links,
            self.paths.clone(),
            self.available,
        );
        plan.config = self.config.clone();
        plan
    }
}

/// What a sync reports while it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncEvent {
    /// A step begins.
    Step {
        /// 1-based index.
        index: usize,
        /// Number of steps.
        total: usize,
        /// Short name.
        name: &'static str,
        /// What it does.
        detail: String,
    },
    /// Upload progress of one file.
    Upload {
        /// Relative path.
        path: String,
        /// Bytes sent of this file.
        done: u64,
        /// Size of this file.
        total: u64,
        /// Bytes sent overall.
        overall_done: u64,
        /// Bytes to send overall.
        overall_total: u64,
    },
    /// Something worth showing.
    Log(String),
}

/// What a sync did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Files uploaded.
    pub uploaded_files: usize,
    /// Bytes uploaded.
    pub uploaded_bytes: u64,
    /// Files removed.
    pub removed_files: usize,
    /// Symlinks removed.
    pub removed_links: usize,
    /// Directories removed.
    pub removed_dirs: usize,
    /// Whether `overmount_games` reported games.
    pub games_mounted: bool,
}

/// Something that runs commands on the console and uploads files to it. [`Session`] is the
/// real one; tests use scripted stand-ins.
pub trait Transport: Send + Sync {
    /// Runs a shell command with optional stdin.
    fn exec<'a>(
        &'a self,
        command: &'a str,
        stdin: Option<&'a [u8]>,
    ) -> impl Future<Output = Result<ExecOutput>> + Send + 'a;

    /// Creates `directories` (parents first; existing ones are fine) and stores `files` under
    /// `upload_path`, reporting progress. Returns the bytes sent.
    fn upload<'a>(
        &'a self,
        upload_path: &'a str,
        directories: &'a [String],
        files: &'a [LocalFile],
        progress: &'a mut (dyn FnMut(SyncEvent) + Send),
    ) -> impl Future<Output = Result<u64>> + Send + 'a;
}

impl Transport for Session {
    async fn exec(&self, command: &str, stdin: Option<&[u8]>) -> Result<ExecOutput> {
        Ok(Shell::exec(self, command, stdin).await?)
    }

    async fn upload(
        &self,
        upload_path: &str,
        directories: &[String],
        files: &[LocalFile],
        progress: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<u64> {
        {
            let mut ftp = self.ftp().await?;
            for dir in directories {
                ftp.mkdir(dir).await?;
            }
            let overall_total: u64 = files.iter().map(|f| f.size).sum();
            let mut overall_done = 0u64;
            for file in files {
                let remote = file.remote_path(upload_path);
                let part = format!("{remote}{}", commands::PART_SUFFIX);
                let mut report = |done: u64| {
                    progress(SyncEvent::Upload {
                        path: file.path.clone(),
                        done,
                        total: file.size,
                        overall_done: overall_done + done,
                        overall_total,
                    });
                };
                let sent = match &file.source {
                    Source::Path(path) => ftp.upload_file(path, &part, |p| report(p.done)).await?,
                    Source::Bytes(bytes) => ftp.upload(&part, bytes, |p| report(p.done)).await?,
                };
                ftp.rename(&part, &remote).await?;
                overall_done += sent;
            }
            if let Err(error) = ftp.quit().await {
                debug!(%error, "FTP quit failed");
            }
            Ok(overall_done)
        }
    }
}

/// Names of the sync steps, in order.
pub const STEPS: [&str; 9] = [
    "stop-ui", "umount", "diff", "cleanup", "upload", "link", "config", "finish", "done",
];

/// Runs a plan: stops the UI, unmounts the games, re-reads the console's listings and
/// recomputes the diff, removes stale files, uploads the rest over FTP and sets their
/// modification times, runs the link script, writes the config, then `overmount_games` and
/// `uistart`. Refuses to start when the collection does not fit, or when it has compressed
/// games and the console has no `tiny7zx` (nothing is touched in either case).
///
/// A failure after the UI was stopped (a dropped cable, a full flash) is followed by a
/// best-effort `overmount_games; uistart` so the console is not left on a black screen; when
/// the console is already gone it comes back on its own at the next power cycle. Files are
/// uploaded under a temporary name and renamed once complete, so a rerun of the sync keeps
/// what was fully copied and only transfers the rest.
pub async fn sync<T: Transport>(
    target: &T,
    plan: &SyncPlan,
    mut progress: impl FnMut(SyncEvent) + Send,
) -> Result<SyncReport> {
    if !plan.fits() {
        return Err(Error::NotEnoughSpace {
            needed: plan.total_size(),
            available: plan.available.unwrap_or(0),
        });
    }
    if plan.needs_extractor() {
        let which = target.exec(commands::WHICH_EXTRACTOR, None).await?;
        if !which.success() {
            return Err(Error::MissingExtractor {
                games: plan.collection.compressed,
            });
        }
        progress(SyncEvent::Log(format!(
            "{} compressed games; the console extracts them with {}",
            plan.collection.compressed,
            which.stdout_trimmed()
        )));
    }
    match sync_steps(target, plan, &mut progress).await {
        Ok(report) => Ok(report),
        Err(error) => {
            progress(SyncEvent::Log(format!(
                "sync failed ({error}); restarting the console's menu"
            )));
            match target.exec(commands::FINISH, None).await {
                Ok(_) => progress(SyncEvent::Log("console menu restarted".into())),
                Err(recovery) => {
                    debug!(%recovery, "could not restart the menu after the failed sync");
                    progress(SyncEvent::Log(
                        "the console did not answer; it will start normally when powered on \
                         again"
                            .into(),
                    ));
                }
            }
            Err(error)
        }
    }
}

/// The steps of [`sync`], without the recovery.
async fn sync_steps<T: Transport>(
    target: &T,
    plan: &SyncPlan,
    progress: &mut (impl FnMut(SyncEvent) + Send),
) -> Result<SyncReport> {
    let total = STEPS.len();
    let step = |index: usize, detail: String, progress: &mut (dyn FnMut(SyncEvent) + Send)| {
        progress(SyncEvent::Step {
            index: index + 1,
            total,
            name: STEPS[index],
            detail,
        });
    };
    let upload_path = plan.paths.upload_path();
    let mut report = SyncReport::default();

    step(0, "stopping the console's menu".into(), progress);
    ignore_status(target, commands::UI_STOP).await?;

    step(1, "unmounting the games overlay".into(), progress);
    ignore_status(target, commands::UMOUNT_GAMES).await?;

    step(2, format!("listing {upload_path}"), progress);
    let links = target
        .exec(&commands::list_links(&upload_path), None)
        .await?;
    let remote_links = parse_link_listing(&links.stdout_lossy());
    let listing = target
        .exec(&commands::list_files(&upload_path), None)
        .await?;
    let listing = listing.checked(&commands::list_files(&upload_path))?;
    let remote = parse_listing(&listing.stdout_lossy());
    let plan = plan.refresh(&remote, &remote_links);
    progress(SyncEvent::Log(format!(
        "{} files on the console; {} to upload, {} to remove",
        remote.len(),
        plan.uploads.len(),
        plan.deletions.len() + plan.link_deletions.len()
    )));

    step(
        3,
        format!(
            "removing {} files, {} symlinks, {} directories",
            plan.deletions.len(),
            plan.link_deletions.len(),
            plan.empty_dirs.len()
        ),
        progress,
    );
    run_script(
        target,
        commands::CLEANUP_SCRIPT,
        plan.cleanup_script().as_bytes(),
    )
    .await?;
    report.removed_files = plan.deletions.len();
    report.removed_links = plan.link_deletions.len();
    report.removed_dirs = plan.empty_dirs.len();

    step(
        4,
        format!(
            "uploading {} files ({} bytes)",
            plan.uploads.len(),
            plan.upload_size()
        ),
        progress,
    );
    if plan.uploads.is_empty() {
        progress(SyncEvent::Log("nothing to upload".into()));
    } else {
        let directories = plan.directories();
        report.uploaded_bytes = target
            .upload(&upload_path, &directories, &plan.uploads, progress)
            .await?;
        report.uploaded_files = plan.uploads.len();
        run_script(
            target,
            commands::MODTIME_SCRIPT,
            plan.modtime_script().as_bytes(),
        )
        .await?;
    }

    step(
        5,
        format!("linking {} firmware games", plan.collection.originals),
        progress,
    );
    run_script(
        target,
        commands::LINK_SCRIPT,
        plan.collection.link_script.as_bytes(),
    )
    .await?;

    step(
        6,
        format!("writing {} config values", plan.config.len()),
        progress,
    );
    target
        .exec(commands::EVAL_STDIN, Some(plan.config_script().as_bytes()))
        .await?
        .checked(commands::EVAL_STDIN)?;

    step(
        7,
        "mounting the games and restarting the menu".into(),
        progress,
    );
    let finish = target.exec(commands::FINISH, None).await?;
    report.games_mounted = finish.success();
    if !finish.success() {
        warn!(exit_code = ?finish.exit_code, "overmount_games reported no games");
    }

    step(8, "sync finished".into(), progress);
    info!(
        uploaded = report.uploaded_files,
        bytes = report.uploaded_bytes,
        removed = report.removed_files,
        "sync finished"
    );
    Ok(report)
}

async fn ignore_status<T: Transport>(target: &T, command: &str) -> Result<()> {
    let output = target.exec(command, None).await?;
    if !output.success() {
        debug!(command, exit_code = ?output.exit_code, "ignored exit status");
    }
    Ok(())
}

/// hakchi2 CE's `RunTemporaryScript`: write to `/tmp`, run, remove.
async fn run_script<T: Transport>(target: &T, name: &str, script: &[u8]) -> Result<()> {
    let write = commands::write_script(name);
    target.exec(&write, Some(script)).await?.checked(&write)?;
    let run = commands::run_script(name);
    let result = target.exec(&run, None).await;
    let remove = commands::remove_script(name);
    if let Err(error) = target.exec(&remove, None).await {
        debug!(%error, "could not remove the temporary script");
    }
    let output = result?;
    if !output.success() {
        warn!(name, exit_code = ?output.exit_code, stderr = %output.stderr_trimmed(), "script reported a failure");
    }
    Ok(())
}

/// Reads the firmware's launch entries from the console: `(code, file content)` pairs.
pub async fn fetch_originals<S: Shell>(
    shell: &S,
    squashfs_games: &str,
) -> Result<Vec<(String, String)>> {
    check_device_path(squashfs_games)?;
    let command = format!(
        r#"for d in "{squashfs_games}"/CLV-P-*; do c="${{d##*/}}"; echo "==== $c"; cat "$d/$c.desktop"; echo; done"#
    );
    let output = shell.exec(&command, None).await?.checked(&command)?;
    Ok(parse_original_blocks(&output.stdout_lossy()))
}

/// Reads firmware art files of stock games from the console, read-only, in one command:
/// `files` are `<code>.png` or `<code>_small.png` names, found under `<squashfs_games>/<code>/`.
/// Answers `(file, bytes)` pairs, the bytes empty when the console has no such file. Nothing
/// is run when `files` is empty.
pub async fn fetch_original_art<S: Shell>(
    shell: &S,
    squashfs_games: &str,
    files: &[String],
) -> Result<Vec<(String, Vec<u8>)>> {
    check_device_path(squashfs_games)?;
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        let code = crate::codes::original_art_code(file)
            .ok_or_else(|| Error::InvalidCode(file.clone()))?;
        paths.push(format!("{code}/{file}"));
    }
    let command = art_command(squashfs_games, &paths);
    let output = shell.exec(&command, None).await?.checked(&command)?;
    Ok(parse_art_blocks(&output.stdout))
}

/// The command that prints each of `paths` (relative to `squashfs_games`) after a
/// `==== <path> <size>` line; a missing file is announced with size 0.
fn art_command(squashfs_games: &str, paths: &[String]) -> String {
    format!(
        r#"cd "{squashfs_games}" || exit 1; for f in {}; do echo "==== $f $(stat -c %s "$f" 2>/dev/null || echo 0)"; cat "$f" 2>/dev/null; done; :"#,
        paths.join(" ")
    )
}

/// Splits the output of the art command into `(file name, bytes)` pairs: a `==== <path>
/// <size>` line announces each file and exactly `size` bytes follow. A file cut short by the
/// end of the output is dropped.
pub fn parse_art_blocks(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut blocks = Vec::new();
    let mut rest = bytes;
    while let Some(end) = rest.iter().position(|&b| b == b'\n') {
        let line = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        let Some(header) = line.strip_prefix("==== ") else {
            continue;
        };
        let Some((path, size)) = header.rsplit_once(' ') else {
            continue;
        };
        let size: usize = size.trim().parse().unwrap_or(0);
        if size > rest.len() {
            break;
        }
        let file = path.rsplit_once('/').map_or(path, |(_, file)| file);
        blocks.push((file.to_string(), rest[..size].to_vec()));
        rest = &rest[size..];
    }
    blocks
}

/// Splits the output of the originals command into `(code, text)` pairs.
pub fn parse_original_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.split_inclusive('\n') {
        if let Some(code) = line.strip_prefix("==== ") {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            let code = code.trim().to_string();
            if crate::codes::is_original(&code) {
                current = Some((code, String::new()));
            }
            continue;
        }
        if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
        }
    }
    if let Some(block) = current.take() {
        blocks.push(block);
    }
    for (_, body) in &mut blocks {
        // The command echoes one extra newline after each file.
        if body.ends_with("\n\n") {
            body.pop();
        }
    }
    blocks
}

// --- time ---

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let d = i64::from(day);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date of a day count since 1970-01-01.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    (year, m as u32, d as u32)
}

/// Seconds since the epoch of `yyyy-mm-dd` and `hh:mm:ss[.fraction]` (UTC, the console's clock).
pub fn parse_stat_time(date: &str, time: &str) -> Option<i64> {
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let time = time.split('.').next()?;
    let mut parts = time.split(':');
    let hour: i64 = parts.next()?.parse().ok()?;
    let minute: i64 = parts.next()?.parse().ok()?;
    let second: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

fn civil(seconds: i64) -> (i64, u32, u32, i64, i64, i64) {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    (year, month, day, rest / 3600, rest % 3600 / 60, rest % 60)
}

/// `yyyy-mm-dd hh:mm:ss`, the listing format.
pub fn format_stat_time(seconds: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(seconds);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// `yyyymmddHHMM.SS`, what `touch -t` takes.
pub fn format_touch_time(seconds: i64) -> String {
    let (y, mo, d, h, mi, s) = civil(seconds);
    format!("{y:04}{mo:02}{d:02}{h:02}{mi:02}.{s:02}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{collections::VecDeque, sync::Mutex};

    use super::*;
    use crate::{
        library::LibraryConfig,
        menu::{self, GameRef, MenuOptions, Structure},
        rom::{self, tests::synthetic_snes},
    };

    /// The listing of the console after the sync in the trace (read back in this task).
    const LISTING: &str = include_str!("../tests/fixtures/remote-listing.txt");

    #[test]
    fn time_conversions() {
        assert_eq!(
            parse_stat_time("2026-10-21", "09:01:26.000000000"),
            Some(1_792_573_286)
        );
        assert_eq!(
            parse_stat_time("2026-10-20", "22:41:04"),
            Some(1_792_536_064)
        );
        assert_eq!(parse_stat_time("1970-01-01", "00:01:18"), Some(78));
        assert_eq!(parse_stat_time("1999-12-31", "23:59:59"), Some(946_684_799));
        assert_eq!(
            parse_stat_time("2100-02-28", "12:00:00"),
            Some(4_107_499_200)
        );
        assert_eq!(parse_stat_time("2026-13-01", "00:00:00"), None);
        assert_eq!(parse_stat_time("garbage", "00:00:00"), None);
        assert_eq!(format_stat_time(1_792_573_286), "2026-10-21 09:01:26");
        assert_eq!(format_touch_time(1_792_573_286), "202610210901.26");
        assert_eq!(format_stat_time(0), "1970-01-01 00:00:00");
        assert_eq!(civil_from_days(days_from_civil(2000, 2, 29)), (2000, 2, 29));
        let t = content_mtime(b"abc");
        assert!((946_684_800..946_684_800 + 631_152_000).contains(&t));
        assert_eq!(t, content_mtime(b"abc"));
        assert_ne!(t, content_mtime(b"abd"));
    }

    #[test]
    fn listing_from_the_console_parses() {
        let files = parse_listing(LISTING);
        assert_eq!(files.len(), 35);
        assert_eq!(
            files[0],
            RemoteFile {
                path: "./001/CLV-U-ZJSQU/CLV-U-ZJSQU.desktop".into(),
                size: 583,
                mtime: Some(1_792_573_286),
            }
        );
        let rom = files.iter().find(|f| f.path.ends_with(".7z")).unwrap();
        assert_eq!(rom.size, 2_850_560);
        assert_eq!(rom.path, "./.storage/CLV-U-ZJSQU/Chrono_Trigger.sfrom.7z");
        assert_eq!(
            files
                .iter()
                .filter(|f| f.path.starts_with("./000/CLV-P-"))
                .count(),
            21
        );

        let odd = parse_listing(
            "./dir with space/file name.png 12 2026-01-02 03:04:05.000 +0000\n./x 5\n\nnot a line\n./y 7 2026-01-01 00:00:00\n",
        );
        assert_eq!(odd.len(), 3);
        assert_eq!(odd[0].path, "./dir with space/file name.png");
        assert_eq!(odd[0].size, 12);
        assert_eq!(odd[0].mtime, parse_stat_time("2026-01-02", "03:04:05"));
        assert_eq!(
            odd[1],
            RemoteFile {
                path: "./x".into(),
                size: 5,
                mtime: None
            }
        );
        assert_eq!(odd[2].path, "./y");
        assert_eq!(
            parse_link_listing("./000/CLV-P-SAAAE/autoplay\n./000/CLV-P-SAABE/autoplay\n"),
            ["./000/CLV-P-SAAAE/autoplay", "./000/CLV-P-SAABE/autoplay"]
        );
    }

    #[test]
    fn device_paths() {
        let paths = DevicePaths::for_console(ConsoleType::SnesEur).unwrap();
        assert_eq!(paths.upload_path(), "/var/lib/hakchi/games/snes-eur");
        assert_eq!(
            paths.storage_path(),
            "/var/lib/hakchi/games/snes-eur/.storage"
        );
        assert_eq!(paths.squashfs_games, "/var/squashfs/usr/share/games");
        assert!(!paths.has_pixel_art);
        let nes = DevicePaths::for_console(ConsoleType::Nes).unwrap();
        assert_eq!(
            nes.squashfs_games,
            "/var/squashfs/usr/share/games/nes/kachikachi"
        );
        assert!(nes.has_pixel_art);
        assert!(matches!(
            DevicePaths::for_console(ConsoleType::Unknown),
            Err(Error::UnknownConsole)
        ));

        let info = ConsoleInfo {
            console: ConsoleType::SnesEur,
            system_code: Some("snes-eur".into()),
            game_sync_path: Some("/media/hakchi/games".into()),
            squashfs_path: Some("/var/squashfs".into()),
            original_games_path: Some("/usr/share/games".into()),
            ..ConsoleInfo::default()
        };
        let paths = DevicePaths::from_info(&info).unwrap();
        assert_eq!(paths.upload_path(), "/media/hakchi/games/snes-eur");
        assert_eq!(paths.stock_games, "/usr/share/games");
        assert!(matches!(
            DevicePaths::from_info(&ConsoleInfo::default()),
            Err(Error::UnknownConsole)
        ));
    }

    #[test]
    fn commands_match_the_trace() {
        assert_eq!(
            commands::UMOUNT_GAMES,
            r#"hakchi eval 'umount "$gamepath"'"#
        );
        assert_eq!(
            commands::list_files("/var/lib/hakchi/games/snes-eur"),
            r#"mkdir -p "/var/lib/hakchi/games/snes-eur"; cd "/var/lib/hakchi/games/snes-eur"; find . -type f -exec sh -c "stat \"{}\" -c \"%n %s %y\"" \;"#
        );
        assert_eq!(commands::FINISH, "hakchi overmount_games; uistart");
        assert_eq!(
            commands::write_script("cleanup.sh"),
            "cat > /tmp/cleanup.sh"
        );
        assert_eq!(
            commands::run_script("cleanup.sh"),
            "chmod +x /tmp/cleanup.sh && /tmp/cleanup.sh"
        );
        assert_eq!(commands::remove_script("cleanup.sh"), "rm /tmp/cleanup.sh");
    }

    /// A library with Chrono Trigger (synthetic ROM) and the cached stock entry of Super
    /// Mario World, with the trace's menu (originals at root, "More games..." folder).
    fn trace_library() -> (tempfile::TempDir, Library, Vec<ExportedMenu>, DevicePaths) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path().join("lib")).unwrap();
        let rom = synthetic_snes("CHRONO TRIGGER", rom::snes::RomType::HiRom, 0x02, 0x03);
        let imported = library
            .import_bytes("Chrono Trigger.smc", rom, None)
            .unwrap();
        let stock = include_str!("../tests/fixtures/stock-snes-eur-CLV-P-SAAAE.desktop");
        library
            .store_originals(
                "snes-eur",
                &[("CLV-P-SAAAE".to_string(), stock.to_string())],
            )
            .unwrap();
        let games = vec![
            GameRef {
                code: imported.game.code.clone(),
                sort_title: imported.game.sort_title(),
                original: false,
            },
            GameRef {
                code: "CLV-P-SAAAE".into(),
                sort_title: "super mario world".into(),
                original: true,
            },
        ];
        let menu = menu::build(&games, &MenuOptions::default());
        let menus = menu::export(&menu);
        let paths = DevicePaths::for_console(ConsoleType::SnesEur).unwrap();
        (dir, library, menus, paths)
    }

    #[test]
    fn collect_builds_the_linked_layout() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        let code = library.games().unwrap()[0].code.clone();
        let paths_list: Vec<&str> = collection.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths_list,
            [
                "./000/CLV-P-SAAAE/CLV-P-SAAAE.desktop".to_string(),
                "./000/CLV-S-00001/CLV-S-00001.desktop".to_string(),
                "./000/CLV-S-00001/CLV-S-00001.png".to_string(),
                "./000/CLV-S-00001/CLV-S-00001_small.png".to_string(),
                format!("./.storage/{code}/{code}.png"),
                format!("./.storage/{code}/{code}_small.png"),
                format!("./.storage/{code}/Chrono_Trigger.sfrom.7z"),
                format!("./001/{code}/{code}.desktop"),
                "./001/CLV-S-00000/CLV-S-00000.desktop".to_string(),
                "./001/CLV-S-00000/CLV-S-00000.png".to_string(),
                "./001/CLV-S-00000/CLV-S-00000_small.png".to_string(),
            ]
        );
        assert_eq!(
            (
                collection.games,
                collection.originals,
                collection.folders,
                collection.compressed
            ),
            (1, 1, 2, 1)
        );
        let archive = &collection.files[6];
        let Source::Path(path) = &archive.source else {
            panic!()
        };
        assert!(path.ends_with("Chrono_Trigger.sfrom.7z"));
        assert_eq!(archive.size, std::fs::metadata(path).unwrap().len());
        assert!(
            archive.size < 131_072,
            "a 128 KiB synthetic ROM packs small"
        );

        let original = &collection.files[0];
        let Source::Bytes(bytes) = &original.source else {
            panic!()
        };
        assert_eq!(
            std::str::from_utf8(bytes).unwrap(),
            include_str!("../tests/fixtures/ce-000-CLV-P-SAAAE.desktop"),
            "the firmware game's entry is what CE uploads"
        );
        let more = &collection.files[1];
        let Source::Bytes(bytes) = &more.source else {
            panic!()
        };
        assert_eq!(
            std::str::from_utf8(bytes).unwrap(),
            include_str!("../tests/fixtures/ce-000-CLV-S-00001.desktop")
        );
        let game_entry = &collection.files[7];
        let Source::Bytes(bytes) = &game_entry.source else {
            panic!()
        };
        let text = std::str::from_utf8(bytes).unwrap();
        assert!(text.contains(&format!(
            "Exec=/bin/clover-canoe-shvc-wr -rom /var/lib/hakchi/games/snes-eur/.storage/{code}/Chrono_Trigger.sfrom.7z --volume 100 -rollback-snapshot-period 600\n"
        )));
        assert!(text.contains(&format!(
            "Icon=/var/lib/hakchi/games/snes-eur/.storage/{code}/{code}.png\n"
        )));
        assert_eq!(
            collection.link_script,
            "#!/bin/sh\ncd \"/tmp\"\nsrc=\"/var/squashfs/usr/share/games/CLV-P-SAAAE\" && dst=\"/var/lib/hakchi/games/snes-eur/000/CLV-P-SAAAE\" && mkdir -p \"$dst\" && rm -rf \"$dst/autoplay\" && ln -s \"$src/autoplay\" \"$dst/\" \n"
        );
        assert!(collection.total_size() >= collection.files.iter().map(|f| f.size).sum::<u64>());
        assert_eq!(pad(1), 4096);
        assert_eq!(pad(4096), 4096);
        assert_eq!(pad(4097), 8192);
    }

    #[test]
    fn nes_link_lines_include_pixel_art() {
        let paths = DevicePaths::for_console(ConsoleType::Nes).unwrap();
        let line = link_line(&paths, &paths.upload_path(), "000", "CLV-P-NAAAE");
        assert!(
            line.ends_with("&& rm -rf \"$dst/pixelart\" && ln -s \"$src/pixelart\" \"$dst/\" \n")
        );
        assert!(line.starts_with("src=\"/var/squashfs/usr/share/games/nes/kachikachi/CLV-P-NAAAE\" && dst=\"/var/lib/hakchi/games/nes-usa/000/CLV-P-NAAAE\""));
    }

    #[test]
    fn plan_against_the_console_listing_from_the_trace() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        let remote = parse_listing(LISTING);
        let links: Vec<String> = (0..21)
            .map(|i| format!("./000/CLV-P-SAA{i:02}/autoplay"))
            .collect();
        let plan = sync_plan(
            collection.clone(),
            &remote,
            &links,
            paths.clone(),
            Some(313 * 1024 * 1024),
        );
        // Nothing on the console matches: CE compressed the ROM and used other codes/dates.
        assert_eq!(plan.uploads.len(), collection.files.len());
        assert_eq!(plan.unchanged, 0);
        // Seven remote paths (the SMW entry and the two folders) exist locally too; they are
        // uploaded again, not deleted.
        assert_eq!(plan.deletions.len(), 28);
        assert!(
            plan.deletions
                .contains(&"./.storage/CLV-U-ZJSQU/Chrono_Trigger.sfrom.7z".to_string())
        );
        assert_eq!(plan.link_deletions.len(), 21);
        assert!(plan.empty_dirs.contains(&"./000/CLV-P-SADJE".to_string()));
        assert!(
            !plan.empty_dirs.contains(&"./000/CLV-P-SAAAE".to_string()),
            "gets a new entry"
        );
        assert!(!plan.empty_dirs.contains(&"./000/CLV-S-00001".to_string()));
        assert!(
            plan.empty_dirs
                .iter()
                .position(|d| d == "./.storage/CLV-U-ZJSQU")
                .unwrap()
                < plan
                    .empty_dirs
                    .iter()
                    .position(|d| d == "./.storage")
                    .unwrap_or(usize::MAX),
            "deepest first"
        );
        assert!(plan.fits());
        assert!(!plan.is_noop());
        assert!(plan.needs_extractor());
        assert!(
            plan.summary()
                .starts_with("1 games (1 compressed), 1 firmware games")
        );
        let script = plan.cleanup_script();
        assert!(script.starts_with("#!/bin/sh\ncd \"/var/lib/hakchi/games/snes-eur\"\nrm '"));
        assert!(script.contains("rm './.storage/CLV-U-ZJSQU/metadata.json'\n"));
        assert!(script.contains("rm './000/CLV-P-SAA00/autoplay'\n"));
        assert!(script.contains("rmdir './000/CLV-P-SADJE'\n"));
        let modtime = plan.modtime_script();
        assert!(modtime.contains("touch -m -t "));
        assert!(modtime.contains(" './000/CLV-S-00001/CLV-S-00001.desktop'\n"));
        let dirs = plan.directories();
        assert_eq!(dirs[0], "/var/lib/hakchi/games/snes-eur/.storage");
        assert!(dirs.contains(&"/var/lib/hakchi/games/snes-eur/000/CLV-S-00001".to_string()));
        assert!(plan.summary().contains("upload 11 files"));
    }

    #[test]
    fn plan_is_a_noop_when_the_console_matches() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        let remote: Vec<RemoteFile> = collection
            .files
            .iter()
            .map(|f| RemoteFile {
                path: f.path.clone(),
                size: f.size,
                mtime: Some(f.mtime + 2),
            })
            .collect();
        let plan = sync_plan(collection.clone(), &remote, &[], paths.clone(), None);
        assert!(plan.is_noop());
        assert_eq!(plan.unchanged, collection.files.len());
        assert!(plan.fits(), "unknown free space does not block");
        assert_eq!(
            plan.cleanup_script(),
            "#!/bin/sh\ncd \"/var/lib/hakchi/games/snes-eur\"\n"
        );

        let mut drifted = remote.clone();
        drifted[0].mtime = Some(drifted[0].mtime.unwrap() + 10);
        drifted[1].size += 1;
        let plan = sync_plan(collection.clone(), &drifted, &[], paths.clone(), Some(1));
        assert_eq!(plan.uploads.len(), 2);
        assert!(!plan.fits());
        assert!(plan.summary().contains("NOT ENOUGH SPACE"));
    }

    #[test]
    fn original_blocks_parse() {
        let text = "==== CLV-P-SAAAE\n[Desktop Entry]\nName=A\n\n==== CLV-P-SAABE\n[Desktop Entry]\nName=B\n\n==== junk\nx\n";
        let blocks = parse_original_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0],
            (
                "CLV-P-SAAAE".to_string(),
                "[Desktop Entry]\nName=A\n".to_string()
            )
        );
        assert_eq!(blocks[1].1, "[Desktop Entry]\nName=B\n");
    }

    #[test]
    fn art_blocks_are_cut_by_the_announced_sizes() {
        let mut output = b"==== CLV-P-SAAAE/CLV-P-SAAAE.png 12\n".to_vec();
        output.extend_from_slice(b"\x89PNG\n==== x\n");
        output.extend_from_slice(b"==== CLV-P-SAAAE/CLV-P-SAAAE_small.png 0\n");
        output.extend_from_slice(b"==== CLV-P-SAABE/CLV-P-SAABE.png 5\nabc");
        let blocks = parse_art_blocks(&output);
        assert_eq!(
            blocks,
            [
                ("CLV-P-SAAAE.png".to_string(), b"\x89PNG\n==== x\n".to_vec()),
                ("CLV-P-SAAAE_small.png".to_string(), Vec::new()),
            ]
        );
    }

    #[tokio::test]
    async fn original_art_fetch_asks_only_for_missing_files() {
        let (_dir, library, _menus, paths) = trace_library();
        let stock = include_str!("../tests/fixtures/stock-snes-eur-CLV-P-SAAAE.desktop");
        library
            .store_originals(
                "snes-eur",
                &[("CLV-P-SAABE".to_string(), stock.to_string())],
            )
            .unwrap();
        library
            .store_original_art(
                "snes-eur",
                &[("CLV-P-SAAAE_small.png".to_string(), b"small".to_vec())],
            )
            .unwrap();
        let missing = library.missing_original_art("snes-eur").unwrap();
        assert_eq!(
            missing,
            [
                "CLV-P-SAAAE.png",
                "CLV-P-SAABE.png",
                "CLV-P-SAABE_small.png"
            ]
        );
        let mut reply = b"==== CLV-P-SAAAE/CLV-P-SAAAE.png 9\n\x89PNG\r\n\x1a\n\0".to_vec();
        reply.extend_from_slice(b"==== CLV-P-SAABE/CLV-P-SAABE.png 0\n");
        reply.extend_from_slice(b"==== CLV-P-SAABE/CLV-P-SAABE_small.png 2\nok");
        let shell = Scripted::new(vec![(
            r#"cd "/var/squashfs/usr/share/games" || exit 1; for f in CLV-P-SAAAE/CLV-P-SAAAE.png CLV-P-SAABE/CLV-P-SAABE.png CLV-P-SAABE/CLV-P-SAABE_small.png; do echo "==== $f $(stat -c %s "$f" 2>/dev/null || echo 0)"; cat "$f" 2>/dev/null; done; :"#,
            None,
            ExecOutput {
                exit_code: Some(0),
                stdout: reply,
                stderr: Vec::new(),
            },
        )]);
        let files = fetch_original_art(&shell, &paths.squashfs_games, &missing)
            .await
            .unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(library.store_original_art("snes-eur", &files).unwrap(), 2);
        assert_eq!(
            std::fs::read(library.original_art("snes-eur", "CLV-P-SAAAE.png").unwrap()).unwrap(),
            b"\x89PNG\r\n\x1a\n\0"
        );
        assert_eq!(
            library.missing_original_art("snes-eur").unwrap(),
            ["CLV-P-SAABE.png"]
        );
        // Nothing to fetch: nothing is sent.
        let quiet = Scripted::new(vec![]);
        assert!(
            fetch_original_art(&quiet, &paths.squashfs_games, &[])
                .await
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            fetch_original_art(&quiet, &paths.squashfs_games, &["game.json".to_string()]).await,
            Err(Error::InvalidCode(_))
        ));
    }

    #[test]
    fn config_scripts_escape_quotes() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        let mut plan = sync_plan(collection, &[], &[], paths, None);
        assert_eq!(plan.config_script(), "");
        plan.config
            .insert("snes_extra_args".into(), "--x 'y'".into());
        assert_eq!(plan.config_script(), "cfg_snes_extra_args='--x \\'y\\''\n");
    }

    /// One expected command, the stdin it must receive (if checked) and the reply.
    type Step = (String, Option<Vec<u8>>, ExecOutput);
    /// One recorded upload: upload path, directories, file paths.
    type Uploaded = (String, Vec<String>, Vec<String>);

    /// Replays a scripted conversation, asserting every command string and the uploads.
    struct Scripted {
        steps: Mutex<VecDeque<Step>>,
        uploads: Mutex<Vec<Uploaded>>,
        /// The upload fails part way, as when the cable is pulled.
        fail_upload: bool,
    }

    impl Scripted {
        fn new(steps: Vec<(&str, Option<&[u8]>, ExecOutput)>) -> Self {
            Self {
                steps: Mutex::new(
                    steps
                        .into_iter()
                        .map(|(c, s, o)| (c.to_string(), s.map(<[u8]>::to_vec), o))
                        .collect(),
                ),
                uploads: Mutex::new(Vec::new()),
                fail_upload: false,
            }
        }

        fn with_failing_upload(mut self) -> Self {
            self.fail_upload = true;
            self
        }
    }

    fn ok(stdout: &str) -> ExecOutput {
        ExecOutput {
            exit_code: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn code(exit_code: u32) -> ExecOutput {
        ExecOutput {
            exit_code: Some(exit_code),
            ..ok("")
        }
    }

    impl Transport for Scripted {
        fn exec<'a>(
            &'a self,
            command: &'a str,
            stdin: Option<&'a [u8]>,
        ) -> impl Future<Output = Result<ExecOutput>> + Send + 'a {
            let next = self.steps.lock().unwrap().pop_front();
            async move {
                let (expected, expected_stdin, output) =
                    next.unwrap_or_else(|| panic!("unexpected command {command:?}"));
                assert_eq!(command, expected, "command sent to the console");
                if let Some(expected_stdin) = expected_stdin {
                    assert_eq!(
                        String::from_utf8_lossy(stdin.unwrap_or(&[])),
                        String::from_utf8_lossy(&expected_stdin),
                        "stdin of {command}"
                    );
                }
                Ok(output)
            }
        }

        async fn upload(
            &self,
            upload_path: &str,
            directories: &[String],
            files: &[LocalFile],
            progress: &mut (dyn FnMut(SyncEvent) + Send),
        ) -> Result<u64> {
            {
                if self.fail_upload {
                    return Err(crate::error::io_error(
                        "upload",
                        std::path::Path::new(upload_path),
                        std::io::Error::new(
                            std::io::ErrorKind::ConnectionReset,
                            "connection reset by peer",
                        ),
                    ));
                }
                self.uploads.lock().unwrap().push((
                    upload_path.to_string(),
                    directories.to_vec(),
                    files.iter().map(|f| f.path.clone()).collect(),
                ));
                let total: u64 = files.iter().map(|f| f.size).sum();
                progress(SyncEvent::Upload {
                    path: String::new(),
                    done: total,
                    total,
                    overall_done: total,
                    overall_total: total,
                });
                Ok(total)
            }
        }
    }

    impl Shell for Scripted {
        async fn exec<'a>(
            &'a self,
            command: &'a str,
            stdin: Option<&'a [u8]>,
        ) -> cartload_console::Result<ExecOutput> {
            Transport::exec(self, command, stdin).await.map_err(
                |error| -> cartload_console::Error { unreachable!("scripted shell: {error}") },
            )
        }
    }

    #[tokio::test]
    async fn sync_replays_the_verified_order() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        // The console holds one stale file and one file that is already right.
        let kept = &collection.files[2];
        let listing = format!(
            "./001/CLV-S-00000/CLV-S-00000_mdmini.png 2953 2026-10-21 09:02:47.000000000\n{} {} {}.000000000\n",
            kept.path,
            kept.size,
            format_stat_time(kept.mtime)
        );
        let initial = sync_plan(collection.clone(), &[], &[], paths.clone(), Some(1 << 30));
        let expected = initial.refresh(
            &parse_listing(&listing),
            &["./000/CLV-P-SAAAE/autoplay".to_string()],
        );
        assert_eq!(expected.uploads.len(), collection.files.len() - 1);
        let cleanup = expected.cleanup_script();
        let modtime = expected.modtime_script();
        let up = "/var/lib/hakchi/games/snes-eur";
        let shell = Scripted::new(vec![
            ("which tiny7zx", None, ok("/bin/tiny7zx\n")),
            ("uistop", None, ok("")),
            (r#"hakchi eval 'umount "$gamepath"'"#, None, code(1)),
            (
                &format!(r#"cd "{up}" && find . -type l"#),
                None,
                ok("./000/CLV-P-SAAAE/autoplay\n"),
            ),
            (&commands::list_files(up), None, ok(&listing)),
            ("cat > /tmp/cleanup.sh", Some(cleanup.as_bytes()), ok("")),
            ("chmod +x /tmp/cleanup.sh && /tmp/cleanup.sh", None, ok("")),
            ("rm /tmp/cleanup.sh", None, ok("")),
            ("cat > /tmp/modtime.sh", Some(modtime.as_bytes()), ok("")),
            ("chmod +x /tmp/modtime.sh && /tmp/modtime.sh", None, ok("")),
            ("rm /tmp/modtime.sh", None, ok("")),
            (
                "cat > /tmp/linkgamessync.sh",
                Some(collection.link_script.as_bytes()),
                ok(""),
            ),
            (
                "chmod +x /tmp/linkgamessync.sh && /tmp/linkgamessync.sh",
                None,
                ok(""),
            ),
            ("rm /tmp/linkgamessync.sh", None, ok("")),
            ("hakchi eval", Some(b""), ok("")),
            (
                "hakchi overmount_games; uistart",
                None,
                ok("menu code: 000\n"),
            ),
        ]);
        let mut events = Vec::new();
        let report = sync(&shell, &initial, |e| events.push(e)).await.unwrap();
        assert!(
            shell.steps.lock().unwrap().is_empty(),
            "every command was sent"
        );
        assert_eq!(report.uploaded_files, collection.files.len() - 1);
        assert_eq!(report.removed_files, 1);
        assert_eq!(report.removed_links, 1);
        assert!(report.games_mounted);
        let uploads = shell.uploads.lock().unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].0, up);
        assert_eq!(uploads[0].1, expected.directories());
        assert!(!uploads[0].2.contains(&kept.path));
        let steps: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                SyncEvent::Step { name, .. } => Some(*name),
                _ => None,
            })
            .collect();
        assert_eq!(steps, STEPS);
        assert!(cleanup.contains("rm './001/CLV-S-00000/CLV-S-00000_mdmini.png'\n"));
        assert!(cleanup.contains("rm './000/CLV-P-SAAAE/autoplay'\n"));
    }

    #[tokio::test]
    async fn sync_restarts_the_menu_when_the_upload_fails() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        let plan = sync_plan(collection, &[], &[], paths.clone(), Some(1 << 30));
        let upload_path = paths.upload_path();
        // A leftover of an interrupted upload is a stale file: the next sync removes it.
        let listing =
            "./000/CLV-P-SAAAE/x.sfrom.cartload-part 1234 2026-10-21 09:02:47.000000000\n";
        let refreshed = plan.refresh(&parse_listing(listing), &[]);
        assert_eq!(
            refreshed.deletions,
            ["./000/CLV-P-SAAAE/x.sfrom.cartload-part"]
        );
        let scripted = Scripted::new(vec![
            (commands::WHICH_EXTRACTOR, None, ok("/bin/tiny7zx")),
            (commands::UI_STOP, None, ok("")),
            (commands::UMOUNT_GAMES, None, code(1)),
            (&commands::list_links(&upload_path), None, ok("")),
            (&commands::list_files(&upload_path), None, ok(listing)),
            (
                &commands::write_script(commands::CLEANUP_SCRIPT),
                None,
                ok(""),
            ),
            (
                &commands::run_script(commands::CLEANUP_SCRIPT),
                None,
                ok(""),
            ),
            (
                &commands::remove_script(commands::CLEANUP_SCRIPT),
                None,
                ok(""),
            ),
            // The upload fails here; the menu is restarted and the error reported.
            (commands::FINISH, None, ok("")),
        ])
        .with_failing_upload();
        let mut events = Vec::new();
        let error = sync(&scripted, &plan, |e| events.push(e))
            .await
            .expect_err("the upload fails");
        assert!(error.to_string().contains("connection reset"), "{error}");
        assert!(scripted.steps.lock().unwrap().is_empty(), "FINISH was sent");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SyncEvent::Log(m) if m == "console menu restarted"))
        );
    }

    #[tokio::test]
    async fn sync_skips_the_upload_when_nothing_changed_and_refuses_without_space() {
        let (_dir, library, menus, paths) = trace_library();
        let collection = collect(&library, &menus, &paths).unwrap();
        let listing: String = collection
            .files
            .iter()
            .map(|f| {
                format!(
                    "{} {} {}.000000000\n",
                    f.path,
                    f.size,
                    format_stat_time(f.mtime)
                )
            })
            .collect();
        let plan = sync_plan(collection.clone(), &[], &[], paths.clone(), None);
        let up = "/var/lib/hakchi/games/snes-eur";
        let shell = Scripted::new(vec![
            ("which tiny7zx", None, ok("/bin/tiny7zx\n")),
            ("uistop", None, ok("")),
            (r#"hakchi eval 'umount "$gamepath"'"#, None, ok("")),
            (&format!(r#"cd "{up}" && find . -type l"#), None, ok("")),
            (&commands::list_files(up), None, ok(&listing)),
            (
                "cat > /tmp/cleanup.sh",
                Some(format!("#!/bin/sh\ncd \"{up}\"\n").as_bytes()),
                ok(""),
            ),
            ("chmod +x /tmp/cleanup.sh && /tmp/cleanup.sh", None, ok("")),
            ("rm /tmp/cleanup.sh", None, ok("")),
            ("cat > /tmp/linkgamessync.sh", None, ok("")),
            (
                "chmod +x /tmp/linkgamessync.sh && /tmp/linkgamessync.sh",
                None,
                ok(""),
            ),
            ("rm /tmp/linkgamessync.sh", None, ok("")),
            ("hakchi eval", None, ok("")),
            ("hakchi overmount_games; uistart", None, code(1)),
        ]);
        let report = sync(&shell, &plan, |_| {}).await.unwrap();
        assert!(shell.steps.lock().unwrap().is_empty());
        assert_eq!(report.uploaded_files, 0);
        assert!(shell.uploads.lock().unwrap().is_empty());
        assert!(!report.games_mounted);

        let full = sync_plan(collection.clone(), &[], &[], paths.clone(), Some(10));
        let shell = Scripted::new(vec![]);
        assert!(matches!(
            sync(&shell, &full, |_| {}).await,
            Err(Error::NotEnoughSpace { .. })
        ));

        // No extractor on the console: nothing else is sent.
        let plan = sync_plan(collection, &[], &[], paths, None);
        let shell = Scripted::new(vec![("which tiny7zx", None, code(1))]);
        let error = sync(&shell, &plan, |_| {}).await.unwrap_err();
        assert!(
            matches!(error, Error::MissingExtractor { games: 1 }),
            "{error}"
        );
        assert!(error.to_string().contains("cartload-flow base-mods"));
        assert!(shell.steps.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn uncompressed_library_skips_the_extractor_check() {
        let (_dir, library, menus, paths) = trace_library();
        let mut game = library.games().unwrap().remove(0);
        library.set_compress(&mut game, false).unwrap();
        let collection = collect(&library, &menus, &paths).unwrap();
        assert_eq!(collection.compressed, 0);
        assert!(
            collection
                .files
                .iter()
                .any(|f| f.path.ends_with("/Chrono_Trigger.sfrom"))
        );
        let plan = sync_plan(collection, &[], &[], paths, Some(1));
        assert!(!plan.needs_extractor());
        let shell = Scripted::new(vec![]);
        assert!(matches!(
            sync(&shell, &plan, |_| {}).await,
            Err(Error::NotEnoughSpace { .. })
        ));
    }

    #[test]
    fn library_config_default_matches_ce() {
        let config = LibraryConfig::default();
        assert_eq!(config.menu.structure, Structure::Count { max: 30 });
        assert!(config.menu.originals_at_root);
    }
}
