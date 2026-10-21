# Task 15: art for the built-in games

## Why

The Library grid shows a glyph for the 21 built-in games. The firmware ships a cover and a
thumbnail for each of them, next to the menu entries the library already caches on first
contact with a console: `/var/squashfs/usr/share/games/<code>/<code>.png` and
`<code>_small.png` (42 files, about 1.3 MB on a SNES Classic). Showing them makes the grid
look like the console's own menu. This is the last task the API credit covers, so keep it
small and commit as soon as it works.

## Do

1. When the library caches a system's original entries (see how `originals/<system>/` is
   filled today), also fetch the two PNGs per game, read-only, over the existing SSH or FTP
   session, into `originals/<system>/<code>.png` and `<code>_small.png`. Skip files already
   present. A failed fetch leaves the glyph; it must not fail the connection or the listing.
2. Serve them through the cover scheme: the handler currently resolves only `library/<code>/`
   for `CLV-U` codes; let it resolve `originals/<system>/` for built-in codes (the game view
   knows its system). Keep the path checks as strict as they are now.
3. Show them for "original" tiles in the grid, the list view's thumbnail and the detail
   panel, falling back to the glyph when the file is missing.
4. Tests for the path resolution and the fetch-skips-existing logic with the existing
   simulated console; `pnpm check`, clippy and the app tests green.

## Do not

- Write anything to the console.
- Fetch art from TheGamesDB for built-in games; the firmware art is the right art.

## Verify

Dry: tests and a preview screenshot (downscaled) of the grid with original tiles carrying
art. Live, read-only: with the console attached, delete `originals/snes-eur/*.png` locally,
start the app for 20 s, and check the 42 files appear. Report to
`.agent/reports/15-originals-art.md` and commit.
