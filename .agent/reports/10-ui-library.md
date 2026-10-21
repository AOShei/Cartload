# Report: 10-ui-library

## What was done

Library crate (`crates/library`):

- `menu.rs`: editing a tree by hand. `MenuPath` (entry indices from the root), `MenuItem`
  (a game by code or a folder by path), `Menu::entries_at` / `folder` / `find` /
  `game_count`, `move_items` (into any menu, at an index or after the last game, refusing a
  folder into itself, nested selections move once, moved items keep their tree order, the
  target's coordinates are kept in step with the removals), `add_folder` (with its back
  link), `rename_folder`, `remove_folder` (contents take the folder's place in the parent),
  `reconcile` (drops games that left the library, adds new ones to the root menu) and
  `ensure_back_links`. 7 new tests.
- `library.rs`: `LibraryConfig::custom_menu`, the edited tree stored in `library.json`.
  Absent, the automatic structure applies as before.

App backend (`src-tauri/src`):

- `library.rs`: `LibraryState` (one `Library` at `CARTLOAD_LIBRARY` or the user data dir,
  the hash database loaded once and downloaded into the cache when missing, a long-task slot
  so an import, an art fetch and a sync never overlap). Views: `GameView` (game + size +
  cover URLs), `LibraryView`, `MenuView` (entries, options, `custom`, system code, the cached
  firmware games with their names). `import` walks folders for ROMs and zips, imports each
  file on the blocking pool, emits a `library-import` event before and after every file with
  `ImportOutcome` (imported games with hash-database match, replaced flag and warnings;
  failed with the error; skipped), and returns the report; with an edited tree the new games
  go to the folder being viewed. `GameEdit` + `apply_edit` (name required, players 1 or 2,
  date as `YYYY-MM-DD` via `art::valid_date`, empty publisher back to `UNKNOWN`,
  simultaneous only with 2 players). `menu` / `menu_view` (edited tree reconciled on every
  read, else `menu::build`), `set_structure` (drops the edits), `move_items` and
  `apply_menu_edit` (an automatic tree becomes an edited one on the first change),
  `ensure_originals` (caches the firmware's launch entries once per console, over the
  session), `fetch_art` (key from the settings or `CARTLOAD_TGDB_KEY`, `art-progress`
  events with a running count, the crate's report plus its summary line, key and allowance
  errors reworded), `set_cover` from a file, `key_status` / `set_key`. The covers are served
  by a `cover://localhost/<code>/<file>?v=<n>` URI scheme protocol registered in `lib.rs`
  that only answers `<code>.png` and `<code>_small.png` inside a game directory (CSP in
  `tauri.conf.json` allows `cover:` and `http://cover.localhost`).
- `sync.rs`: `SyncPlanView` (uploads, deletions, links, empty dirs, sizes, fits, noop,
  `changes`, the console's storage), `SyncSnapshot` (state, plan, step with a label, upload
  progress, last log lines, report, error) and `SyncRunner`: `plan()` reads the console's
  listings over the open session (read only, caching the firmware entries first, saving
  `last_console`) and diffs the exported tree; `run()` takes the library slot, pauses the
  watcher, publishes `ConsoleStatus::Working { flow: "sync" }` per step so the status bar
  reads "Syncing games: step 5 of 9, upload the games…", holds the session for the whole
  run, plans again against the console as it is now, refuses when the collection does not
  fit, runs `cartload_library::sync` (upload events coalesced to one every 150 ms), emits
  `sync-progress` on every change, then re-identifies the console so the free space is
  fresh. The work continues if the webview stops waiting.
- `commands.rs`: `library_list`, `library_import(paths, folder)`, `library_update(edit)`,
  `library_remove(codes)`, `menu_get`, `menu_set_structure(options)`, `menu_move(items,
  target, index)`, `menu_edit(edit)`, `art_fetch(codes, force)`, `art_set_cover(code,
  path)`, `tgdb_key_get`, `tgdb_key_set(key)`, `sync_plan`, `sync_run`, `sync_status`.
  `status.rs`: the working sentence names the Library page for a sync.
- `examples/plan.rs`: the backend without the UI: lists the library and the tree, waits
  for the console and prints the plan (nothing is synced).

Frontend (`src`):

- `api/types.ts` and `api/commands.ts`: mirrors of the payloads and wrappers.
- `state/library.svelte.ts`: games, tree, the folder being viewed, multi-selection (click,
  Ctrl, Shift ranges, select all), grid/list mode (localStorage), filter, import progress,
  art progress, a `revision` bumped on every change; the tiles of a folder as the console
  orders them (left folders, games by sort title, right folders; back links implicit);
  native open dialogs for games, folders and images; file drops through Tauri's
  `onDragDropEvent` (the page switches to the Library); `move`, `edit`, `setStructure`,
  `save`, `remove`, `fetchArt`, `chooseCover`. `state/sync.svelte.ts`: plan (debounced
  400 ms after a change while connected, forgotten when the console leaves), snapshot
  events, review / run / dismiss, a 250 ms clock while running.
- `components/library/`: `FolderTree` + `TreeNode` (recursive tree of folders with counts,
  drag and drop onto folders and the root, drag of folders, Up/Down/Home/End/Left/Right/F2,
  new folder and inline rename, remove with an inline confirmation worded "Remove folder",
  and the structure popover: one list / A to Z / folders of N games, keep the built-in games
  in the main menu, a warning that applying replaces hand-made folders); `GameGrid`
  (breadcrumbs that are drop targets, counts, filter, grid/list toggle, Add games… / Add a
  folder…, cover tiles with lazy images and a "No art yet" mark, built-in tiles, folder
  tiles that open on double-click or Enter and accept drops, roving tabindex with arrows,
  Ctrl+A, Escape, Backspace; the list view as a table); `GameDetail` (cover with Fetch art
  and Choose image…, art progress line, name / sort name / publisher / release date /
  players and "at the same time" / description, Save and Revert appearing when dirty,
  Advanced disclosure with the launch command, the rendered command line and the file
  facts, Remove with inline confirmation; a folder or built-in game summary; a multi-select
  panel with bulk Fetch art, Move to folder and Remove N games); `SyncBar` (console meter
  with used and planned, library size, the pending sentence, the one accent Sync button;
  the plan panel with uploads and removals under disclosures, Sync now / Cancel; the
  progress panel with step, bar, current file, last console line, Full log; done and failed
  results with Done / Try again); `ImportPanel` (progress bar and per-file outcomes,
  unmatched hashes in amber); `EmptyState` (connect, install hakchi, add games, each ticked
  from the status; Add games… is the primary action).
- `pages/LibraryPage.svelte`: tree / grid / detail columns over the sync bar, the drop
  overlay, the empty state; `App.svelte`: `main` is a flex column so the page can fill it.
  `SettingsPage`: library location and the TheGamesDB key (store, replace, remove,
  requests left). `dev/preview.ts`: `?page=library&library=some|empty|many&sync=plan|
  review|uptodate|running|done|failed` fixtures with inline SVG covers.
- `docs/hardware-tests.md`: "Library page (desktop app)" checklist, with the headless plan
  recorded.

## Verified

- `cargo test --workspace`: 298 tests (library 91, app 54+), among them the menu editing
  operations, imports walking folders with per-file events and a broken file failing on its
  own, edits checked and saved, automatic tree until edited then following the library
  (new games into the chosen folder, removed games pruned, rename / remove folder, apply a
  structure drops the edits), the busy slot, the cover protocol refusing anything but game
  art and path tricks, key storage, payload shapes, snapshot folding and log cap, plan and
  run refused without a console with the slot released. `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm check` (0 errors,
  0 warnings), `pnpm build`: clean.
- Headless against the attached SNES Classic Mini (EUR), read only: `cargo run -p
  cartload-app --example plan` connected in 2.4 s, cached the 21 firmware launch entries,
  and planned 31 uploads (4 236 052 bytes), 5 removals and 21 stale links for the one game
  in the real library, fitting in the free space; the same figures as task 06's CLI plan.
- The debug app ran 25 s with the console attached (`WEBKIT_DISABLE_DMABUF_RENDERER=1`):
  library opened, console identified, no errors from the new state or the protocol.
- Headless Chrome screenshots of the Vite previews (half scale): the populated page with the
  tree, built-in tiles and the bar; the plan review panel (uploads and removals listed, Sync
  now); the running panel (step 5 of 9, bar, current file); the empty state. A first
  screenshot showed the preview plan lost at mount, because the page's effect invalidated it
  before the console status existed; fixed (invalidate only once a report exists).

## Decisions

- The edited tree is stored whole in `library.json` and reconciled with the library on every
  read; an automatic structure stays computed, so new games fall into place until the first
  manual change, which snapshots it. "Apply" in the structure popover is the way back.
- Folders have no ids; the UI addresses them by index path, games by code. Reorder within a
  menu is accepted but the console sorts games by title anyway; folder positions stay as
  built (new folders are "right").
- Covers go through a custom URI scheme rather than the asset protocol, so the library
  location is not baked into the config and only game art is reachable.
- Long tasks are exclusive (one of import, art, sync) rather than queued.
- "Fetch art" on one game forces a refetch; the bulk action only fills games still without
  art, to spare the TheGamesDB allowance.
- Internal drag and drop uses HTML5 DnD; file drops come from Tauri. On Windows the two are
  known to conflict (Tauri's drop handler takes over HTML5 drags); "Move to" in the detail
  panel is the fallback there, and task 11 can revisit.
- No sync cancel: the crate's sync has no cancellation point, and a run takes seconds on USB.

## Left or uncertain

- The sync was not run from the app (user absent, console attached); the checklist covers
  it. Likewise art fetching against the live API (no key at hand), the native dialogs and
  the file-drop overlay were not exercised in the window.
- Selection, drag and drop and keyboard handling were checked by reading and by the preview
  screenshots, not by driving the window (no input tool on this host).
- The `.agent/tasks/*` moves and the driver's edits stay uncommitted, as in earlier tasks.
