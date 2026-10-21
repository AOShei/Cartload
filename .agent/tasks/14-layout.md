
6. **Art for the built-in games.** The firmware ships a cover and a thumbnail for every
   built-in game, next to the menu entries the library already caches
   (`/var/squashfs/usr/share/games/<code>/<code>.png` and `<code>_small.png`, 21 games and
   42 files of about 1.3 MB on a SNES Classic). Fetch them read-only the first time a console
   of that system is seen, store them beside the cached entries under `originals/<system>/`,
   and serve them through the cover scheme so the grid and the detail panel show them for
   "original" tiles instead of the glyph. If the fetch fails the glyph stays. Never write to
   the console for this.
