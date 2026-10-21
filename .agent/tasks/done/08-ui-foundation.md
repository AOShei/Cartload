# Task 08: UI foundation and the Console page

Depends on tasks 03 and 04. Read `docs/design.md` first (written for task 11; it states the
product's identity and interaction principles) and apply it from the start. Then read the Tauri
shell in `src-tauri/src/lib.rs` and the Svelte shell in `src/`.

Backend (src-tauri):
- A long-lived `AppState` holding the console connection (`cartload-console::Session`) behind
  a tokio mutex, plus a background "watcher" task that probes for the console every 2 s via
  `discover()` and keeps a `ConsoleStatus` (Disconnected / Fel / Booting / Connected(info)).
- Tauri events: `console-status` (emitted on every change) and `log` (structured lines with
  level, source, message) so every page can show live activity.
- Commands: `console_status`, `console_info` (refresh), `console_exec` (dev-only, behind a
  feature flag), `fel_probe`.

Frontend:
- An app store (`src/lib/state/`) using Svelte 5 runes: console status, log lines (capped),
  current page, preferences (theme follows system by default, with a manual override persisted
  via the Tauri store plugin or localStorage).
- Console page: a large status card that says in one line what the console is and what the user
  can do next (for example "SNES Classic Mini, EUR, hakchi 1.0.4, 310 MB free" or "Console in FEL
  mode: ready to install the kernel" or "Nothing found. Connect the console with USB and turn it
  on."), with the detected details below it, and a collapsible live log panel shared by all pages.
- Replace the placeholder sidebar labels with icons plus labels; the current page is clearly
  marked; keyboard navigation works.
- A notification toaster for errors (non-modal). No modal dialogs anywhere for progress.

Verify: `cargo test`, clippy, `pnpm check`, the debug build runs. Without a console the page must
render the "nothing found" state cleanly; with one attached (read-only), the connected state.
Commit.
