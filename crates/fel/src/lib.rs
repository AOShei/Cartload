//! Allwinner FEL USB boot protocol (memboot of the Classic Mini consoles) over nusb.

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Placeholder until the crate has real functionality.
    #[error("FEL protocol not implemented yet")]
    Unimplemented,
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Crate name and version, for CLI banners and diagnostics.
pub const fn version() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_mentions_crate_name() {
        assert!(version().starts_with(env!("CARGO_PKG_NAME")));
    }

    #[test]
    fn error_displays_message() {
        assert_eq!(
            Error::Unimplemented.to_string(),
            "FEL protocol not implemented yet"
        );
    }
}
