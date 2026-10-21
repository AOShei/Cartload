# Report: 12-compression

## What was done

How CE does it, confirmed in its code and on the console (read-only `cartload-console exec`):
`NesApplication.Compress` packs each game file under 16 MiB (`MaxCompress`) into `<file>.7z`
(one LZMA entry, CE's `tiny7z` writer) and swaps the name in `Exec`; nothing else changes.
The extraction is in hakchi's own rootfs wrappers (`clover-canoe-shvc-wr`,
`clover-kachikachi-wr`, `m2engage` in `hakchi.hmod`'s ramdisk): a `-rom` ending in `.7z` is
`tiny7zx x`-ed into `/tmp/rom`, the one file renamed `rom.<ext>`, the emulator started on it.
Only the `tiny7zx` binary comes from a base mod (`tiny7zx.hmod`, pts-tiny-7z-sfx, GPL-2,
26 578 bytes, md5 `eb8fa3b5…`); `font_remount` is CE's extended title font and has nothing to
do with compression. On this console `which tiny7zx` → `/bin/tiny7zx` (the overlay's upper
dir `/var/lib/hakchi/rootfs/bin/tiny7zx`, same md5), `/var/lib/hakchi/hmod/uninstall-*`
registers the installed mods, `/etc/hmodblacklist` has `tiny7zx-dynamic` but not `tiny7zx`.
`basehmods.tar` is generated at CE build time from `hakchi_gui/mods/hmods/<name>.hmod/`
directories in CE's git tree. All of this is in `docs/device-protocol.md`, "Compressed games".

`crates/library`:
- `archive.rs`: `pack(name, data)` / `unpack(bytes)` on `sevenz-rust2` (pure Rust, feature
  `compress` only: LZMA, no AES), one entry named after the ROM, no timestamps, LZMA preset 6
  with the dictionary clamped to the input size, deterministic output; `MAX_SIZE` = CE's
  16 MiB; `can_compress`, `archive_name`.
- `Game.compress: bool` (serde default **true**, so a `game.json` from before this task counts
  as compressed), `archive_file()`, `console_rom_file()`; `desktop_entry` names the `.7z`.
- `Library`: `ImportOptions { compress }` (+ `import_with`, `import_bytes_with`; the old
  `import`/`import_bytes` keep working with the default), import packs SNES/NES ROMs (never
  Mega Drive, and ROMs above 16 MiB fall back to uncompressed with a warning), `set_compress`
  (builds/removes the archive, saves `game.json` and the entry), `ensure_archive` (rebuilds a
  missing or stale `.7z`), `rom_path`, `archive_path`; `game_files` returns the archive instead
  of the ROM for a compressed game and builds it when missing (legacy libraries heal on first
  use). The raw ROM stays in the game directory, unlike CE (toggling is instant and lossless).
- `sync`: `Collection.compressed`, `SyncPlan::needs_extractor()`, `commands::WHICH_EXTRACTOR`
  (`which tiny7zx`, what the wrappers rely on); `sync()` runs it before touching anything when
  the plan has compressed games and fails with `Error::MissingExtractor` pointing at
  `cartload-flow base-mods` or `compress --off`. Summary line shows `(N compressed)`.
- Errors: `Archive`, `TooLargeToCompress`, `MissingExtractor`.
- CLI: `import --no-compress`, `compress [--off] [--all] <CODE>...` (prints ROM → archive
  sizes and percentages), `list` shows the file the console gets.
- Workspace: `rust-version` 1.90 → 1.93 (sevenz-rust2's MSRV; CI already pins 1.95.0),
  `[profile.dev.package.{lzma-rust2,sevenz-rust2}] opt-level = 3` so debug builds and tests
  pack at a usable speed.

`crates/hmod`: `basemods.rs`: catalog (`TINY7ZX`, `FONT_REMOUNT`, `CATALOG`,
`FOR_COMPRESSION`) with every file's path, size, md5 and executable bit, pinned to CE commit
`4fb3092c29cd39a85504bd75889ffb18e749a704` (head of `mainline`); URLs on
`raw.githubusercontent.com` with the `github.com/.../raw/` route as fallback; cache
`<cache>/basemods/<commit>/<name>.hmod/…` (`default_cache_dir`, `status`, `load` with md5
verification and `ModNotCached` / `ModChecksum` errors); `LoadedMod` with a deterministic plain
tar (0755 for executables, root owner) which is what the console's `tar -xvC` + `packs_install`
take (CE's `ModTasks.TransferHmod`); `fetch()` (feature `fetch`) downloads only missing or
damaged files to `.partial`, verifies size+md5, renames. CLI `cartload-hmod base-mods [--fetch]
[--cache DIR] [NAME...]`. `fetch::client()` shared. Dependencies `dirs`, `md5`.

`crates/flow`: `steps/mods.rs`: `InstallBaseMods` (`base-mods`): already done when every payload
file is in the rootfs with the uploaded content's md5 and `<installpath>/hmod/uninstall-<name>`
exists; otherwise `mkdir -p`, `tar -xvC '/tmp/cartload/hmods/<name>.hmod'` with the tar on
stdin, `hakchi packs_install '/tmp/cartload/hmods'` (output echoed), md5 verification, then
`sync; umount -ar; reboot -f` (the rootfs is an overlayfs upper dir written behind the
overlay's back; CE's mod install also ends in a boot). `VerifyBaseMods` (`verify-mods`): files
and registration again, plus `which <binary>` for every `bin/` file. `DetectState::expect_installed()`
(`Expect::Installed`). Flows: `install_base_mods()` (wait, detect, base-mods, wait, detect,
verify-mods) and `install_kernel_with_base_mods()` (the 13 install steps + the same four).
`Context.base_mods` / `with_base_mods`. Commands in `steps::commands` (`MKDIR_MODS`,
`mkdir_mod`, `extract_mod`, `PACKS_INSTALL`, `mod_registered`, `which`, `md5sum_all`). CLI:
`cartload-flow base-mods [--mods tiny7zx,font_remount] [--mods-cache DIR] [--offline]` and
`install --base-mods`; the binary downloads on demand (feature `cli` enables
`cartload-hmod/fetch`; the library stays HTTP-free). The hakchi rootfs path comes from the
console (`hakchi get rootfs`), default `/var/lib/hakchi/rootfs`.

App: `GameEdit.compress: Option<bool>` applied through `set_compress`; TS `Game.compress`,
`GameEdit.compress?`; the detail panel's Advanced section has a "Compress for the console (7z)"
checkbox, the command preview shows the `.7z`, "Size on the console" says "(compressed)";
preview fixtures updated.

Docs: `device-protocol.md` "Compressed games" (wrappers, tiny7zx, base mods, `pack_install`
mechanics, console facts); `hardware-tests.md`: "Sync one game" updated for compression (import
line, `.7z` size, `compress --off/on`, plan numbers, the `which tiny7zx` check, the sync
numbers, a results row) and a new "Base mods" checklist under the flow section.

## Verified

- `cargo test --workspace`: 312 tests, 0 failed, no hardware or network. New: archive round trip
  (lossless, 4× smaller for a synthetic ROM, deterministic bytes, random data, empty input,
  garbage refused), the CE fixture `ce-001-CLV-U-ZJSQU.desktop` reproduced from a compressed
  game with only `country` set by the test (CE fills it from its own `snescarts.xml`, which the
  library does not have), uncompressed entry, `game.json` default, import with/without
  compression (archive content, `game_files` substitution, `set_compress` off/on, legacy
  library healing), `collect` with the `.7z` and `compressed` count, plan `needs_extractor`,
  sync replay with the `which tiny7zx` check first and the refusal when it fails, uncompressed
  library skipping the check; base mods catalog (names, URLs, payload rule, cache paths, md5),
  cache status / `ModNotCached` / `ModChecksum`, tar layout; flow unit tests (commands, flow
  sequences) and simulated-console tests: base mods flow (exact command order, files landing
  in the rootfs, install script not copied, upload dir removed, reboot, `which` after it,
  second run "already done" without reboot), memboot shell refused, no mods loaded refused,
  install + base mods (17 steps); Tauri import test adjusted for compressed sizes.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`,
  `cargo doc --no-deps` for the three crates, `--no-default-features` checks of flow, hmod and
  library, `pnpm check` (0 errors): clean.
- Real ROM: `import "Chrono Trigger.smc"` → `4194400 bytes packed to 2850539 (67%)` in 0.9 s
  (debug build, lzma at opt-level 3); CE's archive of the same file in the trace is 2 850 560
  bytes. `compress --off` removes the archive and rewrites the entry; `compress` rebuilds the
  same bytes (same md5); `--all` reports unchanged.
- Console (SNES Classic Mini EUR, attached, read-only): `which tiny7zx`, the rootfs copy and its
  md5, the mod registry, fonts, mounts (overlayfs with upperdir `/var/lib/hakchi/rootfs`),
  blacklist, memory. `plan` against it: `1 games (1 compressed), 21 firmware games, 2 folders in
  31 files (2973696 bytes on the console)`, `upload 31 files (2867841 bytes)`; CE's `.7z` at the
  same path is replaced (size/time differ), its `metadata.json`/`_mdmini.png` removed. **No
  sync and no mod install were run** (user absent; the console already has tiny7zx).
- Downloads: every catalog file fetched from the pinned commit with the expected md5 (curl and
  `cartload-hmod base-mods --fetch` into a temp cache); `cartload-flow --dry-run base-mods`
  before and after shows `3 of 3 files to download` then `cached`; `install --base-mods
  --dry-run` lists 17 steps; `--mods clovercon` refused.

## Decisions

- LZMA (method 1), like CE's writer; tiny7zx handles LZMA, LZMA2, BCJ, BCJ2, COPY. Header is
  LZMA-encoded as 7-Zip does by default.
- The ROM stays next to its archive in the library (CE deletes it). Costs disk on the host, keeps
  toggling instant and the hash/art code untouched.
- `compress` missing in `game.json` means on, as the brief's default; a sync then re-uploads the
  ROM as `.7z` and the `which tiny7zx` precondition protects a console without the mod.
- The extractor check is a precondition of `sync()` (nothing changed when it fails), not a tenth
  step, so the UI's step list and the verified order stay as they are.
- `InstallBaseMods` runs on the booted kernel and reboots, rather than CE's memboot-into-recovery
  route: fewer moving parts, no FEL, and the device script is built for live installs
  (`hakchi hmod install` does the same). Verification compares against the uploaded bytes (equal
  to the catalog md5 for real mods) so the simulated tests can use fake content.
- Only `tiny7zx` is installed by default ("what a compressed library needs"); `font_remount` is
  in the catalog and selectable because the brief named it, with its purpose documented.
- `rust-version` bumped to 1.93 for sevenz-rust2; the alternative was an older crate version.
- The driver's `.agent/` changes (`run.py`, task moves) stay uncommitted, as in earlier tasks.

## Left or uncertain

- Hardware: the "Sync one game" sync itself (sections 3 and 4) and the "Base mods" flow were not
  run. In particular untested on hardware: that the wrapper's `tiny7zx x` accepts the
  sevenz-rust2 archive (it is a standard single-entry LZMA 7z with an encoded header, the format
  7-Zip itself writes; CE's archives differ only by 21 bytes), the exact `packs_install` output
  lines, and whether the reboot after the mod install is strictly needed (kept because overlayfs
  does not promise to show upper-dir writes).
- `country` in the launch entry is still empty where CE writes `us`: CE takes it from its cart
  database, not from anything the library holds.
- `game_files` packs a missing archive synchronously; in the app this runs inside the command
  that lists the library the first time an old library is opened (about a second per SNES game).
- Mega Drive games are never compressed; `m2engage` would support it, but the console is out of
  scope anyway.
