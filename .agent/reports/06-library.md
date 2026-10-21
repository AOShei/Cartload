# Report: 06-library

## What was done

`crates/library` (`cartload-library`) is the games library and the sync. Layout:

- `src/rom.rs`: `System` (SNES, NES, Mega Drive) from extensions; `prepare(name, bytes)`:
  copier header removal (`len % 1024 != 0`, 512 bytes), CRC-32 (headerless ROM for SNES, PRG+CHR
  for NES, plus the whole-file CRC), hakchi2 CE's `GenerateSafeFileName` and title cleaning,
  the SNES internal header with CE's LoROM/HiROM detection order, and the `.sfrom` container:
  48-byte header (`0x100`, sizes, `rom_end + 27`, `WUP-XXXX`) + ROM + 48-byte packed footer
  (60 fps, preset id, 2 controllers, volume `0x5A`, `0x14`/`0x15`, chip flag, the two `1`s),
  CE's preset table (40 titles), DSP-1 → `0x10BD`, SA-1 → `0x109C`, Super FX → chip `0x0C`,
  SRAM → 3 save slots, CE's problem-game list as a warning. NES: iNES header parsing as CE's
  `NesFile` (archaic headers cleaned), normalised header, unsupported mapper / four-screen /
  trainer warnings, battery → 3 saves. Mega Drive returns `Error::MegaDriveUnsupported`.
- `src/codes.rs`: `game_code(crc32, prefix)` (five letters from 5-bit groups mod 26, `U`/`H`/`G`),
  `folder_code(index)`, code shape checks.
- `src/desktop.rs`: `DesktopEntry` parse/render in two formats: `Stock` (the firmware's layout)
  and `Hakchi` (CE's `GetConfig`: `CePrefix`, `[m2engage]`, `[Description]`), `binary()`,
  `relocate()`, `set_binary()`, the apostrophe-digit fix and the sort key rule.
- `src/hashdb.rs`: `HashDb` from `romfiles.xml` / `.gz` (quick-xml; first entry per CRC wins),
  `fetch()` of `ROMFILES_URL` into `<cache>/cartload/romfiles.xml.gz` (atomic, validated).
- `src/game.rs`: `Game` (`game.json`) with CE's defaults (1 player, `1900-01-01`, `UNKNOWN`)
  and `desktop_entry(games_path)`.
- `src/images.rs`: deterministic 228×204 / 40×40 placeholder PNGs for folders and games.
- `src/menu.rs`: `Menu`/`Entry`/`Folder`/`BackLink`, `Position` with CE's sort fields
  (`Э`/`Ю`/`Я` prefixes, `7777-77-77`...), `Structure::{Flat, Letters, Count{max}}` parsed from
  `flat|letter|count:N`, `MenuOptions { structure, originals_at_root }`, pure `build()` (CE's
  `Original_*` behaviour: originals at root, library in "More games..." with an "Original
  games" back link; CE's equal split with `TrimFolderNames`; A–Z/# folders) and `export()`
  (indices in CE's pre-order, `CLV-S-<index>` codes, empty folders dropped,
  `ExportedFolder::desktop_entry` = CE's `chmenu` entry).
- `src/library.rs`: `Library` at `<data dir>/cartload/library` (or any root): `import(path, db)`
  for ROMs and zips (one game per ROM inside), `games()`, `game()`, `save()`, `remove()`,
  `game_files()`, `library.json` (`LibraryConfig { menu, last_console }`), a cache of the
  firmware's launch entries per system code (`originals/<code>/CLV-P-*.desktop`).
- `src/sync.rs`: `DevicePaths` (from `ConsoleInfo` or a `ConsoleType`), `parse_listing()` for
  the `find`/`stat` format, `collect()` building CE's linked layout (`.storage/<code>/` once per
  game, one `.desktop` per menu entry, folder icons, firmware entries relocated into the
  squashfs with the `-wr` wrappers, the link script), pure `sync_plan()` (uploads by
  path+size+mtime within 3 s, deletions, stale symlinks, emptied directories), `SyncPlan`
  scripts (`cleanup.sh`, `modtime.sh`, config), `Transport` trait (implemented by `Session`:
  SSH exec + FTP `MKD`/`STOR`), `sync()` in the verified order, `fetch_originals()`, UTC time
  conversion without a date crate. Generated files get a content-derived mtime so unchanged
  entries are not re-uploaded.
- `src/main.rs`: `cartload-library [--library DIR] import <FILE>... [--db FILE|--no-db]`,
  `list [--json]`, `remove <CODE>...`, `menu [--structure] [--originals-at-root] [--console]
  [--save] [--json]`, `plan [--host] [--json]` (against the console when one answers, else an
  empty console of `--console`/last type), `sync [--host] [-y]`, `fetch-db`.
- `docs/hardware-tests.md`: "Sync one game" checklist with the plan output observed.
- Test fixtures (`tests/fixtures/`): the stock SNES and NES entries from CE's
  `data/desktop_entries`, the four entries hakchi2 CE uploaded to this console, and the
  console's real `find`/`stat` listing (all read from the console in this task, read-only).

## Verified

- `cargo test -p cartload-library`: 55 tests, no hardware or network. Synthetic SNES and NES
  ROMs (header detection, title rules, `.sfrom` byte layout against CE's struct constants and
  the trace's 4 194 400-byte size, presets/chips, copier header, safe names, NES CRC and
  normalisation), the Chrono Trigger code `CLV-U-ZJSQU` from CRC `2D206BF7`, stock SNES and NES
  entries re-rendered byte for byte, the Hakchi format and the folder/original/game entries
  equal to the files CE uploaded (byte for byte for the original game and the two folders; the
  game entry differs only by CE's `.7z` suffix and `country=us`), hash database parsing (gz and
  plain), placeholder image sizes, menu structures (the trace's layout, letters, count with CE's
  name trimming, nested export, empty folders), the listing from the console (35 files), the
  plan against it (28 deletions, 21 symlinks, emptied dirs deepest first), no-op and drifted
  plans, and a scripted `Transport` asserting every command string and stdin of a sync in order.
- `cargo test --workspace` (209 tests), `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `cargo doc -p cartload-library --no-deps`: clean.
- Real ROM: `import` of `~/Documents/Games/SNES/Chrono Trigger.smc` gives `CLV-U-ZJSQU`,
  `Chrono_Trigger.sfrom` of 4 194 400 bytes, matched in the hash database (`tgdb 1255`), as in
  the CE trace. The live `romfiles.xml.gz` (518 698 bytes) is identical (md5) to CE's copy.
- Console (SNES Classic Mini EUR, attached, read-only): `plan` connected, cached the 21 firmware
  entries and produced 31 uploads whose entry sizes equal CE's files on the console (e.g.
  `CLV-P-SAAAE.desktop` 600 bytes), plus the removal of CE's `_mdmini.png`, `metadata.json`,
  `.7z` and the autoplay symlinks. `sync` was **not** run (user absent).

## Decisions

- The hash database has only names, CRCs and TheGamesDB ids (checked on the live file); the
  brief's publisher/date/players are not in it. They default to CE's values and `tgdb_ids` is
  stored for task 07 to fill them from TheGamesDB. CE's `snescarts.xml`/`nescarts.xml` were not
  copied.
- Layout is CE's default linked sync (`.storage` + per-menu entries), read back from the console.
- Launch entries use CE's full format (verified byte for byte against the console); the stock
  format is kept for parsing and round-trip tests.
- Deletions are plan-driven: every removed file, symlink and directory is named in the plan and
  removed by path. CE's blanket removal of non-system directories under the games root is not
  replicated (Cartload has no legacy layout to migrate), and the "remove empty directories"
  pass is computed from the listing instead of a device-side loop.
- No ROM compression (CE's 7z); files go up uncompressed. The `uistop` of the trace is sent, the
  framebuffer splash is not.
- `stat %y` on the console prints UTC without a zone; the clock is UTC; times are handled as UTC.
- Mega Drive is a typed error. `.sfrom` input files are stored unchanged.

## Left or uncertain

- Hardware run of the "Sync one game" checklist (sections 3 and 4): not executed.
- NES ROMs were tested with synthetic images only; whether hash-db keys NES CRCs on headered or
  headerless files is unknown, so both are looked up.
- Cover art is a placeholder (task 07); the UI's custom folder trees (task 10) can use the
  `Menu` model but nothing persists a custom tree yet.
- The driver's `.agent/tasks/*` moves stay uncommitted, as in earlier tasks.
