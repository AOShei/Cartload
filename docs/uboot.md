# u-boot from source (in progress)

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

## Plan

1. A reproducible build (`scripts/uboot/`): the pinned archive, hakchi's patch, our Buildroot
   toolchain, Cartload's patches for what hakchi's build adds (the boot command, FAT and bootm
   options, the ext4 backport from upstream u-boot), the board configuration from fex text,
   joined and checksummed as sntool's `join`/`check -f` do.
2. Compare it with hakchi's: commands, strings, sizes, the boot command, the configuration.
3. On the console: first from RAM through FEL (u-boot loaded over USB, nothing written), then
   flashed to copy 1 only, with the user present and FEL as the way back.
