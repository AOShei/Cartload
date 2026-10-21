# Task 05: the kernel install flow

Depends on tasks 02, 03 and 04 being done (check `.agent/reports/`). Goal: a resumable, observable
flow that takes a console from stock firmware (or a broken install) to a working hakchi kernel,
runnable from the CLI with no UI.

Read `~/Documents/Hakchi2-CE/hakchi_gui/Tasks/MembootTasks.cs` end to end. It contains the exact
sequences hakchi2 CE uses for: memboot, "Install / Repair", uninstall, dump stock kernel, flash
stock kernel, and the shell commands it runs on the console between steps (grow partition, run
the hmod `install` script, `hakchi flashBoot2`, reboot and reconnect). Also read
`hakchi_gui/hakchi.cs` for how it decides console type and region, and what "stock", "hakchi
kernel", "memboot" and "legacy" states look like. Verified on 2026-10-21: the full sequence
memboot → grow → install → reboot → reconnect works on an SNES Classic EUR from Linux.

Implement in a new crate `crates/flow` (add to the workspace):

- A `Flow` type: an ordered list of `Step`s, each with a name, a user-facing description, an
  async `run(&mut Context) -> Result<Outcome>`, and an idempotency check so a flow can be resumed
  after a failure without redoing finished steps. Progress and log lines are reported through a
  channel (`tokio::sync::mpsc`) so a UI or CLI can render them.
- Flows: `InstallKernel` (memboot via `cartload-fel`, wait for the console to reappear on USB
  network via `cartload-console::discover`, detect state, grow, upload hmod, run install, reboot,
  wait, reconnect, verify), `Memboot` (memboot only, for recovery), `UninstallKernel`,
  `DumpStockKernel` (writes `kernel.img` to a path). Keep each step small.
- Waiting steps must have timeouts with clear messages ("console did not reappear within 60 s;
  check the USB cable and that FEL mode was entered").
- CLI binary `cartload-flow` with `install --hmod <path>`, `memboot --hmod <path>`,
  `uninstall`, `dump-kernel <out>`. It prints each step, its outcome and timing.
- Tests: the flow engine with fake steps (ordering, resume after failure, timeout propagation).
  No hardware in tests. Add an "Install kernel" checklist to `docs/hardware-tests.md` with the
  exact command and expected console behaviour at each step.

Do not run any flow against hardware unattended. Commit.
