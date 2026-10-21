> **Continuation.** An earlier run of this brief was interrupted after the implementation was
> written and while it was taking screenshots. Its uncommitted work is in the working tree
> (`git status`: new `src-tauri/src/{flow,failure,package}.rs`, `src/lib/components/
> {FailureCard,FelIllustration}.svelte`, and changes across the app crate and `src/lib/`). It
> compiles, is clippy-clean, passes `cargo test -p cartload-app` (43 tests) and `pnpm check`.
> Start by reading `git diff` and the new files, keep what meets the brief, finish what is
> missing, verify, and commit. Do not start over.

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

> Continuation note 2: the previous run crashed after reading full-size PNG screenshots
> (the driver's message buffer overflowed). Screenshots are fine, but downscale them before
> reading (for example `magick in.png -resize 50% out.png`, or take them at a smaller
> viewport) so each image stays well under 1 MB.
