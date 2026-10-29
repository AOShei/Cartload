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
| sntool (sunxi-flash, sunxi-part, decodepng) | ramdisk and package | `5833d00 (dirty)` 2020-03-30: not in public hakchi | Buildroot | public hakchi `sntool/` (last change 2018-10); ours: the fork's branch `cartload` |
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

## Assembling the system

```sh
scripts/userland/assemble.sh [hakchi checkout]   # into target/assemble/out
```

Builds the boot ramdisk, `boot.img` (our kernel with that ramdisk) and the package
(`cartload-system-<version>.hmod`, the version from `SYSTEM_VERSION`) from `scripts/kernel/build.sh`'s and `scripts/userland/build.sh`'s output and
the hakchi fork's scripts (`mod/`, `kernel-hmod/`: hakchi v1.0.4-126's as shipped, commit
`ca39431`, then our changes). `scripts/userland/manifest.txt` names the source of every file; the
shipped package only gives the layout (directories, links, file list), and a shipped program
without a manifest line stops the build. `report.txt` lists every file with its source and
SHA-256. Two runs give the same bytes.

First result (2026-10-29): `boot.img` 4,212,736 bytes (our kernel's current one, with the
shipped ramdisk, is 4,712,448; u-boot loads 4,980,736), ramdisk 2,235,440 bytes xz. Every
ramdisk program runs under qemu-arm with the ramdisk as its root (libraries found). Same layout
as the shipped ramdisk and package except: `decodepng` its own program, `e2initrd_helper` left
out (nothing calls it; Buildroot does not install it), `libcryptsetup`, `libpopt` and
`libdevmapper` added (hakchi's cryptsetup had them built in). Nothing is taken from the shipped package any more: `detached-fallback` (a shell script with
hakchi's memboot functions) comes from the hakchi fork, `uboot.bin` from `scripts/uboot/`
(docs/uboot.md), and Realtek's RTL8188FU Wi-Fi firmware, a binary Realtek publishes without
source, from linux-firmware at the commit that added it (`9aa8db1c`, the same bytes as hakchi's
copy).

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
| tinysvcmdns | geekman's library; the shipped main program was never published, so `main-tinysvcmdns.c` replaces it with the same command line (`S93mdnsd`: instance, type, port, TXT records). The console has no default route, so the library's join of the mDNS group on "any interface" fails (`IP_ADD_MEMBERSHIP: No such device`; hakchi's copy of the library was modified too): `tinysvcmdns-patches/` joins on each interface instead. On a SNES it then announces what the shipped one does (`hakchi` `_ssh._tcp` on `madmonkey.local`, 169.254.13.37, port 22, the same TXT records) |
| clovershell | Cluster's daemon at `5edb874`, the last commit before the shipped build; same strings |
| sunxi-fexc, sunxi-pio, sunxi-nand-part | sunxi-tools at the commits they report (`1e219c0`, `5c19710`), with our toolchain instead of Linaro GCC 4.8 |
| xdelta3 3.1.1 | jmacd's xdelta at `7508fd2` (the shipped binary, unpacked from UPX 3.94, reports 3.1.1; main had no commits from 2017-08 to 2026), without lzma like the shipped one, not UPX-packed |
| sntool | **partly ours**: the public source (2018) has the boot-area commands (`sunxi-flash`, `check`, `kernel`, `devmem`); the shipped one (793 KB, `5833d00` 2020, unpublished) also has `sunxi-part`, `isRecovery`, `ismd`, `mdui`, `decodepng`, a `clover-mcp` mode and Mega Drive helpers. `sunxi-part`, `isRecovery`, `ismd` and `mdui` are now in the hakchi fork (branch `cartload`, `a3e8359`, pinned in `sources.txt`) and match the shipped tool on a console backup (below); `decodepng` is its own program; the `clover-mcp` mode is still to do |

The byte-identical files show that this toolchain and Buildroot are the ones the shipped binaries
were built with. Built programs can be run in the container with Ubuntu's `qemu-user` (`qemu-arm -L
target/userland/buildroot-2015.11.1/output/target <program>`): BusyBox lists its 375 applets,
llmnrd reports `v0.5 (git id v0.5-4-g4dc24d0)` like the shipped one.

The newer tools link the same libraries as the shipped ones, except that the
shipped ones also need `libgcc_s` and are 10 to 50% smaller: they were built outside Buildroot,
probably with flags like those in hakchi's `sntool/Makefile` (`-Os -mthumb -flto`,
`--gc-sections`). The difference is size, not behaviour.

## sntool's unpublished commands

hakchi's scripts call these, and only the shipped 2020 sntool has them. Their behaviour was read
by running that sntool under qemu-arm against copies of a console backup
(`scripts/userland/fakenand/run.sh`: a preload library answers the NAND ioctls from the backup's
boot area and logs, instead of running, the programs it starts), with a look at the
disassembly where a trace did not say enough.

| Command | Used by | What it does |
|---|---|---|
| `decodepng <png> > /dev/fb0` | boot logo, splash (`init`, `b0010_functions`) | **built**: public after all, DanTheMan827's decodepng at `ac7f36a` (hakchi's submodule pin) with LodePNG `c7bb19a`; its own program here instead of part of sntool. Under QEMU it turns hakchi.png into two 1280×720 32-bit frames |
| `sntool isRecovery` | `preinit` (loads `script/install`), `b0010_functions` (`gameover`: `shutdown` or `poweroff`) | exit 0 if `/lib/modules-ramfs/kernel/drivers/block/nand/nand.ko` exists, a file only hakchi's boot ramdisk has; nothing else is checked. Observed on a console: 0 in the memboot shell, 1 in the normal system |
| `sntool ismd` | Mega Drive checks (`install`, `ismdPartitioning`, sunxi-part) | exit 0 if the 32-bit word at physical `0x01c23818` (chip ID area, read through `/dev/mem`) is `0x005a7213`, else 1 |
| `sntool mdui` | | always exit 0 |
| `clover-mcp` (a link to sntool) | the ramdisk's `rootfs/usr/bin/clover-mcp`, which hakchi copies into the console's overlay: Nintendo's `S81clover-mcp` (the menu) starts it | **not done**. Traced: it looks up `/dev/input/by-path/platform-gpio-keys.0-event-kbd` (the console's buttons), syncs, forks a child (which quits without that device), raises its priority to -15 and runs Nintendo's `/var/squashfs/usr/bin/clover-mcp`. On a SNES it does (checked 2026-10-29, docs/hardware-tests.md): Nintendo's program runs under it at nice -15, and two sntool processes (nice 10 and 18) stay behind with no device open, polling on a timer; traced under emulation: helper A (nice 10) runs `modprobe uinput`, binds UDP `127.0.0.1:1001`, starts helper B and loops every 5 ms; B (nice 18) checks every 0.9 s that A is alive (`/proc/<A>/status`) and looks at `/dev/input/` for new devices. The binary bundles libevdev (MIT), a database of about 150 USB gamepad names (Logitech, HORI, Mad Catz...) and the name "Nintendo Clovercon - controller": a USB gamepad driver presenting gamepads as Nintendo controllers through uinput (older hakchi had it as `S80joydriver`). No published source found. On a SNES with a Nintendo controller plugged in (2026-10-29) the helpers have only that UDP socket open (nothing queued); the controller (`event4`) is open only in Nintendo's `clover-mcp` and the menu (`ReedPlayer-Clover`); `uinput` is loaded and unused. **Decision (the user, 2026-10-29): Cartload's system leaves this driver out for now** and starts Nintendo's `clover-mcp` directly; USB gamepads (through an OTG hub) wait for a rewrite someone can test |
| `sunxi-part` and its commands | Cartload's install (`grow`), hakchi's uninstall (`shrink`) and repair (`tune`), `ismdPartitioning` (list) | below |

`sunxi-part` reads the four u-boot copies (NAND blocks 0x08-0x27) with `phy_read` and the Linux
partition table from `/dev/nand`, then:

- no command: prints the table as `mbr::32768 nanda:boot:6144 nandb:rootfs:131072
  nandc:data:741376` (name, label, sectors).
- `grow` and `shrink` change something only from hakchi's ramdisk (the `isRecovery` test) and
  when the layout is not already the asked one; otherwise they print the table and exit 0.
  `--dry-run` prints the new table and `(not) writing <grow|shrink> partition table...` (on
  stderr) and changes nothing.
- `shrink` (grown to factory): `hakchi umount_base`; shrinks the data filesystem to 612,352
  sectors (1 MiB under the factory partition: `fsck.ext4 -fy`, `resize2fs -fs` (64bit off),
  `resize2fs -f /dev/nandc 612352s`, `tune2fs -m 3`, `fsck.ext4 -fy`); rewrites the boot-info
  block of u-boot copies 1 and 2 (blocks 0x0d and 0x14, `nand sectors written: 2`) with the
  factory layout; `hakchi reload_nand`; `echo y | sunxi-nand-part -f a20 /dev/nand 32768 'boot 6144
  32768' 'rootfs 131072 32768' 'data 614400 32768' 'private 2048 32768' 'UDISK 0 0'`; then what
  `tune` does.
- `grow` (factory to grown): `hakchi umount_base`; the boot-info block of copies 1 and 2 with
  data = data + private + UDISK (741,376) and no private or UDISK; `hakchi reload_nand`;
  `sunxi-nand-part` as above with `'data 741376 32768'` and nothing after it; then `tune`.
- `tune`: `hakchi umount_base`, `fsck.ext4 -fy`, `resize2fs -fs`, `resize2fs -f` (fill the
  partition), `tune2fs -m 3` (3% reserved), `fsck.ext4 -fy`, all on `/dev/nandc`. **It ignores
  `--dry-run`**: never run the shipped `sunxi-part tune --dry-run` on a console expecting no
  change.

Copies 3 and 4 are never written, so a console keeps its factory layout there (the fixtures:
`scripts/userland/fixtures/`). From the factory block the shipped `grow` writes exactly the
grown block found on a console, and `shrink` then `grow` gives the boot area back byte for byte.
The shrunk block is the factory layout but not the factory bytes (see the fixtures' README).

### Our replacement

`sntool/sunxi_part.c` in the hakchi fork (branch `cartload`). Written against the shipped tool's
behaviour, not its code, and tested two ways by `scripts/userland/sntool-test/run.sh`:

1. the layout logic, built for the host, against the fixtures: from the factory and from the
   shrunk block `grow` writes the console's grown block byte for byte, `shrink` writes exactly
   the shipped tool's shrunk block, damaged blocks and unknown layouts are refused;
2. with a console backup (`BACKUP=<dir>`), ours and the shipped tool under qemu-arm
   (`scripts/userland/fakenand/`) on 15 cases: listing, grow and shrink from the grown, shrunk and
   factory layouts, dry runs, outside the recovery shell, each filesystem step failing. Same
   boot area, same Linux partition table, same output, same commands in the same order.

The same test flashes u-boot and the kernel as hakchi's install and uninstall do
(`sunxi-flash burn_boot1`, `burn_boot2`, and `echo default | sunxi-flash burn_boot1`). The public
code differed from the shipped tool in three ways, all fixed (fork `cd99085`), found when the
first install of our package erased copy 1's boot-info (docs/hardware-tests.md):

- `burn_boot1` padded the end of u-boot's last block with 0xff: u-boot (0xba000 bytes) ends in
  the block holding the boot-info, so flashing u-boot erased copy 1's layout. The shipped tool
  keeps the rest of that block, and so does ours now (`burn_boot2` still pads with 0xff, as the
  shipped tool does after a kernel).
- `burn_boot1` wrote any input; the shipped tool writes only a u-boot image.
- `read_boot1` appended the boot-info after u-boot; the shipped tool reads u-boot alone, which
  hakchi's `flashBoot1` compares with the uboot.bin it flashed (fork `2c20d1a`, found by the second
  install: `flash md5 mismatch!`). `read_boot1 <n>` with a copy number (no script uses it) still
  counts copies five blocks apart, the public code's guess, where the shipped tool finds them.
- `default` (hakchi's uninstall) wrote the word into copy 1. The shipped tool puts Nintendo's
  u-boot back in every copy, as on a console from the factory: u-boot and its script from a copy
  hakchi never writes, joined again (u-boot padded to 64 KiB, the script to 8 KiB, header
  lengths and checksum, 0x8c000 bytes) with the current boot-info in copies 1-2 and the factory
  one in the others, five blocks apart, the last two blocks the start of a seventh copy.

Where ours differs, on purpose (also checked by the test):

- `tune` runs only from the recovery shell and `tune --dry-run` runs nothing (the shipped one
  runs fsck and resize2fs on `/dev/nandc` in both cases);
- each block written is read back; a failed write on the first u-boot copy stops before the
  second, and before the driver reload and the Linux table;
- nothing is changed while `/dev/nandc` is mounted;
- `fsck.ext4` exit code 1 ("errors corrected") counts as success (the shipped one stops);
- `shrink` takes the factory sizes from the copies hakchi never writes (copies 3 and 4) rather
  than built-in numbers, and refuses without them;
- the shipped tool's other options (`--nand-part`, `--backup`, `--file`, `dump`...), which hakchi's
  scripts do not use, are refused;
- `burn_boot1` with a u-boot image followed by more data writes only the header's length (the
  shipped tool writes the rest over the boot-info).

