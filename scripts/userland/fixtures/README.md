# NAND partition data from a SNES Classic Mini (EUR)

Test data for a replacement of hakchi's `sunxi-part` (docs/userland.md). Read on 2026-10-28
from a console whose data partition hakchi had grown; only partition metadata and NAND
geometry, no user data.

- `snes-eur-nand-mbr.bin`: the first 64 KiB of `/dev/nand`, four copies of Allwinner's Linux
  partition table (`softw411`, version 0x200, 16 KiB each): boot, rootfs, data (grown).
- `snes-eur-nandinfo-grown.bin`: the 32 KiB boot-info block after u-boot copies 1 and 2
  (NAND blocks 0x08 and 0x0f, at offset 0xba000): the NAND driver's own partition list as hakchi
  left it (data 741,376 sectors, no UDISK).
- `snes-eur-nandinfo-stock.bin`: the same block after u-boot copies 3 and 4 (blocks 0x16, 0x1d),
  untouched since the factory: data 614,400 sectors, private 2,048, UDISK the rest.

The block layout and its checksum are in docs/device-protocol.md ("NAND boot area").
