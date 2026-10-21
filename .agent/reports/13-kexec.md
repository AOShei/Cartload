# Task 13: memboot from a running hakchi kernel the way hakchi2 CE does it

## What was done

- **Shipped CE's two kexec tools in `data/`.** `kexec.static` (722 876 bytes, md5
  `85e2470a0c78c9876536be7d9798c004`, static ARM kexec-tools build, GPL-2) and
  `detached-fallback` (1 873 bytes, md5 `7a3607d5bc19bef626f03eb120819368`, hakchi, GPL-3),
  copied unchanged. `data/README.md` documents origin (repository, path, commit `74707cd6`),
  sizes, MD5, SHA-256, what each file does and the licence. Deviation from the brief: in the
  CE checkout `kexec.static` is not under `GeneratedData/tools/arm/` (only `detached-fallback`
  is); it lives in the source tree at `hakchi_gui/tools/arm/kexec.static` and in the build
  output `hakchi_gui/bin/Release/net48/tools/arm/`. Both copies have the md5 the brief gives.
- **Embedded them** with `include_bytes!` in a new `crates/flow/src/tools.rs`
  (`cartload_flow::tools::kexec_static()` / `detached_fallback()`), the way `fes1.bin` is
  embedded in `cartload-hmod`. Only the flow crate uses them, so they live there rather than
  in `cartload-hmod`. Unit tests guard both blobs (length, ELF32/ARM header, md5; script
  markers such as `export PATH="$cdir:$PATH"` and `kexec -e`).
- **Rewrote the kexec memboot** (`kexec_memboot` in `crates/flow/src/steps/mod.rs`) to CE's
  `MembootTasks.Memboot` sequence: `[ -f /proc/atags ]`; `uistop` (non-zero exit ignored and
  logged at debug level); `mkdir -p /tmp/cartload`; upload `kexec.static` to
  `/tmp/cartload/kexec` and `detached-fallback` to `/tmp/cartload/detached-fallback`;
  `chmod +x` both (CE's `UploadFile` does a `chmod +x` per file); upload the boot image to
  `/tmp/cartload/boot.img` when there is one; upload the launcher; run it backgrounded with
  a 10 s limit, transport errors ignored. The launcher keeps the existing traps and stdio
  detachment and now ends with `cd /tmp/cartload || exit 1` and
  `exec /bin/sh /tmp/cartload/detached-fallback recovery "$@"`; `hakchi recovery` is gone.
  The no-image variant (uninstall and dump from an installed console) runs
  `detached-fallback recovery` with no argument, so the script kexecs the kernel in NAND.
  Like CE's catch block, a failure between `uistop` and the launch runs `uistart` (best
  effort) and returns the error, so the console is not left without its menu.
- **`reboot-grown` kept as a kexec.** CE's install builds `GrowTask(rebootToRecovery: true,
  shouldGrow: true)`; its `Reboot` task runs `MembootTasks.Memboot` (the same
  `detached-fallback` kexec of the package's `boot.img`) unless `sntool ismd` succeeds, and
  `ShellTasks.Reboot` (`sync; umount -ar; reboot -f`) is only used when `rebootToRecovery` is
  false, which the install never does. So the step now runs the new sequence from the memboot
  shell, with the Mega Drive skip unchanged. Its description and docs say so.
- **Simulated console** (`crates/flow/tests/simulated.rs`) answers `uistop` (exit 1 in the
  memboot shell, 0 otherwise), `uistart` and the `chmod`; tracks which uploads were made
  executable; and at launch asserts that both tools were uploaded byte for byte, are
  executable, that the launcher calls the uploaded script and not `hakchi recovery`, and that
  the no-image variant uploaded no image. The install, repair and uninstall tests check the
  full command order. New test `failed_kexec_preparation_restarts_the_ui_and_reports_the_command`
  injects a failing `chmod`: the error names the command, `uistart` runs, nothing more is
  uploaded, no kexec happens, and a rerun succeeds.
- **Docs.** `docs/hardware-tests.md`: "Install kernel" section 2 step 6, section 4 (repair
  from a running kernel), the results table's Memboot column and the Kernel page's "Install
  again" paragraph describe the new sequence and that it is untested on hardware.
  `docs/device-protocol.md` gains a "Memboot from a running hakchi kernel (kexec)" section
  with CE's exact commands and what `detached-fallback` does. `CLAUDE.md` lists the new
  blobs under `data/`.
- Step docs in `memboot.rs` and `grow.rs` and the `steps::commands` module doc updated.

## Verified

- `cargo fmt --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: all green, among them flow 35 unit + 17 simulated (one new),
  app crate (`src-tauri`) 54 unit tests. The app's Kernel page needed no change: it runs
  `flows::install_kernel()` / `flows::memboot()` and its failure text matching does not
  depend on any changed message. The user-facing lines the docs quote
  (`membooting the package's boot.img with kexec ...`, `uploading the boot image for kexec
  (N bytes)`, `kexec: the console drops off the network now ...`) are unchanged.
- `cargo run -p cartload-flow -- --dry-run install --hmod ~/.cache/cartload/hakchi.hmod`:
  13 steps, step 6 reads "Reboot into the memboot shell (kexec with hakchi2 CE's
  detached-fallback) ...". `--dry-run memboot`: 3 steps. Nothing was run against the
  attached console.

## Left or uncertain

- **Untested on hardware**, both the kexec from a running kernel (the sequence that hung the
  console in its old form) and `reboot-grown` from the memboot shell after a grow. The
  checklist says what to expect and what to do if the console hangs (power-cycle; a rerun
  goes through FEL and skips the grow).
- The launcher runs `detached-fallback` through `/bin/sh` with HUP/TERM/PIPE ignored and
  stdio on `/dev/null`; the script re-executes itself in the background and `wait`s, so the
  ignored signals are inherited by the child. CE instead runs the script directly from the
  SSH exec with a 100 ms limit and lets the session die. If the hardware run shows the kexec
  not happening (console stays up, `/tmp/cartload` present), the next thing to try is CE's
  bare command without the launcher.
- `uistop` in the memboot shell: CE runs it there too and ignores the result; the real exit
  code in that shell is unknown and only logged at debug level.
