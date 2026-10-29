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

Made from those by the shipped sntool (`sunxi-part`, hakchi v1.0.4-126) under emulation
(`scripts/userland/fakenand/run.sh`, 2026-10-29), the reference a replacement must reproduce:

- `snes-eur-nandinfo-shrunk.bin`: the boot-info block `sunxi-part shrink` writes over copies 1
  and 2 of the grown layout. The factory layout again (data 614,400, private 2,048, UDISK
  124,928), but not byte-identical to `-stock`: UDISK's length is written in the MBR entry (the
  factory has 0) and the names are padded with 0xff after their NUL, as in `-grown`.
- `snes-eur-nand-mbr-shrunk.bin`: the Linux partition table after that `shrink` (written by
  sunxi-nand-part `5c19710`: boot, rootfs, data 614,400, private 2,048, UDISK 0 = the rest).
- `snes-eur-nand-mbr-regrown.bin`: the table after `sunxi-part grow` on the shrunk one. Same
  partitions as `snes-eur-nand-mbr.bin`; differs in the CRC and in two bytes (0xc8, at 0x1e1 and
  0x261 of each copy) the console's table still carries in its unused entries 4 and 5.

`sunxi-part grow` on `-stock` writes `-grown` exactly (the whole boot area then equals the
console's), and on `-shrunk` too.

