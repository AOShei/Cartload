# Task 09: the Kernel page

Depends on tasks 05 and 08. Follow `docs/design.md`.

- Backend commands wrapping `cartload-flow`: `flow_start(kind)`, `flow_cancel`, with progress
  streamed as a Tauri event `flow-progress` carrying step index, name, state (pending, running,
  done, failed, skipped), elapsed time, and the last message. Only one flow at a time.
- Kernel page: a guided install. Step 1 explains how to enter FEL mode with a short illustration
  (an inline SVG of the console with the Reset switch, no external assets), and watches for the
  FEL device; Step 2 shows the hmod version to be installed (from the bundled or cached package,
  with an "update package" action calling `fetch_latest`); Step 3 runs the flow with a vertical
  step list, each with its state and timing, and the live log beneath; Step 4 confirms the
  result with the console info card from the Console page.
- Secondary actions in a quiet menu: memboot only, uninstall, dump stock kernel (with a native
  save dialog).
- Every failure state names the likely cause and the next action, using the timeout messages
  from the flow crate.

Verify as in task 08. Do not run a flow against hardware unattended. Commit.
