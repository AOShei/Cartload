# Kernel feasibility build

## Done

- Kernel fork `AOShei/sun-nontendocm-kernel`, branch `cartload` (pushed by the user), on public
  `f8c6e0b`: `4f554d6a` `sun_nontendocm_cartload_defconfig` and NAND driver as a module;
  `c4eab2dd` rtl8812au / rtl8192eu / rtl8188fu at the shipped vendor versions; `1ed43a0f`
  version `3.4.113.30-cartload`.
- `scripts/kernel/Containerfile` + `scripts/kernel/build.sh`: pinned sources, Ubuntu 16.04
  GCC 4.9.3, kernel + Mali + clovercon, boot image and memboot test package. Reproducible.
- `docs/kernel.md`, hardware-test section "Cartload kernel", roadmap and `data/README.md` notes.

## Verified

- Config: same 179 built-in drivers and 116 modules as the shipped package's lists (two extras).
- Packing: repacking the pinned package's own kernel and ramdisk reproduces its `boot.img`
  (checked by the script on every build).
- Reproducibility: two clean builds give identical `zImage` (f8cffd1c…), `boot.img` (b286df50…)
  and test package (a66b5f41…).
- `cartload-hmod info` accepts the test package.
- Hardware (SNES EUR, user present): shell memboot works; NAND, USB network, SSH, Mali,
  clovercon; kernel warnings identical to the installed kernel. Ran with an earlier, equivalent
  build from the scratchpad (same sources; archives built with cpio instead of gen_init_cpio).
- Menu (FEL, `cartload-fel memboot --shell boot`, new option): our kernel boots the console's
  system to the SNES menu; mali in use, clovercon, uinput, zram, usb_storage loaded from the data
  partition; fbgrab capture shows the menu. Needed: modules on the data partition (a ramdisk
  with all modules does not start from u-boot), USB device mode forced by a preinit file
  (`p0000_config_defaults` forces `cfg_usb_host='y'`). All test files removed afterwards and
  the config restored, checked over SSH. This run used `build.sh`'s output.

- Install path (read from v1.0.4-126's scripts): the package's `install` refuses kernels whose
  name lacks `madmonkey`; build.sh patches that line and names the output
  `hakchi-3.4.113.30-cartload.hmod`. `cartload-flow --dry-run install --hmod` accepts it.
- `BOOT_IMAGE_MAX_LEN` (0x36 blocks of 128 KiB) in `Package::validate`, unit-tested: a larger
  boot.img would overwrite the stock kernel backup at block 0x68.
- `scripts/console-backup.sh`: raw copies of nanda/b/c, u-boot and the kernels from the memboot
  shell, MD5-checked against the console. Not run yet (needs the user).

## Left / uncertain

- rtl8188fu without sunxi platform hooks; clovercon's source differs from the shipped one
  (same parameters).
- Backup, way-back rehearsal, install and return on hardware; NES and Famicom: not run.
- Cartload has no boot-image size check; the test package must never be installed.
- Two RTC lines in our FEL boot's log that the installed kernel's NAND boot lacks; probably FEL.
- Games, sound, controllers, USB storage, Wi-Fi: not tested.
