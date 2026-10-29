# u-boot from source

Step 3 of "Independence from unverifiable builds" (`docs/roadmap.md`): rebuild hakchi's
`boot/uboot.bin` (u-boot and its board configuration, flashed into u-boot copy 1 by every
install) from source. Findings so far (2026-10-29).

## The source

Nintendo's `r16-uboot` (u-boot 2011.09-rc1, Allwinner's tree for the R16), commit
`fc3061df4dbd4153819b2d2f141d82b88fea51cf`, in the NES Classic's open-source archive:

| File | SHA-256 |
|---|---|
| `http://data.nintendo.co.jp/oss/NintendoEntertainmentSystemNESClassicEdition_OSS.zip` (260,346,561 bytes, still online) | `d6cfa41888185889c2b580d91ae4a8b07f7e1b737334110ee50173f20386187b` |
| inside it, `r16-uboot-fc3061df4dbd4153819b2d2f141d82b88fea51cf.tar.gz` | `3afd5abcebc7e18b4de2923c51f48eea2f4cca5d7bcba589f44dac51c7ae283f` |

No separate SNES archive was found; the SNES's own u-boot (copy 3 on a console) reports
`U-Boot 2011.09-rc1 (May 11 2017)`, the same tree. hakchi's public patch is
`r16-uboot/r16-uboot.diff` in the hakchi fork (a `phy_write` command, `CONFIG_FINAL`, build
fixes for GCC 5).

Prebuilt binaries in the tree: Allwinner's DRAM setup (`arch/arm/cpu/armv7/*/dram/libdram*`), linked
only into the SPLs (boot0, fes1, sbrom), not u-boot; NAND libraries for other chips (the R16's,
`nand_sunxi/sun8iw5`, is source); display/HDCP libraries for other chips; OpenSSL 1.0.1g
archives. A build of the `clover` target with hakchi's patch links **none of them** into u-boot
(`u-boot.map`: no OpenSSL, no libdram; only GCC's libgcc helpers). So u-boot proper can be built
entirely from source; boot0 and fes1 cannot (DRAM setup is a blob; mainline u-boot's SPL has
open DRAM code for the A33/R16, a possible later replacement).

## hakchi's u-boot (v1.0.4-126 `boot/uboot.bin`)

`U-Boot 2011.09-rc1-00000-g32c3df5-dirty (Nov 19 2019)`, built with `Buildroot 2015.11.1` GCC
4.9.3 and binutils 2.24 (the userland's toolchain), from an unpublished tree. 0xba000 bytes:
u-boot 0x90000, its board configuration (`script.bin`) at 0x90000, the boot-info after.

- Board configuration: Nintendo's, with one change, `dcdc2_vol = 1140` (was 1100: CPU core
  voltage 1.14 V). The extra size is padding. Decoded with `sunxi-fexc`, it is readable fex
  text, so it can be kept as source.
- Boot command: `ext4load sunxi_flash 4:0 43800000 hakchi/boot/boot.img; boota 43800000;`, the
  same on `2:0`, then `sunxi_flash phy_read 43800000 30 28; boota 43800000;`, then Nintendo's
  fallbacks (`fatload ... uImage`, `bootm`, `efex`). Nintendo's: `sunxi_flash phy_read 43800000
  30 20; boota 43800000`.
- Compared with a build of the public source and patch, it adds: **ext4** (`ext4load`, `ext4ls`,
  `ext4fs_devread`: Nintendo's tree has no fs/ext4; upstream u-boot gained it in 2012.10, so it
  is a backport), FAT commands (`fatload`, `fatls`, `fatinfo`), `bootm` with legacy images and
  gzip/bzip2, GPT/EFI partitions, `fel_gpio`, `check_update_key_gpio`; and it lacks Nintendo's
  fastboot and recovery key handling (`fastboot_key`, `boot-recovery`, `misc partition`).

## Building

```sh
scripts/uboot/build.sh   # into target/uboot/out: uboot.bin, u-boot.map, report.txt
```

Nintendo's source (archive and tarball checked by SHA-256), hakchi's `r16-uboot.diff` from the
hakchi fork at the userland's pin, then Cartload's patches (`scripts/uboot/patches/`):

1. `0001-build-with-gcc-4.9`: `-Wno-maybe-uninitialized`, so the tree builds with the userland's
   toolchain (Buildroot 2015.11.1 GCC 4.9.3, the one hakchi's u-boot was built with).
2. `0002-boot-command`: `sunxi_flash phy_read 43800000 30 28;boota 43800000;efex`. The kernel
   from NAND block 0x30, 0x28 blocks (5 MiB, as hakchi's u-boot reads; Nintendo's read 0x20, too
   little for a 4.2 MB boot.img), then FEL (`efex`) if it does not boot. hakchi's u-boot first
   tries `hakchi/boot/boot.img` on the data partition through ext4, which only the Mega Drive
   Mini uses (hakchi's install creates that directory only there; on a SNES it does not exist),
   so the unpublished ext4 backport is left out, with hakchi's FAT, bootm and GPT additions.

Only u-boot is built: boot0 and fes1 link Allwinner's DRAM setup, a binary (the build stops if
`u-boot.map` shows a prebuilt library). The board configuration is `scripts/uboot/sys_config.fex`,
decoded from hakchi's image (Nintendo's with `dcdc2_vol = 1140`); sunxi-fexc compiles it back to
hakchi's 41,828 bytes exactly. The image is laid out as Nintendo's: u-boot, 0xff to 0x80000, the
configuration, 0xff to 0xba000 (the length every copy in the NAND boot area is spaced by: it must
not change, as the installer rewrites copy 1 only), header lengths at 0x14 and 0x18, checksum by
sntool's `check -f`. The build date in the banner is the hakchi checkout's commit time; two
builds give the same bytes. So a new hakchi pin changes uboot.bin (its date and checksum): rebuild
u-boot before assembling, or the package will not match a build from scratch (CI checks). `scripts/userland/assemble.sh` puts it in the package as
`boot/uboot.bin`.

First result (2026-10-29): u-boot 515,916 bytes (Nintendo's 515,804, hakchi's 589,824), image
`059caf38...`; same header fields and configuration as hakchi's.

## Testing on a console

1. From RAM through FEL (nothing written): power off, hold Reset, power on, release after two
   seconds; `cargo run -q --release -p cartload-flow -- memboot --hmod target/assemble/out/cartload-system-<version>.hmod`.
   FEL loads fes1 (hakchi2 CE's, Cartload's `data/fes1.bin`), then this u-boot with its boot
   command replaced by `boota 47400000`, and the package's boot.img. It shows that the u-boot
   runs (display, USB, boota); not the NAND path.
2. Installed (`cartload-flow install`): hakchi's installer writes it to u-boot copy 1 (copy 2
   keeps hakchi's). Then a cold start reads the kernel from NAND through `sunxi_flash phy_read`.
   If it did not start, FEL (Reset at power-on) brings it back; Cartload's way back is
   `install --hmod data/hakchi.hmod` from a FEL memboot.

Done on a SNES (EUR) on 2026-10-29, both steps (docs/hardware-tests.md "Cartload u-boot"): from RAM
through FEL, then installed in copy 1 and cold-started through it. One difference from hakchi's
u-boot shows on the kernel command line: `partitions=` is empty, as Nintendo's u-boot leaves it
(its `phy_read` skips the logical NAND setup); nothing reads it.

