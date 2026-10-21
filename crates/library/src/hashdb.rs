//! The ROM hash database: `romfiles.xml` from TeamShinkansen's hash-db, the file hakchi2 CE
//! downloads at start-up. It maps the CRC-32 of a ROM file to its No-Intro name and
//! TheGamesDB ids. It carries no publisher, date or player count; those come from TheGamesDB
//! (task 07) and default until then.

use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use quick_xml::{Reader, events::Event};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, error::io_error, rom};

/// Where hakchi2 CE downloads the database from.
pub const ROMFILES_URL: &str =
    "https://github.com/TeamShinkansen/hash-db/releases/download/data/romfiles.xml.gz";

/// One `<file>` element.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomRecord {
    /// The file name in the database (`Chrono Trigger (USA).sfc`).
    pub name: String,
    /// TheGamesDB game ids, best match first.
    pub tgdb_ids: Vec<u32>,
}

impl RomRecord {
    /// The title without extension and region tags (`Chrono Trigger`).
    pub fn title(&self) -> String {
        rom::title_from_file_name(&self.name)
    }
}

/// The database in memory, keyed by CRC-32.
#[derive(Clone, Debug, Default)]
pub struct HashDb {
    records: HashMap<u32, RomRecord>,
}

impl HashDb {
    /// Parses the XML. Entries whose `crc32` attribute is not eight hex digits are skipped;
    /// the first entry for a checksum wins, as in hakchi2 CE.
    pub fn parse(xml: &str) -> Result<Self> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut records = HashMap::new();
        let mut current: Option<(Option<u32>, RomRecord)> = None;
        let mut in_tgdb = false;
        loop {
            let event = reader
                .read_event()
                .map_err(|e| Error::HashDb(format!("invalid XML: {e}")))?;
            match event {
                Event::Start(start) if start.name().as_ref() == b"file" => {
                    let mut crc = None;
                    let mut name = String::new();
                    for attribute in start.attributes().flatten() {
                        let value = attribute
                            .unescape_value()
                            .map_err(|e| Error::HashDb(format!("bad attribute: {e}")))?;
                        match attribute.key.as_ref() {
                            b"crc32" => {
                                let text = value.trim();
                                if text.len() == 8 && text.chars().all(|c| c.is_ascii_hexdigit()) {
                                    crc = u32::from_str_radix(text, 16).ok();
                                }
                            }
                            b"name" => name = value.into_owned(),
                            _ => {}
                        }
                    }
                    current = Some((
                        crc,
                        RomRecord {
                            name,
                            tgdb_ids: Vec::new(),
                        },
                    ));
                }
                Event::Start(start) if start.name().as_ref() == b"tgdb" => in_tgdb = true,
                Event::Text(text) if in_tgdb => {
                    if let Some((_, record)) = current.as_mut()
                        && let Ok(id) = text.xml_content().map(|t| t.trim().parse::<u32>())
                        && let Ok(id) = id
                    {
                        record.tgdb_ids.push(id);
                    }
                }
                Event::End(end) if end.name().as_ref() == b"tgdb" => in_tgdb = false,
                Event::End(end) if end.name().as_ref() == b"file" => {
                    if let Some((Some(crc), record)) = current.take() {
                        records.entry(crc).or_insert(record);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(Self { records })
    }

    /// Loads the database from a file, gzip-compressed (`romfiles.xml.gz`) or plain.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| io_error("read", path, e))?;
        let xml = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
            let mut xml = String::new();
            decoder
                .read_to_string(&mut xml)
                .map_err(|e| io_error("decompress", path, e))?;
            xml
        } else {
            String::from_utf8(bytes)
                .map_err(|e| Error::HashDb(format!("{}: {e}", path.display())))?
        };
        Self::parse(&xml)
    }

    /// The record for a checksum.
    pub fn lookup(&self, crc32: u32) -> Option<&RomRecord> {
        self.records.get(&crc32)
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the database is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Where the downloaded database is cached (`<cache dir>/cartload/romfiles.xml.gz`).
pub fn default_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join("cartload").join("romfiles.xml.gz"))
}

/// Downloads the database to `dest` (atomically, through `<dest>.partial`) and returns its
/// size. Parent directories are created.
#[cfg(feature = "fetch")]
pub async fn fetch(dest: &Path) -> Result<u64> {
    fetch_from(ROMFILES_URL, dest).await
}

/// Downloads the database from `url` to `dest`.
#[cfg(feature = "fetch")]
pub async fn fetch_from(url: &str, dest: &Path) -> Result<u64> {
    use tokio::io::AsyncWriteExt;

    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .user_agent(concat!("cartload/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|source| Error::Download {
            url: url.to_string(),
            source,
        })?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|source| Error::Download {
            url: url.to_string(),
            source,
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| io_error("create directory", parent, e))?;
    }
    let partial = dest.with_extension("gz.partial");
    let mut file = tokio::fs::File::create(&partial)
        .await
        .map_err(|e| io_error("create", &partial, e))?;
    let mut written = 0u64;
    let result: Result<()> = async {
        while let Some(chunk) = response.chunk().await.map_err(|source| Error::Download {
            url: url.to_string(),
            source,
        })? {
            file.write_all(&chunk)
                .await
                .map_err(|e| io_error("write", &partial, e))?;
            written += chunk.len() as u64;
        }
        file.flush()
            .await
            .map_err(|e| io_error("write", &partial, e))?;
        drop(file);
        // Reject anything that does not parse before replacing the cache.
        let check = partial.clone();
        tokio::task::spawn_blocking(move || HashDb::load(&check))
            .await
            .map_err(|e| io_error("verify", &partial, std::io::Error::other(e)))??;
        tokio::fs::rename(&partial, dest)
            .await
            .map_err(|e| io_error("rename", &partial, e))?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&partial).await;
    }
    result.map(|()| written)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<romfiles xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
	<file name="Chrono Trigger (USA).sfc" crc32="2D206BF7">
		<tgdb>1255</tgdb>
	</file>
	<file name="Reigen Doushi (Japan).nes" crc32="0021ED29">
		<tgdb>2005</tgdb>
		<tgdb>53104</tgdb>
	</file>
	<file name="Original Frisbee Disc Sports - Ultimate &amp; Golf (USA).nds" crc32="000B9D7F">
		<tgdb>24167</tgdb>
	</file>
	<file name="No ids (USA).sfc" crc32="0000000A">
	</file>
	<file name="Bad crc.sfc" crc32="xyz">
		<tgdb>1</tgdb>
	</file>
	<file name="Duplicate (Europe).sfc" crc32="2d206bf7">
		<tgdb>9</tgdb>
	</file>
</romfiles>
"#;

    #[test]
    fn parses_names_checksums_and_ids() {
        let db = HashDb::parse(SAMPLE).unwrap();
        assert_eq!(db.len(), 4);
        let chrono = db.lookup(0x2D20_6BF7).unwrap();
        assert_eq!(chrono.name, "Chrono Trigger (USA).sfc");
        assert_eq!(chrono.title(), "Chrono Trigger");
        assert_eq!(
            chrono.tgdb_ids,
            [1255],
            "the first entry for a checksum wins"
        );
        assert_eq!(db.lookup(0x0021_ED29).unwrap().tgdb_ids, [2005, 53104]);
        assert_eq!(
            db.lookup(0x000B_9D7F).unwrap().name,
            "Original Frisbee Disc Sports - Ultimate & Golf (USA).nds"
        );
        assert!(db.lookup(0x0000_000A).unwrap().tgdb_ids.is_empty());
        assert!(db.lookup(0xDEAD_BEEF).is_none());
    }

    #[test]
    fn loads_gzip_and_plain_files() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("romfiles.xml");
        std::fs::write(&plain, SAMPLE).unwrap();
        assert_eq!(HashDb::load(&plain).unwrap().len(), 4);

        let gz = dir.path().join("romfiles.xml.gz");
        let mut encoder = flate2::write::GzEncoder::new(
            std::fs::File::create(&gz).unwrap(),
            flate2::Compression::fast(),
        );
        encoder.write_all(SAMPLE.as_bytes()).unwrap();
        encoder.finish().unwrap();
        assert_eq!(HashDb::load(&gz).unwrap().len(), 4);

        assert!(
            HashDb::parse("<romfiles><file name=\"x\" crc32=\"00000001\"></wrong></romfiles>")
                .is_err()
        );
        assert!(HashDb::load(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn cache_path_is_under_cartload() {
        if let Some(path) = default_cache_path() {
            assert!(path.ends_with("cartload/romfiles.xml.gz"));
        }
    }
}
