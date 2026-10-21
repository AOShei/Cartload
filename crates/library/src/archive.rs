//! 7z archives for compressed games.
//!
//! hakchi2 CE's "Compress games" option packs each ROM file into `<file>.7z` (one LZMA entry,
//! `NesApplication.Compress`) and points the launch entry at the archive. On the console the
//! emulator wrappers in the hakchi rootfs (`clover-canoe-shvc-wr`, `clover-kachikachi-wr`)
//! see the `.7z` extension, run `tiny7zx x` in a temporary directory, rename the one extracted
//! file to `rom.<its extension>` and start the emulator on that, so the archive's entry must
//! keep the ROM's own name and extension. `tiny7zx` comes from CE's `tiny7zx.hmod` base mod
//! (see `docs/device-protocol.md`); it decodes LZMA, LZMA2, BCJ, BCJ2 and COPY and keeps the
//! whole uncompressed file in memory, which is why CE never compresses files above
//! [`MAX_SIZE`].
//!
//! Packing is pure Rust (`sevenz-rust2`): no external binary. The output is deterministic for
//! the same input (no timestamps, fixed encoder settings), so a rebuilt archive is byte for
//! byte the one already on the console and the sync diff leaves it alone.

use std::io::Cursor;

use sevenz_rust2::{
    ArchiveEntry, ArchiveReader, ArchiveWriter, Password, encoder_options::LzmaOptions,
};

use crate::{Error, Result};

/// Extension of a packed ROM (`Chrono_Trigger.sfrom.7z`).
pub const EXTENSION: &str = "7z";
/// Largest file hakchi2 CE compresses (`NesApplication.MaxCompress`, 16 MiB): the extractor
/// holds the whole file in RAM on a console with 256 MB.
pub const MAX_SIZE: u64 = 16 * 1024 * 1024;
/// LZMA preset (xz levels): 6 is 7-Zip's "normal", the ratio CE's writer gets.
const LEVEL: u32 = 6;
/// Smallest dictionary worth using.
const MIN_DICTIONARY: u32 = 64 * 1024;
/// Largest dictionary: one SNES ROM at most, as the input never exceeds [`MAX_SIZE`].
const MAX_DICTIONARY: u32 = 16 * 1024 * 1024;

/// The archive name of a ROM file.
pub fn archive_name(rom_file: &str) -> String {
    format!("{rom_file}.{EXTENSION}")
}

/// Whether a file of this size may be packed (hakchi2 CE's limit).
pub fn can_compress(size: u64) -> bool {
    size <= MAX_SIZE
}

/// Dictionary size for an input: the next power of two at or above its length, between 64 KiB
/// and 16 MiB. A larger dictionary buys nothing for a single file and only costs memory while
/// encoding.
pub fn dictionary_size(len: usize) -> u32 {
    let len = u32::try_from(len).unwrap_or(MAX_DICTIONARY);
    len.next_power_of_two()
        .clamp(MIN_DICTIONARY, MAX_DICTIONARY)
}

/// Packs `data` as the single entry `name` of a 7z archive (LZMA, no timestamps).
pub fn pack(name: &str, data: &[u8]) -> Result<Vec<u8>> {
    if !can_compress(data.len() as u64) {
        return Err(Error::TooLargeToCompress {
            name: name.to_string(),
            size: data.len() as u64,
        });
    }
    let mut options = LzmaOptions::from_level(LEVEL);
    options.set_dictionary_size(dictionary_size(data.len()));
    let mut writer = ArchiveWriter::new(Cursor::new(Vec::with_capacity(data.len() / 2)))
        .map_err(|e| archive_error(name, e))?;
    writer.set_content_methods(vec![options.into()]);
    writer
        .push_archive_entry(ArchiveEntry::new_file(name), Some(data))
        .map_err(|e| archive_error(name, e))?;
    let cursor = writer
        .finish()
        .map_err(|e| archive_error(name, sevenz_rust2::Error::from(e)))?;
    Ok(cursor.into_inner())
}

/// Reads the single entry of an archive made by [`pack`] (or by hakchi2 CE): its name and
/// content. An archive with several entries is refused, as the console would start the wrong
/// one.
pub fn unpack(archive: &[u8]) -> Result<(String, Vec<u8>)> {
    let mut reader = ArchiveReader::new(Cursor::new(archive), Password::empty())
        .map_err(|e| archive_error("archive", e))?;
    let mut found: Option<(String, Vec<u8>)> = None;
    let mut count = 0usize;
    reader
        .for_each_entries(|entry, stream| {
            if entry.is_directory() {
                return Ok(true);
            }
            count += 1;
            if found.is_some() {
                return Ok(false);
            }
            let mut data = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
            stream.read_to_end(&mut data)?;
            found = Some((entry.name().to_string(), data));
            Ok(true)
        })
        .map_err(|e| archive_error("archive", e))?;
    if count != 1 {
        return Err(Error::Archive {
            name: "archive".to_string(),
            message: format!("expected one file inside, found {count}"),
        });
    }
    found.ok_or_else(|| Error::Archive {
        name: "archive".to_string(),
        message: "no file inside".to_string(),
    })
}

fn archive_error(name: &str, error: sevenz_rust2::Error) -> Error {
    Error::Archive {
        name: name.to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::rom::{self, tests::synthetic_snes};

    #[test]
    fn names_and_limits() {
        assert_eq!(
            archive_name("Chrono_Trigger.sfrom"),
            "Chrono_Trigger.sfrom.7z"
        );
        assert!(can_compress(MAX_SIZE));
        assert!(!can_compress(MAX_SIZE + 1));
        assert_eq!(dictionary_size(0), MIN_DICTIONARY);
        assert_eq!(dictionary_size(4_194_400), 8 * 1024 * 1024);
        assert_eq!(dictionary_size(128 * 1024), 128 * 1024);
        assert_eq!(dictionary_size(usize::MAX), MAX_DICTIONARY);
    }

    #[test]
    fn round_trip_is_lossless_smaller_and_deterministic() {
        let rom = synthetic_snes("CHRONO TRIGGER", rom::snes::RomType::HiRom, 0x02, 0x03);
        let sfrom = rom::prepare("Chrono Trigger.smc", rom).unwrap().data;
        let archive = pack("Chrono_Trigger.sfrom", &sfrom).unwrap();
        assert_eq!(&archive[..6], b"7z\xBC\xAF\x27\x1C", "7z signature");
        assert!(archive.len() < sfrom.len() / 4, "{} bytes", archive.len());
        let (name, data) = unpack(&archive).unwrap();
        assert_eq!(name, "Chrono_Trigger.sfrom");
        assert_eq!(data, sfrom);
        assert_eq!(pack("Chrono_Trigger.sfrom", &sfrom).unwrap(), archive);

        // Content that does not compress still round-trips.
        let noise: Vec<u8> = (0..70_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        let archive = pack("Game.nes", &noise).unwrap();
        assert_eq!(unpack(&archive).unwrap(), ("Game.nes".to_string(), noise));
    }

    #[test]
    fn refuses_oversized_input_and_garbage() {
        let error = pack("big.bin", &[]).map(|_| ()).err();
        assert!(error.is_none(), "empty input is fine");
        assert!(matches!(
            unpack(b"not an archive at all"),
            Err(Error::Archive { .. })
        ));
        let empty = pack("empty.nes", &[]).unwrap();
        assert_eq!(
            unpack(&empty).unwrap(),
            ("empty.nes".to_string(), Vec::new())
        );
    }
}
