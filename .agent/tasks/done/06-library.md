# Task 06: crates/library, the games library and sync

Read `docs/device-protocol.md` (games model and sync sequence) and
`docs/reference/hakchi-ce-sync-trace.log`. Reference C# under `~/Documents/Hakchi2-CE/hakchi_gui`:
`NesApplication.cs` and `Apps/` (per-system game classes, the SNES `.sfrom` conversion and
`.desktop` entry generation), `NesMenuFolder.cs` and `NesMenuCollection.cs` (folder tree and the
automatic split rules), `Tasks/SyncTask.cs` (the sync), `data/desktop_entries/` (original game
entries per system and region). Keep the console-side formats byte-compatible with what hakchi2
CE produces; the console's menu has to accept them.

Implement:

- `Library` on disk at a user data directory: one directory per game (`CLV-U-XXXXX`, same code
  generation scheme as hakchi2 CE so IDs stay unique), a `game.json` with metadata, the ROM in the
  console's expected container (SNES: `.sfrom`, with the header fields the stock emulator needs;
  NES: `.nes`), art PNGs, and the generated `.desktop` entry. Start with SNES and NES. Mega Drive
  can be a stub with a clear error.
- `import(path)`: accept `.sfc`, `.smc`, `.nes`, and zips of those; strip copier headers; compute
  the hashes hakchi2 CE uses; look up `romfiles.xml` (download from
  `https://github.com/TeamShinkansen/hash-db/releases/download/data/romfiles.xml.gz`, cached)
  for name, publisher, release date, players, and the console-specific fields; fall back to the
  file name. Return the new game.
- Folder tree: a `Menu` model (folders and games, with the `CLV-S-XXXXX` codes generated on
  export), plus the same automatic structures hakchi2 CE offers (flat, split by letter, split by
  count), expressed as pure functions over the model so a UI can show the result before syncing.
- `sync_plan(menu, remote_listing)`: a pure function producing the list of uploads, deletions and
  the link script, using the remote `find/stat` listing format from the trace. Then
  `sync(session, plan, progress)` executing it over `cartload-console` exactly in the verified
  order (umount, diff, FTP upload, cleanup, link script, config, overmount_games + uistart).
- CLI binary `cartload-library`: `import <file>`, `list`, `menu --structure flat|letter|count:N`,
  `plan` (prints the sync plan against a console if reachable, else against an empty listing),
  `sync`.
- Tests: hash and header handling with synthetic ROM bytes, sfrom header layout against the C#
  constants, desktop entry text equal to a sample from `data/desktop_entries/`, menu structuring,
  sync plan from the listing in the trace. No hardware in tests.

Verify, add a "Sync one game" checklist to `docs/hardware-tests.md`, commit.
