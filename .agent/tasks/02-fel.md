# Task 02: crates/fel, the Allwinner FEL USB boot protocol

Goal: a Rust crate that can memboot the console from Linux with no UI, so the hardest piece of
the project is proven first.

Read `docs/device-protocol.md` (FEL section) and the reference C# in
`~/Documents/Hakchi2-CE/Libraries/FelLib/FelLib/` (`Fel.cs` and the `AW*` message structs) and
`~/Documents/Hakchi2-CE/hakchi_gui/Tasks/MembootTasks.cs` for the exact memboot sequence,
addresses, and the in-memory boot image patching it does. `sunxi-tools` (`sunxi-fel`) is a second
reference for the protocol if anything in the C# is unclear.

Implement with the `nusb` crate (pure Rust, no libusb) and async via tokio:

- `fel::Device`: open by VID/PID `1f3a:efe8`, `verify_device()`, `read_memory`, `write_memory`
  with progress callback, `exec(addr)`, `init_dram(fes1: &[u8])`, `run_uboot_cmd`.
- Message structs with explicit byte layouts, round-trip unit tests against byte vectors you
  derive from the C# layouts (no hardware in tests).
- `fel::memboot(device, fes1, uboot, boot_img, options)` reproducing hakchi2 CE's memboot,
  including any command-line patching it performs.
- A CLI binary `cartload-fel` with subcommands `probe` (is a FEL device present, print version
  info), `memboot --hmod <path>` (extracts `boot/boot.img` and `boot/uboot.bin` from the hmod
  tar.gz itself for now; use `data/fes1.bin` from the hakchi2 CE build tree by path argument
  `--fes1`), with `tracing` output.
- Errors via `thiserror`, including a clear "device not found; hold Reset while powering on"
  message.

Verify without hardware: `cargo test -p cartload-fel`, clippy clean. Then write
`docs/hardware-tests.md` section "FEL memboot" with the exact manual steps and expected output
(console reboots and enumerates as `04e8:6863` within ~10 s, SSH reachable at 169.254.13.37).
If a FEL device happens to be present during your run (`lsusb` shows `1f3a:efe8`), run `probe`
only; do not memboot unattended.

Commit.
