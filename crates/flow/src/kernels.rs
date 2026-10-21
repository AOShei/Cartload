//! The stock kernel images Nintendo shipped, by MD5, so that a dump or a user-supplied file can
//! be recognised before it is flashed back. The list is the one hakchi2 CE checks user-supplied
//! dumps against (`Shared.CorrectKernels`); the Mega Drive Mini keeps its stock kernel in place
//! and has no entry.

/// A known stock kernel image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockKernel {
    /// Console the image belongs to.
    pub console: &'static str,
    /// Nintendo's file name for the image.
    pub image: &'static str,
    /// Lowercase hex MD5 of the image.
    pub md5: &'static str,
}

/// Every known stock kernel image.
pub const STOCK_KERNELS: &[StockKernel] = &[
    StockKernel {
        console: "NES Classic Mini",
        image: "kernel-dp-nes-release-v1.0.2-0-g99e37e1.img",
        md5: "5cfdca351484e7025648abc3b20032ff",
    },
    StockKernel {
        console: "NES Classic Mini",
        image: "kernel-dp-nes-release-v1.0.3-0-gc4c703b.img",
        md5: "07bfb800beba6ef619c29990d14b5158",
    },
    StockKernel {
        console: "NES Classic Mini",
        image: "kernel-dp-nes-release-v1.0.7-0-g4ea4041.img",
        md5: "90eec1e2b4f00e53dc2dd53a9e7334c1",
    },
    StockKernel {
        console: "Famicom Mini",
        image: "kernel-dp-hvc-release-v1.0.5-0-g2f04d11.img",
        md5: "ac8144c3ea4ab32e017648ee80bdc230",
    },
    StockKernel {
        console: "Famicom Mini",
        image: "kernel-dp-hvc-release-v1.0.8-0-g32708cb.img",
        md5: "cf5fd8e5ad0835bbf9dbd9bdc198a369",
    },
    StockKernel {
        console: "SNES Classic Mini (EUR)",
        image: "kernel-dp-sneseur-release-v2.0.7-0-geb2b275.img",
        md5: "d76c2a091ebe7b4614589fc6954653a5",
    },
    StockKernel {
        console: "SNES Classic Mini (EUR)",
        image: "kernel-dp-sneseur-release-v2.0.13-0-g9dca6c5.img",
        md5: "c2b57b550f35d64d1c6ce66f9b5180ce",
    },
    StockKernel {
        console: "SNES Classic Mini (EUR)",
        image: "kernel-dp-sneseur-release-v2.0.14-0-gd8b65c6.img",
        md5: "0f890bc78cbd9ede43b83b015ba4c022",
    },
    StockKernel {
        console: "SNES Classic Mini (USA)",
        image: "kernel-dp-snesusa-release-v2.0.7-0-geb2b275.img",
        md5: "5296e64818bf2d1dbdc6b594f3eefd17",
    },
    StockKernel {
        console: "SNES Classic Mini (USA)",
        image: "kernel-dp-snesusa-release-v2.0.13-0-g9dca6c5.img",
        md5: "449b711238575763c6701f5958323d48",
    },
    StockKernel {
        console: "SNES Classic Mini (USA)",
        image: "kernel-dp-snesusa-release-v2.0.14-0-gd8b65c6.img",
        md5: "228967ab1035a347caa9c880419df487",
    },
    StockKernel {
        console: "Super Famicom Mini",
        image: "kernel-dp-shvc-release-v2.0.12-0-gbff4fb3.img",
        md5: "632e179db63d9bcd42281f776a030c14",
    },
    StockKernel {
        console: "Super Famicom Mini",
        image: "kernel-dp-shvc-release-v2.0.14-0-gd8b65c6.img",
        md5: "c3378edfc1b96a5268a066d5fbe12d89",
    },
    StockKernel {
        console: "Famicom Mini Shonen Jump edition",
        image: "kernel-dp-hvcj-release-v3.0.1-0-gad315e1.img",
        md5: "8a6731a5aebea36293f076fad9afa600",
    },
];

/// Looks a lowercase hex MD5 up in [`STOCK_KERNELS`].
pub fn identify(md5: &str) -> Option<&'static StockKernel> {
    let md5 = md5.trim().to_ascii_lowercase();
    STOCK_KERNELS.iter().find(|k| k.md5 == md5)
}

/// Lowercase hex MD5 of `data`.
pub fn md5_hex(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// `kernel-dp-sneseur-release-v2.0.14-0-gd8b65c6.img (SNES Classic Mini (EUR))` or
/// `unknown image (md5 ...)`.
pub fn describe(md5: &str) -> String {
    match identify(md5) {
        Some(kernel) => format!("{} ({})", kernel.image, kernel.console),
        None => format!("not a known stock kernel image (md5 {md5})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_the_reference() {
        // RFC 1321 test vector.
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn identifies_known_images_case_insensitively() {
        let kernel = identify("0F890BC78CBD9EDE43B83B015BA4C022 ").expect("known");
        assert_eq!(kernel.console, "SNES Classic Mini (EUR)");
        assert!(kernel.image.contains("v2.0.14"));
        assert!(identify("00000000000000000000000000000000").is_none());
        assert_eq!(STOCK_KERNELS.len(), 14);
        let unique: std::collections::BTreeSet<_> = STOCK_KERNELS.iter().map(|k| k.md5).collect();
        assert_eq!(unique.len(), STOCK_KERNELS.len());
        assert!(STOCK_KERNELS.iter().all(|k| k.md5.len() == 32));
    }

    #[test]
    fn describes_both_cases() {
        assert_eq!(
            describe("8a6731a5aebea36293f076fad9afa600"),
            "kernel-dp-hvcj-release-v3.0.1-0-gad315e1.img (Famicom Mini Shonen Jump edition)"
        );
        assert_eq!(
            describe("deadbeef"),
            "not a known stock kernel image (md5 deadbeef)"
        );
    }
}
