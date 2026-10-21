# Task 11: design pass and polish

Depends on tasks 08 to 10. Read `docs/design.md` and audit every page against it. Then:

- Identity: a proper wordmark and app icon derived from the cartridge glyph (inline SVG,
  regenerate the icon set), an about panel with version, license and credits (hakchi by
  madmonkey, hakchi2 CE by TeamShinkansen, TheGamesDB).
- Typography and spacing scale defined once as CSS custom properties; consistent 8 px rhythm.
- Motion: short, purposeful transitions for page changes, toasts, step state changes; respect
  `prefers-reduced-motion`.
- States: every async action has loading, success and error presentation; every list has an
  empty state with a next action; every destructive action has an inline undo or a clear
  confirmation (not a modal `confirm()`).
- Accessibility: focus rings, labels on icon buttons, contrast checked in both themes,
  full keyboard operation of the tree and grid.
- Window: remembers size and position; minimum size that keeps the layout intact.
- Performance: the grid stays smooth with 500 games (virtualise if needed); images lazy-load.
- Write `docs/screenshots.md` with instructions to capture screenshots on X11 and Wayland
  (`grim`, `gnome-screenshot`, or `spectacle`, whichever is installed) and capture them if a
  tool is available.

Verify everything still passes, then commit.

Additional items found during review:
- Native Wayland on this host crashes WebKitGTK with a DMA-BUF protocol error unless
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` is set. Set it from `main.rs` on Linux before the Tauri
  builder runs (only when unset, so users can override), and note it in the README.
- Clippy on Rust 1.96 flags seven `chunks_exact` calls with a constant size in
  `crates/library` (`as_chunks` is the suggestion). Fix them so CI can move off the 1.95 pin.
- `Cargo.toml` names a repository that does not exist; it is `https://github.com/AOShei/Cartload`.
