# Cartload

Desktop app (Tauri 2, Rust, Svelte 5 + TypeScript) that installs the hakchi kernel on
NES / SNES / Mega Drive Classic Mini consoles and manages and syncs a games library.
Linux first, then macOS and Windows. License GPL-3.0-or-later.

This is a from-scratch implementation. Reuse the *knowledge* in the reference
implementation, not its code or its UI patterns.

## Reference material

- `docs/device-protocol.md`: everything known about the console side. Read it first.
- `docs/reference/hakchi-ce-sync-trace.log`: a verified log of a real add-game + sync run.
- `~/Documents/Hakchi2-CE` (read-only, GPL-3): the reference C# implementation, hakchi2 CE.
  Useful files: `Libraries/FelLib/FelLib/Fel.cs` (FEL protocol), `hakchi_gui/Tasks/MembootTasks.cs`
  (memboot and kernel install sequence), `hakchi_gui/Tasks/SyncTask.cs` (sync),
  `hakchi_gui/hakchi.cs` (device constants), `hakchi_gui/Hmod/` (hmod package handling).
  A built copy with the kernel package is at `~/Documents/Hakchi2-CE/hakchi_gui/bin/Release/net48/`
  (`mods/hmods/hakchi.hmod`); `fes1.bin` is in the source tree at `~/Documents/Hakchi2-CE/hakchi_gui/data/fes1.bin`. Do not modify anything under that tree.

## Layout

- `crates/fel`: Allwinner FEL USB boot protocol (nusb). No UI, no network.
- `crates/console`: discovery, SSH and FTP session, typed wrappers for the device `hakchi` script.
- `crates/hmod`: hakchi.hmod package parsing and fetching.
- `crates/library`: games on disk, launch entries, art, folder tree, sync diff.
- `src-tauri`: the Tauri shell; thin commands over the crates.
- `src`: Svelte frontend.
- `data/`: binary blobs embedded at build time (`fes1.bin`, `kexec.static`, `detached-fallback`;
  origin and license in `data/README.md`).
- `.agent/`: the autonomous-agent driver and task briefs. Never edit files under `.agent/`
  except to write your report to `.agent/reports/`.

## Rules

- Rust: edition 2024, `cargo clippy -- -D warnings` and `cargo fmt --check` must pass.
  Errors via `thiserror` in library crates, `anyhow` only in binaries. No `unwrap()` outside tests.
- Every crate gets a small CLI binary or example so it can be exercised without the UI.
- Tests must not need hardware. Hardware steps go in `docs/hardware-tests.md` as manual checklists.
- Frontend: Svelte 5 runes, TypeScript strict, pnpm. No component library unless a task says so.
- Commit at the end of each task with a conventional message. Do not push.
- Write a report for each task to `.agent/reports/<task-name>.md`: what was done, what was
  verified and how, what is left or uncertain. Be concrete and brief.
- If a task is blocked, say so in the report and stop. Do not substitute a different scope.
