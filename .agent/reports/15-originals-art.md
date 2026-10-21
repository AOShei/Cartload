# Task 15: art for the built-in games

## Done

- `crates/library`: `codes::is_system_code`, `codes::original_art_code` and
  `ORIGINAL_ART_SUFFIXES` (`<code>.png`, `<code>_small.png`); `Library::missing_original_art`,
  `store_original_art` (empty files skipped, names and system code validated) and
  `original_art` (path of a cached file, or `None`); `sync::fetch_original_art` reads the
  missing files in one read-only SSH command (`cd "<squashfs games>"; for f in ...; echo
  "==== $f <size>"; cat "$f"`), framed by size so PNG bytes never need escaping;
  `parse_art_blocks` drops a file cut short. The library CLI fetches the art too when it
  prepares a console.
- `src-tauri`: `ensure_originals` now runs the art fetch after the entries are cached (also
  when they already were), so an existing library gets the art on its next plan. A failure is
  logged and leaves the glyph. The cover scheme resolves `/<system>/<code>/<file>` for
  `CLV-P` codes from `originals/<system>/`; the two-segment library path is unchanged and the
  file must still be exactly `<code>.png` or `<code>_small.png`. `OriginalView` carries
  `cover` and `thumbnail` URLs (`null` when the file is missing).
- Frontend: the grid tile, the list thumbnail and the detail panel show the firmware art for
  "original" tiles and fall back to the glyph. The sync store reloads the tree once per
  connection when built-in games lack art, so the art appears right after the first plan.
  The preview fixture gives its four built-in games generated covers.
- Nothing is written to the console; no TheGamesDB lookups for built-in games.

## Verified

- `cargo test --workspace` (100 library tests, 54 app tests), `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --check`, `pnpm check`: all green.
- New tests: art file names and system codes (`codes`), the per-file cache round trip
  (`library`), `parse_art_blocks` sizes and truncation, and the fetch against the scripted
  console: it names only the three missing files in the exact command, stores the two the
  console had, sends nothing when nothing is missing (`sync`); the cover scheme serves the
  cached original and refuses the wrong system, code, file name, extra segments and
  traversal; `menu_view` gives the cover URL and no thumbnail URL (`src-tauri`).
- Preview (headless Chrome, `?page=library&library=some&preview=connected`, half scale,
  `.agent/reports/15-originals-art.png`): the four "Built in" tiles carry art.
- Live, read-only, SNES Classic Mini (EUR) attached: with no PNG in
  `originals/snes-eur/`, `cargo run -p cartload-app --example plan` (the documented headless
  equivalent of the Library page; the window starts on the Console page, where no plan runs,
  so a 20 s window start would not have fetched anything) logged
  `cached the firmware's art stored=42 wanted=42` three seconds after connecting; 42 valid
  PNGs on disk (covers 228 × 160, thumbnails 40 × 28, 1.6 MB). Row added to
  `docs/hardware-tests.md`.

## Left or uncertain

- The desktop window was not driven: the grid with the real files in WebKitGTK is checked by
  the unit tests of the scheme handler and the preview only. The firmware covers are
  228 × 160, so they letterbox slightly in the 228 × 204 tile.
- The art is fetched when a sync plan runs (Library page open), as the entries were already;
  nothing fetches on the Console page.
