# Report: 02-fel

## What was done

`crates/fel` is now a working FEL implementation on nusb 0.2 (pure Rust, no libusb) with tokio
for timeouts and delays. Layout:

- `src/message.rs`: the six wire structs with explicit little-endian layouts (`UsbRequest`
  "AWUC" 32 B, `UsbResponse` "AWUS" 13 B, `FelRequest` 16 B, `FelMessage` 16 B, `FelStatus`
  8 B, `VerifyDeviceResponse` "AWUSBFEX" 32 B), `to_bytes`/`from_bytes`, and round-trip tests
  against byte vectors written out by hand from the hakchi2 CE `AW*` classes.
- `src/transport.rs`: `Transport` trait (raw bulk write / read-exactly) and `UsbTransport` on
  nusb: opens the device, selects configuration 1 if unconfigured, claims interface 0, picks the
  first bulk IN/OUT pair (warns if not 0x82/0x01), and does transfers with a 3 s timeout that
  cancels and drains on expiry. IN requests are rounded up to whole packets as nusb requires.
  Also `find_devices()` and `wait_for_device(timeout)` (polls every 500 ms).
- `src/device.rs`: `Device<T: Transport = UsbTransport>` with `open()`, `open_info()`,
  `verify_device()`, `read_memory()`, `write_memory()` / `write_memory_with_progress()`,
  `exec()`, `init_dram(fes1)`, `run_uboot_cmd(uboot, cmd)`. Framing, 64 KiB chunking, word
  padding, status checks and the "fes1 already resident" / "u-boot already resident" shortcuts
  match `Fel.cs`. DRAM addresses are refused with `DramNotInitialised` until `init_dram` ran,
  instead of CE's implicit init (the fes1 bytes are an explicit argument per the brief).
- `src/memboot.rs`: Android boot header parsing and size calculation (`CalcKernelSize`
  equivalent), sector padding, u-boot `bootcmd=` lookup, the SD marker swap (CE memboots with
  `UbootType.SD`: last two 32-bit words of `uboot.bin` swapped), the kernel command line patch
  (`ShellOverride::{Keep, Network, Clovershell}`, CE's force-network / force-clovershell), and
  `memboot(device, fes1, uboot, boot_img, options, progress)` doing verify → R16 check →
  init_dram → upload to 0x47400000 → `boota 47400000` via u-boot at 0x47000000.
- `src/error.rs`: `thiserror` enum. "No FEL device" and "did not answer" messages both tell the
  user to hold Reset while powering on; permission errors point at the udev rule.
- `src/main.rs`: `cartload-fel probe [--wait SECS]` and `cartload-fel memboot --hmod PATH
  --fes1 PATH [--wait SECS] [--shell keep|network|clovershell] [--uboot sd|normal] [--dry-run]`
  with `tracing` output (`-v` for debug, `RUST_LOG` honoured). The binary extracts
  `boot/boot.img`, `boot/uboot.bin` and `var/version` from the hmod with `flate2` + `tar`
  (behind a default `cli` feature so the library stays lean for the Tauri app).
- `docs/hardware-tests.md`: "FEL memboot" section with the exact commands, expected output
  and a results table.

## Verified

- `cargo test -p cartload-fel`: 39 unit tests pass, no hardware. Beyond the byte-layout tests,
  a scripted `MockTransport` replays the exact expected wire sequence (every AWUC header,
  payload, AWUS, FEL message and status) for verify, read/write chunking and padding, exec,
  both init_dram paths, both run_uboot_cmd paths, and a full memboot of a synthetic image
  (two 64 KiB kernel chunks, cmdline patched to `hakchi-clovershell`, u-boot upload,
  `boota 47400000\0` written at `bootcmd` + 8, exec). The real v1.0.4-126 `boot.img` header is
  embedded as a vector: declared size 4 585 472 matches the file.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `cargo doc -p cartload-fel --no-deps`: clean.
- Real inputs, no device: `memboot --dry-run` against the hakchi2 CE `hakchi.hmod` and
  `fes1.bin` reports uboot 761 856 B with `bootcmd` value at 0x6a54b, boot.img 4 585 472 B
  (kernel 1 968 736 + ramdisk 2 612 272, page 2048) padded to 4 587 520 B, cmdline ending in
  `hakchi-shell`; `--shell clovershell` rewrites the tail. `probe` and `memboot` without a
  device exit 1 with the "hold Reset while powering on" message.
- A compile-time test asserts the `memboot` and `verify_device` futures are `Send` for Tauri.
- No FEL device was present (`lsusb` had no `1f3a:efe8`; the console on this host was already
  running hakchi as `04e8:6863` with SSH open on 169.254.13.37), so per the brief nothing was
  membooted and `probe` only exercised the not-found path.

## Decisions

- `fes1.bin` is not in the build tree's `data/` directory as CLAUDE.md says; it lives in the
  source tree at `hakchi_gui/data/fes1.bin` (14 048 B, md5 2d1c7b3b…). Docs point there.
- CE's cmdline patch has a no-op `Replace` (C# strings are immutable) so it actually appends
  a second token. I implemented the evident intent: drop existing `hakchi-shell` /
  `hakchi-clovershell` tokens, then append the chosen one. Default is `Keep` (no patch),
  which is what CE does with default settings.
- `device-protocol.md` says the FEL IN endpoint is 0x81; CE's `OpenFel` requires 0x82 and
  sunxi-tools detects it dynamically. The code detects endpoints and only warns, and the
  hardware checklist asks to record the real numbers. I did not edit the protocol doc.
- Transfer timeout 3 s (CE 1 s, sunxi-tools 10 s). After a timeout the pipe is treated as
  unusable; callers drop the `Device`.
- The brief's size limit for the memboot image is CE's `transfer_max_size` (32 MiB), not
  `kernel_max_size` (4 MiB), since the stock boot.img is 4.4 MiB.
- Flash read/write (`sunxi_flash phy_read/phy_write` through u-boot) and the u-boot
  "return to FEL" wait loop were not implemented: not needed for memboot. `run_uboot_cmd`
  documents that callers re-open the device for commands that return.
- `.agent/tasks/*` changes present in the working tree (moved/added briefs) were left
  uncommitted; only crate, docs, lockfile and this report are in the commit.

## Left or uncertain

- Hardware run of `docs/hardware-tests.md` "FEL memboot" (steps 2 to 5). In particular:
  real endpoint numbers, whether a 64 KiB bulk OUT in one URB is fine on this host (CE and
  sunxi-tools do the same, so expected yes), and what the stock firmware produces in the
  negative check (timeout vs no bulk endpoints).
- macOS and Windows untested (nusb supports both; Windows needs WinUSB bound to 1f3a:efe8).
