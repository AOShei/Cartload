//! hakchi2 CE's device-side kexec tools, embedded from `data/` at the repository root (origin,
//! checksums and licences in `data/README.md`).
//!
//! A memboot from a running hakchi kernel does not use the console's own `kexec`: hakchi2 CE
//! uploads a static `kexec` build and `detached-fallback`, a standalone copy of hakchi's
//! `memboot`, `reboot_kexec` and `recovery` shell functions that puts its own directory first
//! on `PATH`, and runs those (`MembootTasks.Memboot`). Cartload ships the same two files
//! unchanged, the way `fes1.bin` ships in `cartload-hmod`.

/// `kexec.static`: kexec-tools (GPL-2) built statically for the console's ARM kernel by the
/// hakchi project. 722 876 bytes, md5 `85e2470a0c78c9876536be7d9798c004`.
pub const fn kexec_static() -> &'static [u8] {
    KEXEC_STATIC
}

/// `detached-fallback` (hakchi, GPL-3): re-runs itself in the background, sources
/// `/etc/preinit`, prepends its directory to `PATH` so that `kexec` resolves to
/// [`kexec_static`] uploaded next to it, and runs the function named by its first argument.
/// `recovery <image>` loads the image (or, without one, the kernel in NAND) with `hakchi-shell`
/// on the command line, unmounts everything and `kexec -e`s. 1 873 bytes, md5
/// `7a3607d5bc19bef626f03eb120819368`.
pub const fn detached_fallback() -> &'static [u8] {
    DETACHED_FALLBACK
}

const KEXEC_STATIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/kexec.static"
));
const DETACHED_FALLBACK: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/detached-fallback"
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernels::md5_hex;

    /// Guards the embedded binary against accidental corruption: a stripped static 32-bit ARM
    /// ELF executable with the md5 hakchi2 CE ships.
    #[test]
    fn kexec_static_is_the_hakchi2_ce_binary() {
        let bytes = kexec_static();
        assert_eq!(bytes.len(), 722_876);
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert_eq!(bytes[4], 1, "ELFCLASS32");
        assert_eq!(u16::from_le_bytes([bytes[16], bytes[17]]), 2, "ET_EXEC");
        assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), 0x28, "EM_ARM");
        assert_eq!(md5_hex(bytes), "85e2470a0c78c9876536be7d9798c004");
    }

    #[test]
    fn detached_fallback_is_the_hakchi2_ce_script() {
        let bytes = detached_fallback();
        assert_eq!(bytes.len(), 1873);
        assert_eq!(md5_hex(bytes), "7a3607d5bc19bef626f03eb120819368");
        let text = std::str::from_utf8(bytes).expect("a shell script");
        assert!(text.starts_with("#!/bin/sh\n"));
        assert!(text.contains("\"$0\" --detached ${1+\"$@\"} &"));
        assert!(text.contains("source /etc/preinit"));
        assert!(text.contains("export PATH=\"$cdir:$PATH\""));
        assert!(text.contains("recovery(){"));
        assert!(text.contains("kexec -l -t zImage"));
        assert!(text.contains("kexec -e"));
    }
}
