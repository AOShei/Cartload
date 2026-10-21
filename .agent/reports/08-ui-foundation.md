# Report: 08-ui-foundation

## What was done

Backend (`src-tauri/src/`):

- `state.rs`: `AppState` managed by Tauri for the life of the app: the console `Session` behind
  a `tokio::sync::Mutex<Option<Session>>`, a `tick_lock` that serialises the watcher with the
  commands, the log ring buffer, the latest `StatusReport`, and a pause counter (`pause()`
  returns a guard; flows in task 09 hold one while they own the USB link).
- `status.rs`: `ConsoleStatus::{Disconnected{hint}, Fel{device, check}, Booting{phase,
  address, since}, Connected{address, link, since, info: ConsoleInfo}}` (serde tag `state`),
  plus `Hint`, `FelCheck` (unchecked / fel with SoC / not FEL / unavailable), `BootPhase`
  (network, connecting, identifying, leaving FEL), `Link` (USB with interface and host
  address, or network). `describe()` composes the `StatusReport` sent to the frontend: a
  `tone`, the one-line `title`, the `next` sentence and a `details` list, so every state has one
  wording, tested in Rust. Connected reads `"SNES Classic Mini, EUR, hakchi v1.0.4-126,
  310 MB free"`, FEL `"Console in FEL mode: ready to install the kernel."`, nothing
  `"Nothing found. Connect the console with USB and turn it on."`; outdated kernels, the
  memboot shell, stock firmware in normal boot, auth refusals and host-side failures each get
  their own sentence and next step.
- `watcher.rs`: the background task. Every 2 s it enumerates USB (`usb.rs`: the FEL ids
  `1f3a:efe8` and the hakchi gadget `04e8:6863` with manufacturer `hakchi`, so a Samsung phone
  in tethering mode is not a console), probes the USB network link with `discover()` (skipped
  while a session over the USB link is open, so a connected console is not pinged every 2 s;
  the gadget's presence and the session's closed flag are the liveness checks), and feeds a pure
  `decide()` function that returns the new status and an action: connect and identify
  (`Session::connect_with` then `Hakchi::info`, with intermediate `Booting{connecting}` and
  `Booting{identifying}` reports), re-identify (the refresh command), or verify a FEL device
  (opened once per plug-in, `verify_device` with a 1 s limit; stock firmware in normal boot has
  the same USB ids and fails that query). Failed connections back off for 8 s; a FEL device
  that disappears gives 45 s of "waiting for the kernel to come up" before "Nothing found".
  Status changes go to a `StatusSink` trait; the Tauri sink emits `console-status`.
- `logging.rs`: a `tracing` layer that turns every event from the app and the `cartload-*`
  crates (`debug` and up for them, `info` and up for everything else) into a `LogLine { seq,
  time, level, source, message }`, stores it in a 500-line ring buffer and emits it as the
  `log` event. Sources are short names (`console`, `fel`, `watcher`, `ssh`). A readable copy
  still goes to stderr (`RUST_LOG`, default `info`).
- `commands.rs`: `app_version` (now with `features` and `debug`), `console_status`,
  `console_info` (forces a tick with re-identification and returns the report), `fel_probe`
  (opens the FEL device and returns the check), `log_recent` (the backlog, so a page that opens
  late shows what happened), and `console_exec` behind the cargo feature `dev-exec`
  (`ExecResult { exit_code, stdout, stderr }`; not compiled otherwise). `error.rs`:
  `CommandError { message, detail }` with the source chain as detail.
- `examples/watch.rs`: the watcher headless (`cargo run -p cartload-app --example watch --
  [--json] [--seconds N]`), printing every report and the log, so the shell can be exercised
  without the UI.
- `src-tauri/Cargo.toml`: tokio, tracing-subscriber, nusb; the cartload crates without their
  CLIs (workspace dependencies now default to `default-features = false`; hmod and library
  keep `fetch`).

Frontend (`src/`):

- `lib/api/`: `types.ts` mirrors of the Rust payloads, `tauri.ts` (`call`, `on`, `inTauri`,
  `AppError`), `commands.ts`.
- `lib/state/` (Svelte 5 runes, class singletons): `console` (report, refresh, FEL probe),
  `log` (capped at 500, backlog merged with live lines by `seq`, error lines raise a toast),
  `navigation` (pages), `preferences` (theme system / light / dark via `data-theme` on
  `<html>`, "show details" in the log, log panel open; persisted in localStorage), `toasts`,
  `app` (version and features).
- `lib/components/`: `Sidebar` (inline SVG icons plus labels, current page marked with the
  accent tint, bar and `aria-current`; Up/Down/Home/End move between pages, Ctrl+1..4 switch),
  `StatusBar` (state dot and sentence, log toggle with a count, version), `LogPanel`
  (collapsible, shared by all pages, Ctrl+L; time, level, source, message; follows new lines
  unless scrolled up; copy, clear, show details), `Toaster` (non-modal, `role="alert"` for
  errors, detail under a disclosure), `StatusCard` (tone colour, cartridge glyph pulsing while
  busy, title, next step, one accent primary action, quiet Refresh / F5), `DetailsList`,
  `Cartridge` (the glyph), `Icon` with `icons.ts`.
- `lib/pages/`: `ConsolePage` (card, details, "everything detected, as data", and a
  command box only when the backend reports `dev-exec`), `SettingsPage` (theme, log details,
  about), `KernelPage` and `LibraryPage` placeholders with one sentence each.
- `app.css`: the design tokens from `docs/design.md` (warm grey scale, the 16-bit purple-blue
  accent for the primary action and focus, green / amber / red, 8 px spacing scale, tabular
  numerals) using `light-dark()`, with a `data-theme` override; focus rings; reduced motion.
- `lib/dev/preview.ts`: fixtures for `pnpm dev` in a plain browser (`?preview=connected|fel|
  booting|stock`), so the layout can be checked without hardware.
- `docs/hardware-tests.md`: a "Console page" checklist (nothing attached, installed kernel,
  FEL, stock firmware) with a results row.

## Verified

- `cargo test --workspace` (app crate: 25 new tests: status wording for every state, the
  `decide()` state machine (nothing, SSH answer over USB and over the network, gadget without
  SSH keeps its start time, connected stays while the gadget is present, unplug disconnects at
  once, closed session reconnects, FEL verified once then remembered, leaving FEL grace then
  give up, back-off after failures), log buffer cap and numbering, source names, error chains,
  USB classification, pause guards), `cargo clippy --workspace --all-targets -- -D warnings`
  (also with `--features dev-exec`), `cargo fmt --all --check`, `pnpm check` (0 errors,
  0 warnings), `pnpm build`: all clean.
- Hardware, read-only, with the SNES Classic Mini (EUR) attached and running hakchi: the
  `watch` example and the debug app both went starting → connecting → identifying →
  "SNES Classic Mini, EUR, hakchi v1.0.4-126, 311 MB free" in 2.4 s, with the fourteen details
  (hwid `0461872a…`, v1.0.4-126 / 3.4.113.29-madmonkey / 1.0.3, stock NAND firmware,
  `/var/lib/hakchi/games`, 353 / 31 / 311 MB, 21 original games, `hakchi0` from
  169.254.68.74) and stayed connected without re-identifying. The app was run three ways:
  `GDK_BACKEND=x11` (fine), native Wayland (WebKitGTK died with `Gdk-Message: Error 71
  (Protocol error)`, a known DMA-BUF issue on this host), native Wayland with
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` (fine). Documented in `hardware-tests.md`.
- Layout: headless Chrome screenshots of the Vite preview for the nothing-found, connected
  (light and dark), FEL and stock-firmware states; the "nothing found" state renders cleanly
  with the sentence from the brief, the grey dot and an empty details area.

## Decisions

- Wording lives in Rust (`status.rs`), not in the frontend: one place, unit-tested, and the
  Kernel and Library pages will reuse the same `StatusReport` while reading structured data
  from `status.info`.
- Theme persistence uses localStorage rather than the store plugin: no extra crate or
  capability, the webview's storage lives in the app data directory. Preferences that belong
  to the backend (library location, API key) will go through commands in later tasks.
- Two states were added to the brief's four as data, not as new variants: `Fel` carries the
  verification result (stock firmware in normal boot shares the FEL USB ids and must not be
  announced as "ready to install"), and `Disconnected` carries a hint (nothing, link without
  answer, SSH refused the hakchi login, host-side probe failure).
- The identification sequence (`Hakchi::info`, which ends every `hakchi` call with the script's
  `save_config`) runs once per connection and on explicit refresh only, never on the 2 s tick.
- mDNS is off in the watcher (a browse costs 3 s and a daemon per tick); a Wi-Fi console that
  answers the TCP probe some other way is still handled as `Link::Network`.
- `fel_probe` and the watcher share the tick lock so they never open the device at the same
  time; task 09's flows should hold `AppState::pause()` while they memboot.

## Left or uncertain

- Not exercised on hardware (the console was attached and running hakchi all day): the
  FEL branch (verify on plug-in, "Check FEL mode"), the booting phases from a cold start, the
  leaving-FEL grace, unplug / replug, and the `console_exec` box. The logic is unit-tested
  against synthetic observations; the checklist in `hardware-tests.md` covers the rest.
- The window could not be captured under Wayland (no `xwininfo`/`grim` on the host), so the
  visual check used the Vite preview in headless Chrome, not the WebKitGTK window.
- `.agent/tasks/*` moves made by the driver stay uncommitted, as in earlier tasks.
