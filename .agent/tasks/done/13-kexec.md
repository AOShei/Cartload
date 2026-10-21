# Task 13: memboot from a running hakchi kernel the way hakchi2 CE does it

## Why

The first hardware run of `cartload-flow install` (2026-10-21, SNES Classic Mini EUR with
hakchi v1.0.4-126 already installed) took the kexec path in the `memboot` step and hung the
console: the power LED went off, the USB device never re-enumerated, and only a power cycle
brought it back. The same package later installed fine through FEL, so the image is good.

Cartload's kexec runs the console's own `hakchi recovery`, which uses the `kexec` binary of
the installed rootfs. hakchi2 CE does not trust that binary: it uploads its own
`kexec.static` and a copy of the recovery functions (`detached-fallback`) and runs those.
Read `~/Documents/Hakchi2-CE/hakchi_gui/Tasks/MembootTasks.cs` (the method around line 370,
"use detached-fallback script and up-to-date boot.img") and
`~/Documents/Hakchi2-CE/GeneratedData/tools/arm/detached-fallback`. Note that the script
prepends its own directory to `PATH` so `kexec` resolves to the uploaded binary, and that CE
runs `uistop` first and ignores the (expected) failure of the launching command.

## Do

1. Ship the two files from `~/Documents/Hakchi2-CE/GeneratedData/tools/arm/`:
   `kexec.static` (md5 85e2470a0c78c9876536be7d9798c004, kexec-tools, GPL-2, built by the
   hakchi project) and `detached-fallback` (hakchi, GPL-3). Put them next to `fes1.bin`
   (`data/`), document them in `data/README.md` with provenance and licence, and embed them
   with `include_bytes!` the way `fes1.bin` is embedded.
2. Change the kexec memboot in `crates/flow` so that, from a running hakchi kernel, it does
   what CE does: `uistop`; `mkdir -p /tmp/cartload`; upload `kexec.static` as
   `/tmp/cartload/kexec` and `detached-fallback` as `/tmp/cartload/detached-fallback` (both
   executable); upload the boot image; then run
   `cd /tmp/cartload; /bin/sh /tmp/cartload/detached-fallback recovery /tmp/cartload/boot.img`
   detached from the SSH session (the existing launcher wrapper with its traps is fine, but it
   must call the uploaded script, not `hakchi recovery`). Keep the no-image variant (kexec the
   kernel in NAND) working the same way: `detached-fallback recovery` with no image argument.
3. The `reboot-grown` step (kexec from the memboot shell after `sunxi-part grow`) was skipped
   on the test console because its partition was already grown, so it is untested. Check how
   CE reboots after growing (search MembootTasks.cs and the flash/partition tasks for the grow
   sequence) and make the step match CE. If CE uses a plain `reboot` there, do that.
4. Update the simulated console in the flow tests so the new uploads and commands are covered,
   and keep every existing test passing. Update `docs/hardware-tests.md` ("Install kernel"
   section 2, the "Install again" paragraph of the Kernel page section, and the results
   table's Memboot column) to describe the new sequence and that it is untested on hardware.
5. The app's Kernel page runs the same flow and needs no change; confirm by building the app
   crate and running its tests.

## Do not

- Do not run any flow against the attached console. Dry runs and read-only `cartload-console`
  commands are fine.
- Do not change the FEL path or the installer step; both work on hardware now.
- Do not add a streaming exec or any other feature. Budget is tight: this is the last task
  the credit covers.

## Verify

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo run -p cartload-flow -- --dry-run install --hmod
~/.cache/cartload/hakchi.hmod` (still 13 steps), `cargo run -p cartload-flow -- --dry-run
memboot --hmod ~/.cache/cartload/hakchi.hmod`. Report to `.agent/reports/13-kexec.md` and
commit.
