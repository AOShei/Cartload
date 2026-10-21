# Task 10: the Library page

Depends on tasks 06, 07 and 08. Follow `docs/design.md`. This is the page people live in.

- Backend commands over `cartload-library`: `library_list`, `library_import(paths)` with
  progress events per file, `library_update(game)`, `library_remove(ids)`, `menu_get`,
  `menu_set_structure`, `menu_move` (drag and drop reorder across folders), `art_fetch(ids)`,
  `sync_plan`, `sync_run` with progress events.
- Library page layout: left, the folder tree the console will show (editable, drag and drop,
  automatic structure options in a popover); centre, a grid of cover tiles for the selected
  folder, with a list view toggle; right, a detail panel for the selected game (editable name,
  sort name, publisher, release date, players, art with "fetch art" and "choose image", and the
  launch command behind an "advanced" disclosure). Multi-select for bulk art fetch and removal.
- Import by drag and drop of files or folders onto the window, or a native open dialog. Show
  per-file results, including unmatched hashes.
- A persistent sync bar at the bottom: space used on the console vs. this library (sizes come
  from the console info), the number of changes pending, and one "Sync" button that first shows
  the plan (uploads, deletions) and then runs it with progress.
- Empty state on first run that explains the three steps (connect, install kernel, add games).

Verify as in task 08; with the console attached and the user absent, do not sync. Commit.
