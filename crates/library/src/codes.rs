//! Game and folder codes.
//!
//! hakchi2 CE names every game directory `CLV-<prefix>-<five letters>` where the letters come
//! from the ROM's CRC-32 (`NesApplication.GenerateCode`) and the prefix identifies the system
//! (`AppTypeCollection`: `U` for SNES, `H` for NES, `G` for Mega Drive). The stock games are
//! `CLV-P-...`; menu folders are `CLV-S-<index>` with the index of the menu they open.

/// Prefix of SNES game codes.
pub const PREFIX_SNES: char = 'U';
/// Prefix of NES game codes.
pub const PREFIX_NES: char = 'H';
/// Prefix of Mega Drive game codes.
pub const PREFIX_MEGA_DRIVE: char = 'G';
/// Prefix of the stock games built into the firmware.
pub const PREFIX_ORIGINAL: char = 'P';
/// Prefix of menu folders.
pub const PREFIX_FOLDER: char = 'S';

/// The code hakchi2 CE derives from a ROM's CRC-32: five letters from successive 5-bit groups
/// of the checksum, each reduced modulo 26.
pub fn game_code(crc32: u32, prefix: char) -> String {
    let letters: String = [0u32, 5, 10, 15, 20]
        .iter()
        .map(|shift| char::from(b'A' + ((crc32 >> shift) % 26) as u8))
        .collect();
    format!("CLV-{prefix}-{letters}")
}

/// Code of the folder that opens menu `index` (`CLV-S-00001`).
pub fn folder_code(index: usize) -> String {
    format!("CLV-S-{index:05}")
}

/// Whether `code` has the `CLV-X-XXXXX` shape (one upper-case letter or digit as prefix, five
/// upper-case letters or digits).
pub fn is_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 11
        && bytes.starts_with(b"CLV-")
        && bytes[4].is_ascii_uppercase()
        && bytes[5] == b'-'
        && bytes[6..]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Whether `code` names a stock game (`CLV-P-...`).
pub fn is_original(code: &str) -> bool {
    is_code(code) && code.as_bytes()[4] == PREFIX_ORIGINAL as u8
}

/// Whether `code` names a menu folder (`CLV-S-...`).
pub fn is_folder(code: &str) -> bool {
    is_code(code) && code.as_bytes()[4] == PREFIX_FOLDER as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrono_trigger_gets_the_code_from_the_trace() {
        // CRC-32 of the headerless Chrono Trigger (USA) ROM, see docs/reference/hakchi-ce-sync-trace.log.
        assert_eq!(game_code(0x2D20_6BF7, PREFIX_SNES), "CLV-U-ZJSQU");
        assert_eq!(game_code(0, PREFIX_NES), "CLV-H-AAAAA");
    }

    #[test]
    fn folder_codes_are_zero_padded() {
        assert_eq!(folder_code(0), "CLV-S-00000");
        assert_eq!(folder_code(1), "CLV-S-00001");
        assert_eq!(folder_code(123), "CLV-S-00123");
    }

    #[test]
    fn code_shapes() {
        assert!(is_code("CLV-U-ZJSQU"));
        assert!(is_code("CLV-S-00001"));
        assert!(is_original("CLV-P-SAAAE"));
        assert!(!is_original("CLV-U-ZJSQU"));
        assert!(is_folder("CLV-S-00000"));
        assert!(!is_code("CLV-U-ZJSQ"));
        assert!(!is_code("clv-u-zjsqu"));
        assert!(!is_code("CLV-U-ZJSQU/"));
        assert!(!is_code("../U-ZJSQU"));
    }
}
