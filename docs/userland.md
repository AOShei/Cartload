# The console userland (in progress)

Step 2 of "Independence from unverifiable builds" (`docs/roadmap.md`): rebuild from source every
program and library hakchi v1.0.4-126 puts on a console outside the kernel, with the toolchain
it was built with. This file records what ships, where each piece comes from, and how far the
rebuild is.

## What ships

Two places: the boot ramdisk inside `boot/boot.img` (hakchi's init, the memboot shell and the
installer run from it) and the package's own `bin/` and `lib/` (copied to the data partition).
Read from the pinned package on 2026-10-28 (version strings, `.comment`, `NEEDED`).

Toolchain: almost everything says `GCC: (Buildroot 2015.11.1) 4.9.3` and links glibc 2.22
(`GNU C Library (Buildroot) stable release version 2.22`). That glibc is not the console's
own (the stock system has glibc 2.23). hakchi.net keeps that Buildroot and its configuration in
`/hakchi/sdk/`; the tarball there is the official release (SHA-1
`a267a1fd403df2c3ac8b756ed03ab162ffac21e8`, as in the release notice). The configuration is a
general SDK (267 packages: Kodi, ffmpeg, Qt...); only its toolchain part matters here:
Cortex-A7, NEON-VFPv4, hard float, glibc 2.22, GCC 4.9.3 with C++, LTO, OpenMP and Graphite,
binutils 2.24, kernel headers 4.3. The sources were mostly newer than that Buildroot's own
packages (2018 releases).

| Program | Where | Version | Built with | Source |
|---|---|---|---|---|
| glibc and libgcc_s | ramdisk `lib32/` | 2.22 | Buildroot 2015.11.1 | the toolchain |
| BusyBox (375 applets) | ramdisk | `1.29.0.git` 2018-04-10 20:38 GMT | stripped | git `3632cb15f`, the last commit before that time |
| dropbear (SSH) | ramdisk | 2018.76 | Buildroot | matt.ucc.asn.au |
| e2fsprogs: e2fsck, mke2fs, resize2fs, tune2fs, libext2fs, libe2p, libcom_err, libss, e2initrd_helper | ramdisk and package | 1.44.1 (24-Mar-2018) | Buildroot | kernel.org |
| libblkid, libuuid | ramdisk | util-linux 2.27.1 | built in a tree under `Clover-release/.../clover-dp-hvc-final` (Nintendo's build layout), not the stock system's copies | kernel.org |
| cryptsetup | ramdisk | 1.6.8; popt and device-mapper linked in, libuuid shared | Buildroot | kernel.org (+ lvm2, popt) |
| kexec | ramdisk and package | kexec-tools 2.0.15 | Buildroot | kernel.org |
| rsync | ramdisk | 3.1.3 | Buildroot | samba.org |
| llmnrd, tinysvcmdns | ramdisk | no version string | Buildroot | GitHub (to pin) |
| sntool (sunxi-flash, sunxi-part, decodepng) | ramdisk and package | `5833d00 (dirty)` 2020-03-30: not in public hakchi | Buildroot | public hakchi `sntool/` (last change 2018-10) |
| clovershell | ramdisk `hakchi/shell/` | built 2018-03-19 | Buildroot | Cluster's (to locate) |
| sunxi-fexc, sunxi-nand-part, sunxi-pio | ramdisk and package | sunxi-tools v1.4.2-30/-54 (`5c19710`), dirty | Linaro GCC 4.8.3 2014.02 | linux-sunxi/sunxi-tools + hakchi's `sunxi-tools.diff` |
| fbgrab | package | (libpng 1.6.20) | Buildroot | to pin |
| aplay | package | alsa-utils 1.0.29 | stripped | alsa-project.org |
| xdelta3 | package | static, stripped | | to pin |

Not programs: `boot/uboot.bin` (step 3), `lib/firmware/rtlwifi/rtl8188fufw.bin` (Realtek
firmware blob).

## Building

```sh
scripts/userland/build.sh          # into target/userland/out
scripts/userland/build.sh --clean  # from scratch, the toolchain too
```

Sources are pinned in `scripts/userland/sources.txt` (tarballs by SHA-256, git by commit). The
toolchain is built once (about 3 minutes on 24 cores) and reused; it reports itself as
`(Buildroot 2015.11.1) 4.9.3`, like the shipped binaries, and produces the same ARM attributes
(Cortex-A7, VFPv4, NEON with fused multiply-add, hard-float calls).

## Status

| Program | State |
|---|---|
| Toolchain | built: `buildroot_defconfig`, madmonkey's configuration with every package off except the ones below |
| glibc 2.22 | built by the toolchain. **Byte-identical** to the ramdisk's: `libc`, `ld`, `libm`, `libdl`, `librt`, `libcrypt`, `libutil`, `libgcc_s`. Differ: `libpthread` (Buildroot leaves it unstripped for debuggers; stripped with `--strip-unneeded` it still differs), `libnss_dns`, `libnss_files`, `libresolv` (same size, other link layout) |
| libuuid (util-linux 2.27.1) | built by Buildroot: **byte-identical** |
| aplay (alsa-utils 1.0.29) | built by Buildroot: **byte-identical** |
| BusyBox | built: `busybox.config` gives the shipped 375 applets exactly; Thumb-2 like the shipped one; 646,488 bytes against 650,604 (the other options cannot be read from a stripped binary and are that commit's defaults) |
| libblkid (util-linux 2.27.1) | built by Buildroot; 197,832 bytes against 222,368 (the shipped one was built in another tree, see above) |
| fbgrab 1.2 (libpng 1.6.20) | built by Buildroot; 10,004 bytes against 9,208 |
| cryptsetup 1.6.8 | built by Buildroot with the kernel crypto API (the shipped one links no crypto library); still links libcryptsetup and popt as shared libraries where the shipped one has them, and device-mapper, built in |
| e2fsprogs 1.44.1 | built with Buildroot's recipe from the newer source (`local.mk`); same libraries; e2fsck 235,952 bytes against 220,872, mke2fs 100,968 / 96,928, resize2fs 51,844 / 45,436 |
| dropbear 2018.76 | same; `--disable-static` (Buildroot's `--enable-static` makes dropbear 2018 link itself statically); same libraries; 260,172 bytes against 219,944 |
| kexec-tools 2.0.15 | same, without zlib like the shipped one; 92,276 bytes against 47,160 |
| rsync 3.1.3 | same, with its bundled zlib and popt like the shipped one (`buildroot-patches/`); 375,568 bytes against 278,144 |
| llmnrd | built from `4dc24d0`, the commit the shipped binary names (`git id v0.5-4-g4dc24d0`) |
| tinysvcmdns | geekman's library; the shipped main program was never published, so `main-tinysvcmdns.c` replaces it with the same command line (`S93mdnsd`: instance, type, port, TXT records) |
| clovershell | Cluster's daemon at `5edb874`, the last commit before the shipped build; same strings |
| sunxi-fexc, sunxi-pio, sunxi-nand-part | sunxi-tools at the commits they report (`1e219c0`, `5c19710`), with our toolchain instead of Linaro GCC 4.8 |
| xdelta3 3.1.1 | jmacd's xdelta at `7508fd2` (the shipped binary, unpacked from UPX 3.94, reports 3.1.1; main had no commits from 2017-08 to 2026), without lzma like the shipped one, not UPX-packed |
| sntool | **not equivalent**: the public source (2018) builds a 12.5 KB tool with the boot-area commands; the shipped one (793 KB, `5833d00` 2020, unpublished) also has `sunxi-part` (`grow`, `shrink`, `tune`), `decodepng`, `ismd`, `isRecovery`, `mdui` and Mega Drive helpers. Those need a replacement of our own. |

The byte-identical files show that this toolchain and Buildroot are the ones the shipped binaries
were built with. Built programs can be run in the container with Ubuntu's `qemu-user` (`qemu-arm -L
target/userland/buildroot-2015.11.1/output/target <program>`): BusyBox lists its 375 applets,
llmnrd reports `v0.5 (git id v0.5-4-g4dc24d0)` like the shipped one.

The newer tools link the same libraries as the shipped ones, except that the
shipped ones also need `libgcc_s` and are 10 to 50% smaller: they were built outside Buildroot,
probably with flags like those in hakchi's `sntool/Makefile` (`-Os -mthumb -flto`,
`--gc-sections`). The difference is size, not behaviour.

## sntool's unpublished commands

hakchi's scripts call these, and only the shipped 2020 sntool has them:

| Command | Used by | Notes |
|---|---|---|
| `decodepng <png> > /dev/fb0` | boot logo, splash (`init`, `b0010_functions`) | **built**: public after all, DanTheMan827's decodepng at `ac7f36a` (hakchi's submodule pin) with LodePNG `c7bb19a`; its own program here instead of part of sntool. Under QEMU it turns hakchi.png into two 1280×720 32-bit frames |
| `sntool ismd`, `sntool mdui` | Mega Drive Mini checks | NES, SNES and Famicom answer no; Cartload refuses the Mega Drive |
| `sntool isRecovery` | `preinit` (loads `script/install`), `b0010_functions` (`gameover`: `shutdown` or `poweroff`) | true (exit 0) when running from hakchi's own ramdisk: observed 0 in the memboot shell (2026-10-29: `/init decrypt hakchi-shell hakchi-chainload`, `/newroot` mounted, kernel 3.4.113.29-madmonkey from RAM) and 1 in the normal system. The tool mentions `/newroot` and `cmdline`; which of them it tests is still to read from the binary |
| `sunxi-part` (list), `grow`, `shrink`, `tune`, `--dry-run` | Cartload's install (`grow`), hakchi's uninstall (`shrink`) and repair (`tune`), `ismdPartitioning` (list) | see below |

`sunxi-part` works on two levels, read from the shipped binary's messages:

1. The Linux partition table: Allwinner's MBR at the start of `/dev/nand`
   (`mbr: version 0x00000200, magic softw411`, several copies, checked one by one), the format
   sunxi-tools' `sunxi-nand-part` edits.
2. The NAND driver's own physical layout (`nandinfo0.bin`, `nandinfo1.bin`, `--nand-part`,
   `--nand-states`): the `_boot_info` block in the NAND boot area (kernel
   `drivers/block/nand/lib/src/phy/phy.h`: MBR, FTL partitions, storage info, factory bad
   blocks), read and written raw through hakchi's `phy_read`/`phy_write` ioctls.

Then it reloads the NAND driver (`reload_nand`: `nand.ko`) and resizes the data filesystem
(`resize2fs`, `tune2fs`). `grow` merges the unused `UDISK` partition into the data partition;
`shrink` undoes it. Getting either wrong can make the data partition unreadable (the console
itself recovers through FEL), so a replacement is developed against dumps of both levels and
checked against the shipped tool's `--dry-run` before it writes anything.
