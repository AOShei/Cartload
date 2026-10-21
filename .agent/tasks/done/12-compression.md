# Task 12: optional ROM compression on the console

hakchi2 CE's "Compress" option packs each ROM as a `.7z` and the launch entry points at the
archive; the device-side `tiny7zx` (from CE's base hmods) extracts it to RAM at launch. The
trace shows Chrono Trigger at 2.7 MB compressed versus 4.2 MB as `.sfrom`, so compression
roughly doubles how many SNES games fit in the 313 MB.

Read how CE does it: `hakchi_gui/NesApplication.cs` (the `.7z` path and command line),
`Tasks/SyncTask.cs`, and the `tiny7zx` and `font_remount` entries in CE's `basehmods.tar`
(`GeneratedData/basehmods.tar` in the CE tree). Confirm what the console needs on its side
(which hmod provides the extractor, how the launch command invokes it) by reading the hmod
contents; the console has hakchi2 CE's base hmods installed, so `cartload-console exec` may be
used read-only to check what is present.

Implement:
- `library`: a per-game `compress: bool` (default on for SNES/NES), 7z LZMA packing in Rust
  (`sevenz-rust2` or equivalent, no external binary), launch entries that match CE's byte for
  byte for a compressed game, and the sync plan carrying the archive instead of the raw ROM.
- `flow`: an `InstallBaseMods` step (optional, after install) that installs the mods a
  compressed library needs, from the same GPL-3 CE artifacts, downloaded on demand and cached,
  never bundled in this repo.
- CLI and tests as usual (archive round-trip, entry text against the CE fixture for the
  `.7z` case, plan with a compressed game).

Verify, update the "Sync one game" checklist, commit.
