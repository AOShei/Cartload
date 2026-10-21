//! Games library on disk, launch entries, art placeholders, the menu tree and the sync with
//! the console.
//!
//! - [`Library`]: one directory per game under a user data directory, with `game.json`, the
//!   ROM in the console's container (SNES `.sfrom`, NES `.nes`), the launch entry and art.
//!   [`Library::import`] takes `.sfc`/`.smc`/`.fig`/`.swc`/`.nes` files or zips of them.
//! - [`rom`]: header handling, the checksums hakchi2 CE uses and the `.sfrom` conversion.
//! - [`hashdb`]: the `romfiles.xml` hash database (names and TheGamesDB ids).
//! - [`desktop`]: the `.desktop` entry format, byte-compatible with the stock firmware and
//!   with what hakchi2 CE writes.
//! - [`menu`]: the folder tree and the automatic structures (flat, by letter, by count),
//!   as pure functions; [`menu::export`] assigns the `CLV-S-XXXXX` codes.
//! - [`sync`](mod@sync): the file set for the console, the pure diff against the console's listing,
//!   and the sync itself in the verified order.
//! - [`art`]: cover art and metadata from TheGamesDB, with the hash → id index and the
//!   allowance limiter; [`images`] makes the console's 228×204 cover and 40×40 thumbnail.
//!
//! The device-side formats follow hakchi2 CE (`NesApplication`, `SnesGame`, `DesktopFile`,
//! `NesMenuCollection`, `SyncTask`) and were checked against a console CE had synced.

pub mod art;
pub mod codes;
pub mod desktop;
mod error;
pub mod game;
pub mod hashdb;
pub mod images;
pub mod library;
pub mod menu;
pub mod rom;
pub mod sync;

pub use art::{ApiKey, ArtOptions, ArtReport, fetch_art};
pub use error::{Error, Result};
pub use game::Game;
pub use hashdb::HashDb;
pub use library::{Imported, Library, LibraryConfig};
pub use menu::{Menu, MenuOptions, Structure};
pub use rom::System;
pub use sync::{DevicePaths, SyncEvent, SyncPlan, SyncReport, collect, sync, sync_plan};

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
}
