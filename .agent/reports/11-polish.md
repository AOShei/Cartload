# Report: 11-polish

## What was done

Audit of the four pages against `docs/design.md`, then:

**Identity.** `src-tauri/icons/icon.svg` is a new drawing: the cartridge (body, grip notch,
label, edge connector) in white on the accent tile with a slight vertical gradient; the icon
set (`32x32`, `64x64`, `128x128`, `128x128@2x`, `icon.png`, `icon.icns`, `icon.ico`, the
`Square*Logo` and `StoreLogo` PNGs) was regenerated with `pnpm tauri icon`; the old
`source.png` and the mobile sets are gone. `Wordmark.svelte` is a wordmark built from the same
round monoline strokes as the glyph (geometric lowercase: circles for the bowls, round caps;
`viewBox 0 0 115 22`), so it needs no font and renders identically everywhere; `AppMark.svelte`
is the icon as inline SVG. The sidebar brand (glyph + wordmark) is a button that opens the
About panel on the Settings page: version (with the debug mark), the GPL-3.0-or-later line and
the repository, credits for hakchi (madmonkey), hakchi2 CE (TeamShinkansen) and TheGamesDB with
their URLs as selectable text, the Tauri / Rust / Svelte line, and the linked crates under a
disclosure.

**Tokens.** `src/app.css` defines everything once: the colour scale, a six-step type scale in
pixels (`--text-xs` 11 to `--text-xl` 22), line heights and weights, the 4 / 8 px spacing
scale up to `--space-8`, control heights (`--control` 32, `--control-sm` 24, `--control-lg`
40) shared by buttons, inputs, toolbar rows, the sidebar items and the status bar, radii,
shadows and three motion durations with easings. Every component was swept: the nineteen
ad-hoc `font-size` values became the six tokens, rem paddings became spacing tokens, and the
shared pieces (`.btn`, `.btn-danger`, `.btn-sm`, `.icon-btn`, `.spinner`, `.dot.tone-*`,
`.confirm`, `.empty-note`, `.caps`, `.focus-inset`) moved to the stylesheet instead of being
redrawn per component. `scripts/contrast.mjs` reads the `light-dark()` pairs from the
stylesheet and prints the WCAG ratio of every text token on every surface in both themes;
the green, amber and red were darkened and the dark accent lightened until every pair passes
(4.5:1 for text, 3:1 for the faint tertiary text; the tables are in the script's output).

**Motion.** Page changes rise 6 px and fade in (`{#key}` + `fly`, 180 ms); toasts fly in
from the top, fade out and reflow with `animate:flip`; the step markers transition their
colours and pop when a step ends; the stage numbers, the status card edge, the sidebar items
and the tree rows transition their colours; menus and popovers fade in; the sync panels slide.
`src/lib/motion.ts` zeroes the Svelte transition durations under `prefers-reduced-motion`
and `app.css` zeroes the CSS durations and the looping animations (the cartridge pulse, the
FEL halo, the busy dots).

**States.** Loading: the library shows "Reading the library…", the package stage "Checking
the cache…", the key status "Checking the stored key…", and every button that waits on the
backend shows a spinner and its progressive label (Starting…, Stopping…, Fetching…,
Replacing…, Saving…, Removing…, Checking…). Errors: the library shows a card with the error
and "Try again" when it cannot be read; a failed package download stays in place with "Try
the download again"; everything else keeps its toast with the detail disclosure. Empty states
now carry a next action: the empty folder offers "Add games…", a filter without matches
offers "Clear the filter", the log panels explain what will appear. Destructive actions:
removing games (also by the Delete key in the grid), removing a folder (Delete in the tree),
returning the console to stock and removing the TheGamesDB key confirm inline, worded as the
action; applying an automatic structure over hand-made folders now asks "Replace the folders"
instead of only warning; clearing the log is undoable from its toast (toasts gained an
optional action, `toasts.undoable()`).

**Accessibility.** Focus rings are the accent outline; inside scrolling containers (tiles,
tree rows, table rows) they are drawn inward so they are not clipped. Every icon button has
an accessible name; the sidebar items keep theirs when the sidebar collapses to icons. The
tree has one Tab stop (the folder being viewed, whose ancestors are expanded automatically)
with arrows, Home, End, Left, Right, F2 and Delete; the grid has one Tab stop with arrows,
PageUp, PageDown, Home, End, Space, Enter, Backspace, Delete, Escape and Ctrl+A, and the
options carry `aria-posinset` / `aria-setsize` since not all of them are in the DOM. The
Kernel page's More menu focuses its first item when opened, takes arrows, and returns focus
to its button on Escape. The status card and the busy panels set `aria-busy`.

**Window.** `tauri-plugin-window-state` 2.5 restores size, position and the maximised state
(Rust side only, no capability needed). The default window is 1200 × 760 and the minimum
900 × 600: below 1024 px the sidebar collapses to icons and the main padding shrinks, so the
three library columns survive at 900 px. The library side columns are 184 / 256 px (168 /
240 when collapsed) so the grid gets three columns at the default size.

**Performance.** The cover grid is windowed: only the rows in view plus two rows of overscan
exist in the DOM (15 tiles of a 108-game folder at 1200 × 760; the `many` preview fixture
now has 500 games). The tile width follows the pane (128 to 200 px), the row height is
measured from the first rendered tile with a `ResizeObserver`, a sizer keeps the scrollbar
honest and the window is positioned with a transform. Keyboard moves scroll the target row in
first and focus after the tick; a focused tile that scrolls out of the window hands focus to
the listbox so the keys keep working. Images keep `loading="lazy"` and `decoding="async"`.

**Screenshots.** `docs/screenshots.md`: which tool to use on Wayland (grim, gnome-screenshot,
spectacle, or ImageMagick through XWayland with `GDK_BACKEND=x11`), on X11, macOS and
Windows, the headless Chrome route for the preview fixtures, and the list of states to
capture. `docs/screenshots/` holds seven half-scale captures: six from the preview fixtures
(console connected, kernel running, kernel failed, library with a 500-game folder, library
with the sync plan in dark, settings with the About panel) and one of the real WebKitGTK
window (`console-connected-dark.png`, captured through XWayland with `import -window
Cartload` while the SNES Classic Mini was connected). The previews take `&theme=light|dark`
and, for the library, `&folder=<index path>` (the 500-game fixture's largest folder is `4/1`).

**Review items.** `main.rs` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux before the
builder runs, only when unset (an `unsafe` block with a SAFETY note, allowed locally; the
README explains the override). The seven `chunks_exact(4)` calls in `crates/library/src/
images.rs` go through one `rgba()` helper over `as_chunks::<4>()`. `Cargo.toml` names
`https://github.com/AOShei/Cartload`. `docs/hardware-tests.md` has a "Window and polish"
checklist with a results row.

## Verified

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` (also
  `-p cartload-app --features dev-exec`), `cargo test --workspace` (298 passed, 0 failed),
  `pnpm check` (0 errors, 0 warnings), `pnpm build`, `node scripts/contrast.mjs` (all pairs
  pass): all clean on Rust 1.95.0. The 1.96 clippy lint itself could not be run here (no
  1.96 toolchain); the `as_chunks` form is what it suggests.
- Headless Chrome captures of the previews at 1024 × 700, 1200 × 760 and 900 × 600 in both
  themes, read at half scale: Console (connected, nothing found), Kernel (running, failed),
  Library (root, the 500-game folder, the plan in dark, the empty state, the minimum size with
  the collapsed sidebar), Settings with the About panel. A first pass showed the grid falling
  to one stretched column at 1024 px because the side columns left 232 px: fixed by the
  narrower columns, the capped tile width and the larger default window. `--dump-dom` of the
  500-game fixture confirms 15 rendered options of 108 in its largest folder (`folder=4/1`),
  and 500 games listed in the sync bar.
- The real app, three ways, with the SNES Classic Mini attached and running hakchi (read
  only; no flow or sync started): under `GDK_BACKEND=x11` (window captured with ImageMagick:
  the wordmark, sidebar, status card, details and status bar render as in the preview, dark
  theme followed from the desktop); natively on Wayland without any variable (the app's own
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` took effect: no protocol error, console identified in
  2.5 s, 35 s run); the window-state plugin compiled and registered without an error.

## Decisions

- The wordmark is drawn, not typeset: `docs/design.md` asks for a geometric sans, no such font
  is guaranteed on the three platforms, and embedding one would mean shipping an asset. The
  construction (circles of one radius, one stroke width, round terminals) is the glyph's
  language and matches the icon.
- The remove actions keep inline confirmations rather than undo: `Library::remove` deletes
  the game directory, and a trash would be a backend change beyond this brief. Undo was added
  where it is free (clearing the log); the apply-structure case got a confirmation.
- Type sizes are pixel tokens rather than rem so every size lands on the pixel grid at the
  15 px base; the webview's own zoom still scales them.
- The grid is windowed by hand (about 60 lines) rather than through a dependency: the tiles
  are all the same height, which makes the arithmetic trivial, and the brief allows no
  component library.
- The list view is not windowed: 500 one-line rows with lazy thumbnails render quickly, and
  windowing a table would need fixed row heights.
- Credits show URLs as selectable text rather than links: opening the browser needs the
  opener plugin and a capability, and `docs/design.md` wants no external assets at runtime.
- `source.png` was removed from `src-tauri/icons`: `icon.svg` is the single source and the
  README of the icon says how to regenerate.
- The `.agent/tasks/*` moves made by the driver stay uncommitted, as in earlier tasks.

## Left or uncertain

- Not driven by hand (no input tool on this host): the window state round trip (resize,
  close, reopen), the reduced-motion switch, the keyboard paths and the drag and drop. They
  are in the "Window and polish" checklist in `docs/hardware-tests.md`; the keyboard logic
  was checked by reading and by the DOM dumps (tab stops, `aria-setsize`).
- GNOME's own screenshot tools are not installed here and the Shell's D-Bus screenshot
  interface refuses third-party callers, so the Wayland-native window was not captured; the
  XWayland capture is the real window, the rest are the preview fixtures.
- Clippy 1.96 was not run (toolchain 1.95 is what is installed); CI stays pinned to 1.95
  until someone bumps it with the fix in place.
- The window-state plugin cannot restore a position on Wayland (compositors place windows);
  size and maximised state do restore.
