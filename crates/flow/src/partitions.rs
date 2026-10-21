//! Parsing of the device-side `sunxi-part` output (NAND partition table).
//!
//! hakchi2 CE (`Sunxi/NandInfo.cs`) extracts every `(mbr|nand[a-z]):(label):(sectors)` match
//! from the output and multiplies the sector count by 512. The data partition is labelled
//! `data` or `rootfs_data`; a stock console also has an unused `UDISK` partition that
//! `sunxi-part grow` merges into it.

use serde::Serialize;

/// Bytes per sector in `sunxi-part` output.
pub const SECTOR: u64 = 512;
/// Label of the unused partition that `sunxi-part grow` merges away.
pub const UDISK: &str = "UDISK";
/// Labels the data partition carries, depending on firmware.
pub const DATA_LABELS: [&str; 2] = ["data", "rootfs_data"];

/// One NAND partition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Partition {
    /// Device name (`nanda`, `nandb`, ... or `mbr`).
    pub device: String,
    /// Partition label.
    pub label: String,
    /// Size in 512-byte sectors.
    pub sectors: u64,
}

impl Partition {
    /// Size in bytes.
    pub fn bytes(&self) -> u64 {
        self.sectors * SECTOR
    }
}

/// Parses `sunxi-part` output. Lines that do not match are ignored, so garbage or an empty
/// output yield an empty list.
pub fn parse_sunxi_part(output: &str) -> Vec<Partition> {
    let mut partitions = Vec::new();
    for line in output.lines() {
        let mut rest = line;
        while let Some((partition, consumed)) = next_match(rest) {
            partitions.push(partition);
            rest = &rest[consumed..];
        }
    }
    partitions
}

/// Finds the first `(mbr|nand[a-z]):label:digits` in `text`, returning it and the offset just
/// past the digits.
fn next_match(text: &str) -> Option<(Partition, usize)> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        let device_len = if text[start..].starts_with("mbr:") {
            3
        } else if text[start..].starts_with("nand")
            && bytes.get(start + 4).is_some_and(u8::is_ascii_lowercase)
            && bytes.get(start + 5) == Some(&b':')
        {
            5
        } else {
            continue;
        };
        let label_start = start + device_len + 1;
        let Some(label_len) = text[label_start..].find(':') else {
            continue;
        };
        let digits_start = label_start + label_len + 1;
        let digits_len = bytes[digits_start..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits_len == 0 {
            continue;
        }
        let Ok(sectors) = text[digits_start..digits_start + digits_len].parse::<u64>() else {
            continue;
        };
        return Some((
            Partition {
                device: text[start..start + device_len].to_string(),
                label: text[label_start..label_start + label_len].to_string(),
                sectors,
            },
            digits_start + digits_len,
        ));
    }
    None
}

/// The first partition carrying one of `labels`.
pub fn find_label<'a>(partitions: &'a [Partition], labels: &[&str]) -> Option<&'a Partition> {
    partitions
        .iter()
        .find(|p| labels.contains(&p.label.as_str()))
}

/// The data partition (`data` or `rootfs_data`).
pub fn data_partition(partitions: &[Partition]) -> Option<&Partition> {
    find_label(partitions, &DATA_LABELS)
}

/// Whether an unused `UDISK` partition is present, that is, whether `sunxi-part grow` has
/// something to merge.
pub fn has_udisk(partitions: &[Partition]) -> bool {
    find_label(partitions, &[UDISK]).is_some()
}

/// `1.5 GB`, `313.5 MB`, `12 KB`, `7 B`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
mbr:mbr:32768
nanda:bootloader:65536
nandb:rootfs:262144
nandc:data:524288
nandd:UDISK:3145728
";

    #[test]
    fn parses_device_label_and_sectors() {
        let partitions = parse_sunxi_part(SAMPLE);
        assert_eq!(partitions.len(), 5);
        assert_eq!(
            partitions[3],
            Partition {
                device: "nandc".into(),
                label: "data".into(),
                sectors: 524_288,
            }
        );
        assert_eq!(partitions[3].bytes(), 256 * 1024 * 1024);
        assert!(has_udisk(&partitions));
        assert_eq!(
            data_partition(&partitions).map(|p| p.device.as_str()),
            Some("nandc")
        );
    }

    #[test]
    fn matches_anywhere_in_a_line_like_the_ce_regex() {
        let partitions = parse_sunxi_part("part 3: nande:rootfs_data:1048576 (grown)\nnoise\n");
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].label, "rootfs_data");
        assert_eq!(partitions[0].sectors, 1_048_576);
        assert!(!has_udisk(&partitions));
        assert_eq!(
            data_partition(&partitions).map(|p| p.bytes()),
            Some(512 * 1024 * 1024)
        );
    }

    #[test]
    fn ignores_malformed_lines() {
        assert!(parse_sunxi_part("").is_empty());
        assert!(parse_sunxi_part("nandx:data\nnand1:data:12\nmbr::\nnanda:boot:x").is_empty());
        let partial = parse_sunxi_part("nanda:boot:12junk nandb:data:34");
        assert_eq!(partial.len(), 2);
        assert_eq!(partial[0].sectors, 12);
        assert_eq!(partial[1].label, "data");
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human_size(7), "7 B");
        assert_eq!(human_size(12 * 1024), "12.0 KB");
        assert_eq!(human_size(1_610_612_736), "1.5 GB");
        assert_eq!(human_size(328_728_576), "313.5 MB");
    }
}
