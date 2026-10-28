# Cartload's hakchi kernel (experimental)

The kernel Cartload installs today is the one in hakchi v1.0.4-126 (`data/hakchi.hmod`):
`3.4.113.29-madmonkey`, built 2020-03-20. Its exact source was never published; the public
kernel tree stops in May 2019 at `.24`. This is a rebuild from that public tree, brought level
with the shipped kernel, so that the kernel Cartload installs can come with its full source. It
is not used by the app yet.

## Source

`AOShei/sun-nontendocm-kernel`, branch `cartload` (on top of the public `f8c6e0b`):

- `sun_nontendocm_cartload_defconfig`: the configuration of the shipped kernel, read from its
  package's `modules.order` and `modules.builtin`. The published `sun_nontendocm_defconfig` is
  not what shipped: it builds about 80 drivers in that the shipped kernel has as modules or
  leaves out, and embeds its config, which the shipped kernel does not.
- The sunxi NAND driver as a module, as shipped: hakchi's boot ramdisk loads `nand.ko` itself so
  the console can boot from memory without touching the NAND.
- Three Realtek USB Wi-Fi drivers the shipped kernel has and the public tree lacks, at the vendor
  versions the shipped modules report: rtl8812au `v4.3.20_16317.20160108`, rtl8192eu
  `v4.4.1_17696.20160509_BTCOEX20160412-0042`, rtl8188fu `v4.3.23.6_20964.20170110`. The commit
  message names the public repository and commit of each.
- Version `3.4.113.30-cartload`, so it only ever loads its own modules.

Mali (r4p0-00rel0, `modules/mali`) and the controller driver (`clovercon/`) are built outside
`make modules`, as for the shipped package's `extra/` modules.

How close it is, by the shipped package's lists: the same 179 built-in drivers and the same 116
modules, plus two that the tree builds unconditionally (`sunxi-knob`, `crc-t10dif`). zImage
1,973,376 bytes against 1,968,736. Known differences:

- rtl8188fu has no sunxi platform hooks (the public source dropped them); it works as a plain
  USB driver. The shipped one switches the USB host controller on itself.
- rtl8192eu uses the sunxi platform file of rtl8188eu and rtl8812au; its own targets a later
  Allwinner board API this kernel does not have.
- clovercon has the same parameters as the shipped module but a different `srcversion`: the
  shipped one was built from a later, unpublished revision of Cluster's driver.
- The compiler is Ubuntu's GCC 4.9.3, the shipped kernel used Buildroot 2015.11.1's GCC 4.9.3.

Like every Allwinner kernel, the tree carries two binary blobs: the ARISC power-management
firmware (`drivers/arisc/binary/arisc_sun8iw5p1.code`) and, unused by this build,
`modules/nand/sun8iw5p1/libnand_sun8iw5p1`. The NAND driver this build uses
(`drivers/block/nand`) is source.

## Building

```sh
scripts/kernel/build.sh          # into target/kernel/out
scripts/kernel/build.sh --clean  # from scratch
```

It needs podman or docker and fetches the Ubuntu 16.04 image and the two pinned repositories on
the first run (about 25 s afterwards on 24 cores). The build is reproducible: dates, user and
host in the kernel, the vendor drivers' `__DATE__`, Mali's build date and the archive times are
all fixed to the kernel commit, so two builds of the same commits give the same bytes.

Output: the kernel (`zImage`, `System.map`, `.config`), a `boot.img`, the modules
(`target/kernel/stage/lib/modules/3.4.113.30-cartload`) and a package
`hakchi-3.4.113.30-cartload.hmod`. The script first checks that repacking the pinned package's own
kernel and ramdisk reproduces its `boot.img`.

- The boot image is the pinned package's, with the kernel replaced and a second ramdisk archive
  appended that puts this kernel's NAND and SCSI drivers in `/lib/modules-ramfs`, as the
  package's ramdisk has madmonkey's. A ramdisk with every module (4.8 MB) does not start from
  u-boot at all, though kexec boots it.
- The package is the pinned one with that boot image, this kernel's modules in `lib/modules`
  (hakchi's installer copies them to the data partition, replacing madmonkey's) and one line of
  its `install` script changed: it refused to run on a kernel whose name lacks `madmonkey`.

## Size

hakchi's installer flashes `boot.img` with `sunxi-flash burn_boot2` from NAND block 0x30 (blocks
of 128 KiB) and keeps the stock kernel's backup at block 0x68; `sunxi-flash` only stops at the
end of the boot area, so a larger image would overwrite the backup. Cartload refuses packages
whose boot image exceeds 0x36 blocks (7,077,888 bytes, two blocks left for bad blocks):
`cartload_hmod::BOOT_IMAGE_MAX_LEN`, checked by `Package::validate`. This one is 4,712,448 bytes
(36 blocks, like v1.0.4-126's).

## Testing

```sh
cargo run -p cartload-flow -- memboot --hmod target/kernel/out/hakchi-3.4.113.30-cartload.hmod
```

boots it from RAM into hakchi's shell (from a running console through kexec, otherwise through
FEL). Return with `sync; umount -ar; reboot -f` (plain `reboot` does nothing in that shell) or by
power-cycling. `cargo run -p cartload-fel -- memboot --hmod ... --shell boot` boots it through
FEL into the console's own system and menu, with the modules on the console and USB device
mode forced. Checklists and results: `docs/hardware-tests.md`, section "Cartload kernel".

On a SNES Classic Mini (EUR) it boots to the menu with Mali, the controller driver and the
other modules working (2026-10-28).

## Installing

A console cannot be bricked by a kernel: FEL is in the chip's boot ROM, and from FEL a working
kernel boots from RAM and the installer flashes it back. What a faulty kernel can damage is the
data partition (games, saves, settings). So, in order:

1. Back up the NAND: `scripts/console-backup.sh` copies every partition, u-boot and the kernels
   to `~/cartload-backups/` with MD5s checked on the console.
2. Rehearse the way back while nothing is at risk: `cartload-flow install --hmod data/hakchi.hmod`
   (or **Reinstall** in the app) on the console as it is.
3. Install: `cargo run -p cartload-flow -- install --hmod target/kernel/out/hakchi-3.4.113.30-cartload.hmod`.
   The installer runs in the memboot shell on this kernel, flashes u-boot and the kernel and
   copies the modules.
4. Back: the same command with `data/hakchi.hmod`. It kexecs madmonkey's kernel from this one,
   and the installer swaps the modules folder back. If the console does not start: FEL, then the
   same command (the memboot goes through FEL), then restore the data partition from the backup
   if needed.

Steps 1 to 3 were done on a SNES Classic Mini (EUR) on 2026-10-28: it starts from NAND on this
kernel to the menu in 13 s. Step 4 was rehearsed before the install, not yet after it. Still to
do: games, sound, controllers, USB storage and Wi-Fi; the return from this kernel; the NES and
Famicom Minis.
