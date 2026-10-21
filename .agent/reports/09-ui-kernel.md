# Report: 09-ui-kernel

Continuation of an interrupted run. Its implementation was read in full, kept (it met the
brief), finished and verified; nothing was started over.

## What was done

Backend (`src-tauri/src/`):

- `flow.rs`: `FlowKind` (`install`, `memboot`, `uninstall { stock_kernel, force_stock_kernel }`,
  `dump_kernel { path }`, serde-tagged `kind`), `FlowSnapshot` (seq, kind, state idle / running /
  done / failed / cancelled, one `StepSnapshot` per step with index, name, label, description,
  state pending / running / done / failed / skipped, `started_at`, `elapsed_ms`, last message
  and transfer progress; `current`, `failure`, `cancel_pending`, start and end times) folded
  from the `cartload-flow` events, and `FlowRunner`: one flow at a time, `start` refused while
  one runs, the watcher paused and its session dropped while the flow owns the USB link, the
  `ConsoleStatus::Working` status published per step so the status bar reads "Installing
  hakchi: step 5 of 13, grow the data partition…", every change emitted as `flow-progress`
  through a `FlowSink`. `cancel` stops a waiting step at once; during `grow`, `install`,
  `flash-stock-kernel` and `remove` (flash writes) it waits for the step to end
  (`cancel_pending`). A failed or stopped flow keeps its context, so "Try again" of the same
  kind reuses the package and the idempotency checks skip the finished work.
- `failure.rs`: `FlowFailure { step_index, step_name, kind, title, cause, next, detail,
  wants_stock_kernel, wants_force }` composed from the flow crate's structured errors: the
  timeouts (FEL, reappearing, leaving, installer) keep the crate's hint as the next action with
  "rerun the command" turned into "start again"; wrong-state, precondition (missing package,
  no stock backup, unknown kernel md5, Project Lunar, Mega Drive dump), device (installer
  errors, md5 mismatch, version mismatch, `sntool`), FEL (not found, permission denied),
  console (auth, command timeout) and I/O errors each get a title, a likely cause and a next
  step; the raw chain is the detail.
- `package.rs`: the kernel package for step 2: `CARTLOAD_HMOD` override, else
  `~/.cache/cartload/hakchi.hmod`; `package_info` (version, boot, kernel, size, date, source,
  problem of an unusable file, download sources) and `package_update` (`cartload_hmod::
  fetch_from` over the same URLs as `fetch_latest`, progress as `package-download` events,
  verified before replacing the cache; refused while a flow runs).
- `commands.rs`: `flow_start(kind)`, `flow_cancel`, `flow_status`, `flow_steps(kind)`,
  `package_info`, `package_update`; `lib.rs` registers them, the dialog plugin and the runner;
  `status.rs` / `watcher.rs`: the `Working` status, and a paused watcher neither ticks nor
  probes FEL. `capabilities/default.json`: `dialog:allow-open`, `dialog:allow-save`.

Frontend (`src/`):

- `pages/KernelPage.svelte`: the four stages on one page with a connecting rail. 1: the FEL
  instructions beside `FelIllustration.svelte` (inline SVG console with POWER and RESET
  switches, the RESET one haloed with a "hold" arrow, and the USB cable to the computer) and a
  one-line watch fed by the console store (nothing found, FEL ready, FEL device with another
  chip, stock firmware in normal boot, hakchi installed so no Reset needed, recovery shell,
  working). 2: the package version, boot and kernel versions, size and date, "Update package",
  or "Download package" with progress when there is none, and the problem of an unusable file.
  3: the flow title and running time, the accent "Install hakchi" / "Reinstall hakchi" (one
  primary per screen; disabled with the reason in a line), "Stop" while running ("Stopping
  after this step…" during flash writes with an amber note), `StepList.svelte` (state marker,
  label, description, live timing for the running step, transfer bar, last message; the step
  list of the install flow is shown pending before anything runs) and `FlowLog.svelte` (this
  run's lines from the shared log store, sticky to the bottom, "Full log" opens the panel).
  4: on success a one-sentence outcome per flow over the `StatusCard` and `DetailsList` from
  the Console page with "Open Library" / "Open Console"; on failure `FailureCard.svelte`
  (title, failed step, likely cause, what to do, raw error under a disclosure, "Try again",
  and "Choose a kernel dump…" / "Flash it anyway" when the failure asks for them); on stop a
  "Stopped." card with "Continue".
- The quiet "More" menu: "Boot from memory only", "Return to the original system…" (an inline
  confirmation worded as the action, "Keep hakchi" cancels) and "Save the stock kernel…"
  (native save dialog, then the dump flow). The menu closes on Escape or a click elsewhere.
- `state/flow.svelte.ts` (snapshot store: `flow-progress` subscription, newest-seq wins, a
  250 ms clock while running, `begin` / `cancel`, `flowTitle`, `formatDuration`),
  `state/package.svelte.ts` (status, download progress, `formatBytes`), `api/commands.ts` and
  `api/types.ts` mirrors, `navigation` accepts `?page=` for previews, `dev/preview.ts` fixtures
  `?flow=idle|running|failed|done|cancelled` and `?package=cached|none|problem`.
- `docs/hardware-tests.md`: "Kernel page (desktop app)" checklist (package download, install
  from stock and reinstall, stop, FEL timeout failure, the three menu actions) with a results
  row.

This run's own changes on top of the interrupted one: the "Stopped before it finished." note
was hidden (the step list suppressed messages on pending steps), the More menu only closed by
toggling, stage 4's marker stayed neutral on failure; all three fixed. The checklist and this
report were written; everything was verified and committed.

## Verified

- `cargo clippy --workspace --all-targets -- -D warnings` (also `-p cartload-app --features
  dev-exec`), `cargo fmt --all --check`, `cargo test -p cartload-app`: 43 tests, among them the
  snapshot folding of every event kind, cancel putting the running step back to pending, the
  unsafe-to-stop list, kind serialisation and step counts (13 / 3 / 8 / 6), install refused
  without a package, a memboot against a backend with nothing attached failing with the FEL
  timeout explanation and releasing the watcher, one flow at a time and cancel of a waiting
  step, the failure wording for timeouts / preconditions / device / USB / connection / file,
  package candidates and status, the paused watcher, the working status sentence.
- `pnpm check` (0 errors, 0 warnings), `pnpm build`.
- Headless Chrome screenshots of the Vite preview, downscaled to 50 % before reading: fresh
  (nothing attached, no package), FEL ready, running (upload at 5 of 8.9 MB with the bar and
  timings), failed (FEL timeout card with cause and next action), done (console card and
  details, light and dark), cancelled, unusable package. Layout and wording checked in each.
- The debug app launched with `WEBKIT_DISABLE_DMABUF_RENDERER=1` for 20 s with the SNES
  Classic Mini attached and running hakchi: started, connected and identified the console in
  2.5 s as in task 08, no error from the dialog plugin or the new state; no flow was started.

## Decisions

- No package is bundled into the binary: `docs/design.md` names the kernel package as the one
  runtime download besides art, it is a 9 MB GPL artifact of madmonkey's hakchi, and the cache
  plus the `CARTLOAD_HMOD` override cover the offline case. Step 2 says so and downloads it.
- `package_update` calls `fetch_from` with the crate's `DOWNLOAD_URLS` rather than
  `fetch_latest`, which is the same call without progress reporting.
- Flow wording (step labels, failure causes and next actions, the status sentence) lives in
  Rust and is unit-tested, as the status wording was in task 08.
- A stop during a flash-writing step is deferred rather than refused, so the console is never
  left half written; the page says so.
- The event payload is the whole snapshot (a few KB at most, a few times a second at peak),
  not a delta, so a page that opens late or reloads is correct from the first event.
- `.agent/tasks/*` moves and the driver's own edits stay uncommitted, as in earlier tasks.

## Left or uncertain

- No flow was run on hardware (a console running hakchi was attached all day, and the brief
  forbids unattended runs); the checklist in `docs/hardware-tests.md` covers it. In particular
  the installer's output arrives when the step ends, not line by line (a streaming exec in the
  console crate would be needed for live installer lines).
- The native dialogs were not exercised (no window capture or input tool on this host).
- The More menu and the uninstall confirmation were checked by reading, not in a screenshot.
