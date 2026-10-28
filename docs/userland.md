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
| e2fsprogs 1.44.1, dropbear 2018.76, kexec-tools 2.0.15, rsync 3.1.3 | to do: newer than this Buildroot's |
| llmnrd, tinysvcmdns, sntool, clovershell, sunxi-tools, xdelta3 | to do |

The byte-identical files show that this toolchain and Buildroot are the ones the shipped binaries
were built with.
