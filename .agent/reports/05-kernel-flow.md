# Report: 05-kernel-flow

## What was done

`crates/flow` (`cartload-flow`, in the workspace) is the resumable, observable flow engine and
the four flows, built on the three device crates. Layout:

- `src/engine.rs`: `Step<C>` trait (`name`, `description`, optional engine-level `timeout`,
  `already_done` idempotency check returning the reason to skip, async `run` returning
  `Outcome::{Done, AlreadyDone(reason), NotNeeded(reason)}`; `async_trait`), `Flow<C>` with an
  ordered step list and a resume cursor (`run` stops on the first failure, wraps the cause in
  `Error::Step { index, name }` and keeps the cursor there; the next `run` continues from that
  step; `reset` starts over), `Event` (`FlowStarted` with the step list and `resume_from`,
  `StepStarted`, `StepFinished`, `StepFailed`, `Log`, `Progress`, `FlowFinished`; `Serialize`
  for Tauri), `Reporter` (clonable handle over a `tokio::sync::mpsc` unbounded channel, also
  mirrors log lines to `tracing`), `FlowReport`.
- `src/backend.rs`: `Backend` (discover, connect, FEL present, FEL memboot) and `Console`
  (exec with stdin and a per-command time limit, close) traits; `UsbBackend` on
  `cartload-console` (discovery, `Session` with no session-level command timeout) and
  `cartload-fel` (`find_devices`, `Device::open_info`, `memboot` with `fes1()` from
  `cartload-hmod`); `Link` adapts a `Console` to the console crate's `Shell` so `Hakchi::info`
  and the other wrappers run on it.
- `src/context.rs`: `Context` (backend, reporter, `FlowOptions` time limits, `HmodPackage`
  with boot files, raw bytes and md5, the open session, `ConsoleState`, `grew`,
  `expect_cycle`, stock kernel in memory) with `connect`, `refresh_state` (CE's identification
  sequence through `Hakchi::info`), `exec`, `exec_checked`.
- `src/state.rs`: `ConsoleState::{Offline, Online(ConsoleInfo)}` with `is_memboot_shell`,
  `is_installed`, `can_interact`, `md_partitioning` and a one-line `describe()`.
- `src/steps/`: `Memboot` (FEL when offline; kexec from a running hakchi kernel when online,
  through a four-line uploaded launcher that calls the device's own `recovery` function;
  already done when in the memboot shell), `WaitForConsole` (after a reboot or kexec waits for
  the console to go away first, like CE's `WaitForShellCycle`), `WaitOffline`, `DetectState`
  (optionally requiring the memboot shell), `CheckMegaDrive` (CE's Project Lunar refusal),
  `GrowPartition` (`sunxi-part`, `grow --dry-run`, `grow`; already done when no `UDISK`),
  `RebootAfterGrow` (kexec with the package image, skipped unless grown, not needed on MD),
  `PrepareInstall` (the three `HandleHakchi(Install)` commands; already done when the flags
  and directory exist), `UploadHmod` (`cat >`/`cat >>` in 1 MiB chunks with progress; md5
  checked; already done when the remote md5 matches), `RunInstaller` (`boot` under a 15 min
  limit, output echoed, `flash md5 mismatch!` / `invalid image` / `flash boot0 failed!` stop
  the flow), `VerifyInstall` (versions must equal the package's `var/version`, not a memboot
  shell, usable), `ReadStockKernel` (`hakchi getBackup2`, else `--stock-kernel` checked against
  the known md5 list), `FlashStockKernel` (`cat > /kernel.img`, `sntool check`, `sntool
  kernel`, `hakchi flashBoot2`; already done when `hakchi isOriginalBoot2`), `RemoveInstall`
  (`mount_base`, `hakchi mod_uninstall`, `umount_base`), `DumpStockKernel` (already done when
  the file holds a valid boot image), `Reboot`. Command strings in `steps::commands`.
- `src/flows.rs`: `install_kernel` (13 steps), `memboot` (3), `uninstall_kernel` (8),
  `dump_stock_kernel` (6), in CE's `MembootTasks` order.
- `src/partitions.rs`: `sunxi-part` parser equivalent to CE's `NandInfo` regex; `src/kernels.rs`:
  CE's `CorrectKernels` md5 table (14 Nintendo images) and md5 helpers.
- `src/main.rs`: `cartload-flow [--dry-run] [--wait-fel] [--wait-console] [--wait-reboot]
  install --hmod | memboot --hmod | uninstall [--hmod] [--stock-kernel] [--force-stock-kernel]
  | dump-kernel [--hmod] OUT`. Prints `[ n/N] name: description`, the step's log lines, and
  `=> done (time)` / `already done: ...` / `not needed: ...` / `FAILED after ...`; exit 1 with
  "rerun the same command to resume" on failure. `--dry-run` lists steps and validates the
  package without touching USB.
- `docs/hardware-tests.md`: "Install kernel" checklist (dry run, install from stock with the
  expected lines and console behaviour at each of the 13 steps, resume, repair from a running
  kernel, negative checks), plus memboot, dump and uninstall checklists and a results table.

## Verified

- `cargo test -p cartload-flow`: 43 tests, no hardware or network. Engine: ordering and the
  full event sequence, `already_done` skipping, resume after a transient failure (earlier
  steps not rerun, results kept, `resume_from` reported), engine-level timeout and a waiting
  step's own `Error::Timeout` propagating with their messages, `reset`. Parsers and tables:
  `sunxi-part` (CE regex semantics, malformed input), md5 vector, kernel identification, state
  descriptions, command strings. `tests/simulated.rs`: a simulated console (state machine
  answering every command the steps and `Hakchi::info` send, with FEL memboot, kexec, `boot`,
  reboot transitions, paused time) runs install from stock (asserts the exact CE command
  order, one grow, one kexec, one `boot`, chunked upload, progress and verify events, the stock
  kernel backed up), in-process resume after a failed upload, rerun from scratch after a
  failure (every finished step reported already done, nothing redone), repair from a running
  kernel (kexec, no FEL), FEL wait timeout message, "did not reappear" timeout message,
  memboot only, uninstall from a running kernel without a package (exact command order, stock
  kernel flashed, console offline at the end), uninstall needing `--hmod` / `--stock-kernel` /
  `--force-stock-kernel`, dump (file content, idempotent rerun), Mega Drive layout (Project
  Lunar check, no grow reboot, no kernel flash), wrong state after memboot.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `cargo doc -p cartload-flow --no-deps`: clean.
- CLI with the real package and no USB access: `--dry-run install --hmod "$HMOD"` prints the
  package (9340382 bytes, md5 `2e163a25...`), version, the 13 steps and exits 0; `--dry-run
  uninstall` and `dump-kernel` list 8 and 6 steps; `memboot --hmod Cargo.toml` fails with
  `reading Cargo.toml: ... invalid gzip header`.
- No flow was run against hardware. A console running hakchi was attached to this host during
  the task (`04e8:6863`, `hakchi0`), so even the negative checks were not run: any flow would
  have found it and kexec'd it.

## Decisions

- The device-side sequence was traced through the ramdisk scripts (`/tmp/cartload-ref`,
  extracted in task 03): `boot` sources `/hakchi/init` under the shell, which runs
  `script/transfer` because `/hakchi/transfer` exists, `mod_install` because `cf_install=y`,
  then `pack_install hakchi.hmod` whose `install` script flashes u-boot (`flashBoot1`) and the
  kernel (`installBoot2`, after `backupBoot2` saves the stock kernel to sector 68), then the
  preinit and `switch_root`. The `cf_*` flags reach `boot` because the ramdisk's dropbear evidently sets
  `ENV=$HOME/.shrc` (the string is in the binary) and the memboot `.shrc` sources
  `/hakchi/config`. The flow sends CE's exact
  commands and does not depend on this beyond CE's own assumptions.
- Kexec from a running kernel uses the device's `recovery` function through a tiny launcher
  (`trap '' HUP TERM PIPE`, stdio to `/dev/null`) instead of uploading CE's `kexec.static` and
  `detached-fallback` (both GPL artifacts from CE's tree; the ramdisk and rootfs already ship
  `kexec` and the same `memboot`/`recovery` functions). For uninstall and dump from an installed
  console no package is needed: `recovery` without an image kexecs the NAND kernel.
- CE's base hmods (`basehmods.tar`: clovercon, tiny7zx, font_remount, ...) are not transferred;
  only `hakchi.hmod`. They are CE's own mods, not part of the kernel install.
- The splash screen CE pushes to `/dev/fb0` after memboot is skipped (cosmetic).
- `CheckMegaDrive` keeps CE's hard refusal (Project Lunar init scripts) and skips the
  system-file hash comparison, which in CE ends in a user prompt; the step logs that.
- `sunxi-part grow`'s exit code is logged but not fatal (CE ignores it); the step verifies by
  listing the table again and only sets `grew` when `UDISK` is gone.
- Resume across processes relies on the idempotency checks (memboot shell detected, no
  `UDISK`, flags present, remote md5, `isOriginalBoot2`, file content); in-process resume uses
  the cursor. `RebootAfterGrow` cannot tell, across processes, whether a grow was followed by
  the kexec; it is skipped when nothing was grown in the current run, which matches a fresh
  memboot shell (the kernel re-read the table) in every case except a crash between the two.
- Every wait has a limit (`FlowOptions`: FEL 120 s, console 120 s, reboot 180 s, offline 30 s,
  installer 15 min, flash commands 5 min) and a message saying what to check.
- The unbounded event channel never blocks a step; a closed receiver is ignored.
- `.agent/tasks/*` moves left by the driver stay uncommitted, as in earlier tasks.

## Left or uncertain

- Hardware run of `docs/hardware-tests.md` "Install kernel" (all sections). Specific unknowns:
  the real `sunxi-part` output format (parser follows CE's regex; `-v` logs the raw lines), the
  installer's exact output lines and timing, whether the kexec launcher survives the SSH drop
  as CE's `detached-fallback` does (same traps, stdio detached earlier), and the behaviour of
  `hakchi getBackup2` output size.
- Mega Drive Mini paths are modelled on CE's conditions only (no MD hardware).
- The installer's output is collected and echoed when the connection drops, not streamed
  line by line; a UI wanting live installer output needs a streaming exec in the console crate.
