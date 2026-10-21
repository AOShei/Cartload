# Task 14: single-page layout: panels scroll, the page never does

## Why

The user ran the app at 1990 × 1017 on GNOME and reviewed every page. The look is approved.
The layout is not: every page scrolls as a whole, columns are sized for a narrow window, and
wide windows leave a void on the right. Their words: "Elements in a page can scroll but not
the page itself." Treat that as the rule for the whole app, including when the log panel at
the bottom is open: the log must shrink the content area, never push it off screen.

Budget is small and this is the last task the API credit covers. Work in the order below,
commit after each numbered item with its own message, and stop cleanly before the budget runs
out: a report listing what is done and what is left is worth more than a half-finished item.

## Do, in this order

1. **App shell.** The window is a fixed viewport: sidebar, page header, status bar and the
   log panel are fixed; the page content area is the only flexible region and is itself
   `overflow: hidden`. Each page declares which of its regions scroll. Verify with the log
   panel open and closed at 900 × 600, 1280 × 800 and 1990 × 1017 with the `?preview=`
   fixtures and headless Chrome screenshots (downscale them before reading, keep each under
   1 MB).

2. **Library page.** Three columns, all full height, only their inner lists scroll:
   - Folder tree: minimum 240 px so "Console menu" and "More games..." are not truncated;
     names ellipsise only below that.
   - Game grid: flexible; scrolls.
   - Detail panel: 340 px minimum (it is about 180 px now and the fields are squeezed).
   - Description: a fixed-height preview of the first three lines, read-only, with an "Edit"
     button in its corner (or a click on the preview) that opens a dialog with a large
     textarea, Save and Cancel. Escape cancels. This is a deliberate exception to the "no
     modal dialogs" rule in `docs/design.md`: editors may open in a dialog; note it there.
   - "Advanced": do not push the fields down. Open it as a dialog too, or as a popover
     beside the panel, whichever is less code.
   - Sync bar: keep "Sync N changes" but explain N. Show the breakdown that is already
     computed (files to upload with size, files to remove, links to reset) and add a "Show
     changes" disclosure or dialog listing the paths. On a console that CE synced, the first
     Cartload sync replaces CE's files; one sentence in the bar should say so when removals
     are present.

3. **Kernel page.** Use the width. Two columns on windows wider than about 1100 px: the
   three stage cards (FEL, package, install button) on the left, the step list with the run
   log and the result card on the right; the step list and log scroll inside their column.
   Below that width, stack as now but still without page scroll. Stage 4 ("Result") must not
   repeat the Console page's details table: a compact card with the one-line console summary,
   the elapsed time and "Open Console" / "Open Library" is enough.

4. **Console page.** "Everything detected, as data" currently runs off the bottom of the
   page. Put the summary card and the Detected table in the left column and the JSON in a
   scrollable panel on the right (wide windows) or in a fixed-height scrollable box (narrow).

5. **Settings page.** Two columns on wide windows: the settings cards on the left, the About
   card on the right, each column scrolling on its own so the About card is never cut off.

## Keep

- The visual identity, tokens, motion and wording. This is layout only.
- `pnpm check` with zero errors and warnings, clippy and the app tests green.
- Keyboard paths: dialogs trap focus and restore it on close.

## Verify

Screenshots of every page in the three sizes above, light and dark, with the log open on at
least one. Update `docs/hardware-tests.md` "Window and polish" with what was checked. Report to
`.agent/reports/14-layout.md` listing the items done and, explicitly, the ones left.
