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

## Left / uncertain

- Booting to the menu: a full boot through kexec fails with the shipped kernel too, so it needs
  a FEL memboot without the shell token (user must hold reset). Not run.
- `build.sh`'s own output has not been booted on hardware yet (see above).
- rtl8188fu without sunxi platform hooks; clovercon's source differs from the shipped one
  (same parameters); Wi-Fi, audio, video and USB storage untested.
- A 4 MB boot image (modules in the rootfs), install/uninstall, NES and Famicom: not started.
- Cartload has no boot-image size check; a 6.8 MB image must never be flashed.
