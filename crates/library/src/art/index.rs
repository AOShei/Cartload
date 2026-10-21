//! What the art module keeps in the library, under `<library>/art/`:
//!
//! - `index.json`: ROM hash → TheGamesDB game id, with where the match came from and the
//!   search candidates, so a second import of the same ROM needs no search.
//! - `allowance.json`: the allowance as last reported by the API.
//! - `tgdb/<id>/record.json`, `front.<ext>`, `clearlogo.png`: the game record and the
//!   downloads, so re-applying art to a game needs no network.
//! - `names/publishers.json`, `names/developers.json`: the id → name lists.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    Result,
    art::client::{Allowance, ArtSize, GameRecord},
    error::io_error,
    library::{read_json, write_json},
};

/// Directory under the library root.
pub const ART_DIR: &str = "art";
const INDEX_JSON: &str = "index.json";
const ALLOWANCE_JSON: &str = "allowance.json";
const RECORDS_DIR: &str = "tgdb";
const RECORD_JSON: &str = "record.json";
const NAMES_DIR: &str = "names";
/// Current `index.json` format.
pub const INDEX_VERSION: u32 = 1;

/// How a ROM hash got its TheGamesDB id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchSource {
    /// The hash database named the id.
    HashDb,
    /// A search by name picked it.
    Search,
    /// The user chose it.
    Manual,
}

/// A search result kept for the user to pick from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// TheGamesDB game id.
    pub id: u32,
    /// Title.
    pub title: String,
    /// Platform id.
    #[serde(default)]
    pub platform: Option<u32>,
    /// Release date.
    #[serde(default)]
    pub release_date: Option<String>,
}

/// One ROM hash in the index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// The id, or `None` when a search found nothing (so it is not repeated).
    #[serde(default)]
    pub tgdb_id: Option<u32>,
    /// Title of the matched game.
    #[serde(default)]
    pub title: Option<String>,
    /// Where the match came from.
    pub source: MatchSource,
    /// The search term used, for searches.
    #[serde(default)]
    pub query: Option<String>,
    /// Other search results, best first (at most [`MAX_CANDIDATES`]).
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    /// Seconds since the Unix epoch.
    #[serde(default)]
    pub updated_at: i64,
}

/// Candidates kept per entry.
pub const MAX_CANDIDATES: usize = 5;

/// `index.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    /// Format version.
    pub version: u32,
    /// Entries by CRC-32 in upper-case hex (`2D206BF7`).
    #[serde(default)]
    pub entries: BTreeMap<String, IndexEntry>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            version: INDEX_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

impl Index {
    /// The entry of a ROM hash.
    pub fn get(&self, crc32: u32) -> Option<&IndexEntry> {
        self.entries.get(&hash_key(crc32))
    }

    /// Sets the entry of a ROM hash.
    pub fn set(&mut self, crc32: u32, entry: IndexEntry) {
        self.entries.insert(hash_key(crc32), entry);
    }

    /// Removes the entry of a ROM hash.
    pub fn remove(&mut self, crc32: u32) -> Option<IndexEntry> {
        self.entries.remove(&hash_key(crc32))
    }
}

/// The index key of a CRC-32.
pub fn hash_key(crc32: u32) -> String {
    format!("{crc32:08X}")
}

/// Whether the clear logo of a game was looked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogoState {
    /// Not looked for yet.
    #[default]
    Unknown,
    /// The CDN has none.
    Missing,
    /// Downloaded to `clearlogo.png`.
    Present,
}

/// A game record with the state of its downloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedRecord {
    /// The record as the API gave it.
    pub record: GameRecord,
    /// When it was fetched, seconds since the Unix epoch.
    #[serde(default)]
    pub fetched_at: i64,
    /// File name of the downloaded front box art (`front.jpg`), when downloaded.
    #[serde(default)]
    pub front_file: Option<String>,
    /// The size variant that was downloaded.
    #[serde(default)]
    pub front_size: Option<ArtSize>,
    /// The clear logo state.
    #[serde(default)]
    pub logo: LogoState,
}

/// File name of a downloaded clear logo.
pub const CLEAR_LOGO_FILE: &str = "clearlogo.png";

/// The id → name lists the API serves whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameList {
    /// `Publishers`.
    Publishers,
    /// `Developers`.
    Developers,
}

impl NameList {
    /// The route and the JSON key.
    pub fn key(self) -> &'static str {
        match self {
            Self::Publishers => "publishers",
            Self::Developers => "developers",
        }
    }
}

/// The art directory of a library.
#[derive(Clone, Debug)]
pub struct ArtStore {
    root: PathBuf,
}

impl ArtStore {
    /// Opens (creating if needed) the store at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|e| io_error("create directory", &root, e))?;
        Ok(Self { root })
    }

    /// Where the store lives.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The index (empty when there is none yet).
    pub fn index(&self) -> Result<Index> {
        let path = self.root.join(INDEX_JSON);
        if !path.exists() {
            return Ok(Index::default());
        }
        read_json(&path)
    }

    /// Writes the index.
    pub fn save_index(&self, index: &Index) -> Result<()> {
        write_json(&self.root.join(INDEX_JSON), index)
    }

    /// The entry of a ROM hash.
    pub fn lookup(&self, crc32: u32) -> Result<Option<IndexEntry>> {
        Ok(self.index()?.get(crc32).cloned())
    }

    /// Sets the entry of a ROM hash.
    pub fn remember(&self, crc32: u32, entry: IndexEntry) -> Result<()> {
        let mut index = self.index()?;
        index.set(crc32, entry);
        self.save_index(&index)
    }

    /// Removes the entry of a ROM hash.
    pub fn forget(&self, crc32: u32) -> Result<()> {
        let mut index = self.index()?;
        if index.remove(crc32).is_some() {
            self.save_index(&index)?;
        }
        Ok(())
    }

    /// The allowance as last saved.
    pub fn allowance(&self) -> Result<Allowance> {
        let path = self.root.join(ALLOWANCE_JSON);
        if !path.exists() {
            return Ok(Allowance::default());
        }
        read_json(&path)
    }

    /// Saves the allowance.
    pub fn save_allowance(&self, allowance: &Allowance) -> Result<()> {
        write_json(&self.root.join(ALLOWANCE_JSON), allowance)
    }

    /// Directory of a game record and its downloads.
    pub fn record_dir(&self, id: u32) -> PathBuf {
        self.root.join(RECORDS_DIR).join(id.to_string())
    }

    /// The cached record of a game, when there is one.
    pub fn record(&self, id: u32) -> Result<Option<CachedRecord>> {
        let path = self.record_dir(id).join(RECORD_JSON);
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    /// Saves a record.
    pub fn save_record(&self, cached: &CachedRecord) -> Result<()> {
        let dir = self.record_dir(cached.record.id);
        std::fs::create_dir_all(&dir).map_err(|e| io_error("create directory", &dir, e))?;
        write_json(&dir.join(RECORD_JSON), cached)
    }

    /// Path of a downloaded file of a game.
    pub fn image_path(&self, id: u32, name: &str) -> PathBuf {
        self.record_dir(id).join(name)
    }

    /// Reads a downloaded file of a game, when present.
    pub fn image(&self, id: u32, name: &str) -> Result<Option<Vec<u8>>> {
        let path = self.image_path(id, name);
        if !path.is_file() {
            return Ok(None);
        }
        std::fs::read(&path)
            .map(Some)
            .map_err(|e| io_error("read", &path, e))
    }

    /// Stores a downloaded file of a game.
    pub fn save_image(&self, id: u32, name: &str, bytes: &[u8]) -> Result<PathBuf> {
        let dir = self.record_dir(id);
        std::fs::create_dir_all(&dir).map_err(|e| io_error("create directory", &dir, e))?;
        let path = dir.join(name);
        std::fs::write(&path, bytes).map_err(|e| io_error("write", &path, e))?;
        Ok(path)
    }

    /// A cached id → name list, when fetched before.
    pub fn names(&self, list: NameList) -> Result<Option<BTreeMap<u32, String>>> {
        let path = self.names_path(list);
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    /// Saves an id → name list.
    pub fn save_names(&self, list: NameList, names: &BTreeMap<u32, String>) -> Result<()> {
        let dir = self.root.join(NAMES_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| io_error("create directory", &dir, e))?;
        write_json(&self.names_path(list), names)
    }

    fn names_path(&self, list: NameList) -> PathBuf {
        self.root
            .join(NAMES_DIR)
            .join(format!("{}.json", list.key()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::art::client::ImageUrls;

    fn record(id: u32) -> GameRecord {
        GameRecord {
            id,
            title: format!("Game {id}"),
            platform: Some(6),
            release_date: Some("1995-03-11".into()),
            players: Some(1),
            coop: Some(false),
            overview: None,
            publisher_ids: vec![4365],
            developer_ids: vec![],
            alternates: vec![],
            image_urls: ImageUrls::default(),
            images: vec![],
        }
    }

    #[test]
    fn index_round_trips_and_keys_by_hex_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtStore::open(dir.path().join("art")).unwrap();
        assert_eq!(store.index().unwrap(), Index::default());
        assert_eq!(store.lookup(0x2D20_6BF7).unwrap(), None);
        let entry = IndexEntry {
            tgdb_id: Some(1255),
            title: Some("Chrono Trigger".into()),
            source: MatchSource::HashDb,
            query: None,
            candidates: vec![Candidate {
                id: 1256,
                title: "Chrono Trigger (Beta)".into(),
                platform: Some(6),
                release_date: None,
            }],
            updated_at: 42,
        };
        store.remember(0x2D20_6BF7, entry.clone()).unwrap();
        assert_eq!(store.lookup(0x2D20_6BF7).unwrap(), Some(entry.clone()));
        let text = std::fs::read_to_string(dir.path().join("art/index.json")).unwrap();
        assert!(text.contains("\"2D206BF7\""));
        assert!(text.contains("\"source\": \"hash_db\""));
        assert_eq!(hash_key(0xABC), "00000ABC");
        // Entries without an id record a failed search.
        store
            .remember(
                1,
                IndexEntry {
                    tgdb_id: None,
                    title: None,
                    source: MatchSource::Search,
                    query: Some("Nothing".into()),
                    candidates: vec![],
                    updated_at: 43,
                },
            )
            .unwrap();
        assert_eq!(store.index().unwrap().entries.len(), 2);
        store.forget(1).unwrap();
        store.forget(1).unwrap();
        assert_eq!(store.index().unwrap().entries.len(), 1);
    }

    #[test]
    fn records_images_names_and_allowance_persist() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtStore::open(dir.path().join("art")).unwrap();
        assert_eq!(store.record(1255).unwrap(), None);
        let cached = CachedRecord {
            record: record(1255),
            fetched_at: 1,
            front_file: Some("front.jpg".into()),
            front_size: Some(ArtSize::Medium),
            logo: LogoState::Missing,
        };
        store.save_record(&cached).unwrap();
        assert_eq!(store.record(1255).unwrap(), Some(cached));
        assert_eq!(store.image(1255, "front.jpg").unwrap(), None);
        let path = store.save_image(1255, "front.jpg", b"jpeg").unwrap();
        assert_eq!(path, dir.path().join("art/tgdb/1255/front.jpg"));
        assert_eq!(
            store.image(1255, "front.jpg").unwrap(),
            Some(b"jpeg".to_vec())
        );

        assert_eq!(store.names(NameList::Publishers).unwrap(), None);
        let names: BTreeMap<u32, String> = [(4365, "Square".to_string())].into();
        store.save_names(NameList::Publishers, &names).unwrap();
        assert_eq!(store.names(NameList::Publishers).unwrap(), Some(names));
        assert_eq!(store.names(NameList::Developers).unwrap(), None);

        assert_eq!(store.allowance().unwrap(), Allowance::default());
        let allowance = Allowance {
            remaining: Some(10),
            extra: 1,
            refresh_at: Some(99),
            updated_at: 50,
        };
        store.save_allowance(&allowance).unwrap();
        assert_eq!(store.allowance().unwrap(), allowance);
    }
}
