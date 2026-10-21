# Report: 14-layout

All five items are done, each in its own commit, plus a docs commit.

## What was done

1. **App shell** (`5d0e642`). `main` is `overflow: hidden`; the sidebar, page header, status
   bar and log panel are fixed and the keyed page wrapper is the only flexible region. Each
   page declares its scroll regions. The log panel is `flex: none` with a height of
   `clamp(144px, 28vh, 224px)` so it shrinks the content area and never pushes it off screen.
   `?log=open|closed` forces the panel for previews (unstored, like `?theme=`).

2. **Library** (`28fbc5f`). Columns `minmax(240px, 13vw) / 1fr / minmax(340px, 20vw)`, all
   full height, only the tree, the grid viewport and the detail panel's inner scroll move.
   The cover no longer collapses to 0 px in a short panel (`flex: none`). The description is
   a three-line read-only preview (one button: the whole block, with an "Edit" tag in its
   corner) that opens a dialog with a 14-row textarea, Save and Cancel; "Advanced…" opens the
   launch command, arguments, compression, the command preview and the facts in a second
   dialog. Save writes the draft into the form and saves the game; Cancel, Escape, the close
   button and a backdrop click drop it. `Dialog.svelte` is a thin wrapper on the native
   `<dialog>` (`showModal()`): focus trap, inert page and focus restore come from the browser;
   `data-autofocus` names the control that takes focus; a `cancel` event closes through the
   `open` prop so state and element never disagree. `docs/design.md` principle 3 now names
   editors as the one dialog exception. The sync bar's Changes block explains N ("7 changes:
   3 files to upload (6.0 MB), 2 files to remove, 1 link to reset, 1 empty folder to remove"),
   a "Show changes" button opens a dialog listing the paths (uploads with sizes; removals
   tagged file / link / folder) with the sync button at its foot, and when files or links are
   to be removed one sentence says that a console synced by hakchi2 CE has its files replaced
   by this library. The review panel reuses the same lists through a snippet. The bar row
   wraps at narrow widths so the explanation reads as a line, not a stack of words. Preview
   switches: `?select=N`, `?dialog=description|advanced|changes`.

3. **Kernel** (`5b0ced9`). From 1100 px: a grid with the stage cards (FEL, package, install)
   on the left and, on the right, a "progress" card holding the step list (scrolls) and the
   run log (`FlowLog grow` fills the remaining height and scrolls) above the stage 4 card.
   The left column scrolls on its own. Below 1100 px everything stacks in one scrolling
   region; the page itself never scrolls. The FEL illustration stacks above its steps when
   the stage card is narrow (narrow windows and two-column windows under 1500 px). Stage 4 is
   compact: outcome heading and sentence, the console's one-line summary (`report.title`
   with its tone dot) and the elapsed time, and Open Console / Open Library (the primary one
   is the next step: Library after an install that left the console connected). `StatusCard`
   and `DetailsList` are no longer imported by the Kernel page.

4. **Console** (`d84ff89`). From 1100 px: status card, Detected table and the development
   command card in a scrolling left column; "Everything detected, as data" is a card on the
   right (`minmax(340px, 34%)`) whose `<pre>` scrolls inside the column. Below 1100 px one
   scrolling column with the data in a 320 px box. The data is no longer behind a disclosure.

5. **Settings** (`11acb49`). From 1100 px: the settings cards on the left, the About card on
   the right, each column `overflow: auto`. Below, one scrolling column as before.

Docs: `docs/hardware-tests.md` "Window and polish" has a section 4 (layout) checklist and a
results row for this run; `docs/screenshots.md` documents the new preview switches and the
Vite restart; the seven `docs/screenshots/*.png` were regenerated from the new layout at
1280 × 800, half scale.

## Verified

- `pnpm check`: 0 errors, 0 warnings after every item. `cargo clippy --workspace
  --all-targets -- -D warnings` and `cargo test --workspace`: clean (no Rust changed).
- Headless Chrome on the preview fixtures, every page at 900 × 600, 1280 × 800 and
  1990 × 1017, log open and closed (24 combinations) measured through the DevTools protocol:
  the document and the page area never overflow in either axis; the only scrolling elements
  are the tree, the grid viewport, the detail panel, the Kernel columns / step list / log, the
  Console column and data panel, the Settings columns. Screenshots of the same set in light
  and dark, read at reduced scale as montages: no clipped content, the sync bar and dialogs
  as described.
- Dialog focus path by script: clicking the description preview opens the dialog with focus in
  the textarea, the page behind is inert, a `cancel` event closes it and focus is back on the
  preview.
- The real WebKitGTK window (`GDK_BACKEND=x11 pnpm tauri dev --no-watch`, captured with
  `import -window Cartload` at 2560 × 1051, maximised by the window-state plugin) with the
  SNES Classic Mini attached and identified, read only: the Console page renders the two
  columns, the data panel ends at the window's bottom and scrolls inside itself, the status
  bar stays put. Only the Console page could be reached (no input tool).
- Stale Vite: twice a component's old CSS was served after an edit (the HMR updated the
  module but a fresh load still got the old style module). Restarting `pnpm dev` fixed it;
  noted in `docs/screenshots.md`.

## Decisions

- Dialogs are the native `<dialog>` rather than a hand-made trap: fewer lines, and the trap,
  inert page and focus restore are the browser's. WebKitGTK has supported it since 2.36.
- The dialog's Save saves the game (not only the form): "Save" should mean saved, and the
  panel's own Save/Revert row still covers the inline fields.
- The sync "Show changes" is a dialog, not a disclosure in the bar: the bar is the one place
  that must stay short, and the review panel already is the inline listing.
- The Kernel result keeps `report.title` as the summary; the Console page has the table.
- The two-column threshold is 1100 px for all three pages, as the brief suggests; the Library
  keeps its own 1024 px sidebar breakpoint.

## Left or uncertain

- Not driven in the real WebKitGTK window: the keyboard paths through the dialogs and the live
  step list and log during a run (no input tool on this host; listed in the checklist).
- At the 900 × 600 minimum with the log open the Library columns are about 110 px tall and the
  grid shows one row: nothing is cut, but that size is a floor, not a working size.
- At 900 px wide the grid gets about 170 px (one tile column) because the tree and the detail
  panel take their minimums as the brief asks.
