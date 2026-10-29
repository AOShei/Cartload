# data

Binary blobs that the Rust crates embed at build time with `include_bytes!`. `hakchi.hmod` is
the kernel package itself and `tiny7zx.hmod/` one of hakchi2 CE's base mods; the rest are not
part of either. All come from the hakchi2 CE tree or its releases and are redistributed
unchanged.

## hakchi.hmod

| | |
| --- | --- |
| Origin | hakchi2 CE's release asset `supplemental/hakchi-v1.0.4-126.Fri.03.Apr.2020.11_12_31.AM.GMT.hmod` (<https://github.com/TeamShinkansen/Hakchi2-CE/releases>); byte-identical to `https://hakchi.net/hakchi/hmods/hakchi-latest.hmod` and to CE's build output `mods/hmods/hakchi.hmod`, all three checked on 2026-10-28 |
| Version | `hakchiVersion='v1.0.4-126'`, `bootVersion='1.0.3'`, `kernelVersion='3.4.113.29-madmonkey'` |
| Size | 9 340 382 bytes |
| MD5 | `2e163a25c98b4c05bed274817abb38da` |
| SHA-256 | `003994b46504959895f40b0b9e91b16951ef4a638feb6312314c8fc4fdbff592` (pinned as `cartload_hmod::pinned::PINNED_SHA256`, checked by `cargo test -p cartload-hmod`) |
| Embedded by | `crates/hmod` with its `bundled` feature, exposed as `cartload_hmod::pinned::bundled()`; the app writes it to `<cache dir>/cartload/hakchi.hmod` and `cartload-hmod bundled -o FILE` writes it anywhere |

The hakchi kernel package that Cartload installs: madmonkey's hakchi, the last build of it,
as hakchi2 CE ships it. Carrying it means installing hakchi needs no server; the download
locations above remain as a fallback, and anything they serve is accepted only if its SHA-256
is the pinned one.

Contents and licenses (a gzip tar):

- `boot/boot.img`: Linux 3.4.113 for the Allwinner R16 (Nintendo's published kernel source with
  madmonkey's patches) and hakchi's ramdisk. GPL-2.0 (kernel), GPL-3.0 (hakchi scripts).
- `boot/uboot.bin`: U-Boot. GPL-2.0.
- `bin/`, `lib/`: sntool and the sunxi tools (GPL-2.0), kexec (kexec-tools, GPL-2.0),
  e2fsprogs libraries (GPL-2.0 / LGPL-2.0), xdelta3 (Apache-2.0), fbgrab and aplay (GPL-2.0).
- `lib/modules/`: kernel modules, GPL-2.0.
- `lib/firmware/rtlwifi/rtl8188fufw.bin`: Realtek RTL8188FU Wi-Fi firmware, a binary blob under
  Realtek's redistribution terms as carried by linux-firmware; not GPL.
- `install`, `uninstall`, `etc/preinit.d/`, `var/version`: hakchi scripts, GPL-3.0.

Source: hakchi's build tree is <https://github.com/madmonkey1907/hakchi> (GPL-3.0), forked
(master) to <https://github.com/AOShei/hakchi>; the kernel is forked to
<https://github.com/AOShei/sun-nontendocm-kernel>. Both were also submitted to Software
Heritage's "Save code now" on 2026-10-28. Its
submodules hold the kernel (<https://github.com/madmonkey1907/sun-nontendocm-kernel>),
sunxi-tools, e2fsprogs and the boot-image tools; U-Boot is only a patch there
(`r16-uboot/r16-uboot.diff`) against an upstream tree its readme names. Its public history ends in April 2019 at `fd525ac` (`v1.0.3` plus 47 commits);
the source of the exact v1.0.4-126 build (April 2020) is not published anywhere we could find
(GitHub, the 36 forks, hakchi.net, Software Heritage; checked 2026-10-28). The version number
is hakchi's total commit count from a tree that was never pushed: public commits already name
v1.0.4-121 to -130 in 2018 and 2019, the shipped kernel is `3.4.113.29` where the public kernel
tree is at `.24`, and the build supports the Mega Drive Mini (September 2019), which no public
commit does. About a fifth of the shipped ramdisk scripts differ from public master. Decision
(2026-10-28): ship with the closest public source linked (the forks above) and revisit if
anyone asks. A rebuild of the kernel from the public tree, level with the shipped one's
configuration and drivers, is under way: `docs/kernel.md`.

## tiny7zx.hmod/

| | |
| --- | --- |
| Origin | hakchi2 CE source tree, `hakchi_gui/mods/hmods/tiny7zx.hmod/` (<https://github.com/TeamShinkansen/Hakchi2-CE>, commit `74707cd6`, the same files as at the pinned `4fb3092c` of `crates/hmod/src/basemods.rs`) |
| Files | `bin/tiny7zx` (26 578 bytes, md5 `eb8fa3b5a0d8a832e81eec9b5fccb60b`), `install` (37 bytes, md5 `9fc874cd8fe53b3662fdd9446b7f38cc`), `readme.md` (3 501 bytes, md5 `b19c71bef9cbc6f8e42ba5743aad0ec6`), checked against the catalog by `cargo test -p cartload-hmod` |
| Embedded by | `crates/hmod` with its `bundled` feature, as `basemods::TINY7ZX.load_bundled()`; the app installs it after every hakchi install |

The 7z extractor the console's emulator wrappers run when a game's ROM is a `.7z` archive:
pts-tiny-7z-sfx by Péter Szabó, packaged as an hmod by cluster, madmonkey and KMFDManic.

License: pts-tiny-7z-sfx is GNU General Public License v2.0 (its source is
<https://github.com/pts/pts-tiny-7z-sfx>); the packaging is part of hakchi2 CE (GPL-3.0).
Redistributed unchanged.

## fes1.bin

| | |
| --- | --- |
| Origin | hakchi2 CE source tree, `hakchi_gui/data/fes1.bin` (<https://github.com/TeamShinkansen/Hakchi2-CE>) |
| Size | 14 048 bytes |
| MD5 | `2d1c7b3b9fe5f88bc93aef662abe5cfe` |
| SHA-256 | `2ccd22b8db392153643ba06e9189a81df7a11412044fabb092368a25c31685da` |
| CRC-32 | `0x93aada9f` (checked by `cargo test -p cartload-hmod`) |
| Embedded by | `crates/hmod`, exposed as `cartload_hmod::fes1()` and run by `cartload-fel memboot` |

`fes1.bin` is the Allwinner first-stage "FES" binary for the R16 SoC used by the NES, SNES and
Mega Drive Classic Mini (it starts with the Allwinner `eGON.BT0` header). In FEL mode the boot
ROM can only use SRAM; memboot uploads this binary to `0x2000` and executes it to bring up DRAM
before u-boot and the kernel are loaded (`docs/device-protocol.md`, "FEL memboot"). It is not part
of `hakchi.hmod`, which is why it ships here.

License: the file is distributed with hakchi2 CE, whose repository is licensed under the GNU
General Public License v3.0, and with madmonkey's hakchi. Cartload redistributes it unchanged on
the same terms as the rest of this repository (GPL-3.0-or-later). Do not modify it.

## kexec.static

| | |
| --- | --- |
| Origin | hakchi2 CE source tree, `hakchi_gui/tools/arm/kexec.static` (<https://github.com/TeamShinkansen/Hakchi2-CE>, commit `74707cd6`); the same file ships in CE's build output as `tools/arm/kexec.static` |
| Size | 722 876 bytes |
| MD5 | `85e2470a0c78c9876536be7d9798c004` |
| SHA-256 | `b38abcb593cf397cfb444faf81c9b0c991026b5187366dcc6802a10942df7bda` |
| Embedded by | `crates/flow`, exposed as `cartload_flow::tools::kexec_static()` and uploaded by the kexec memboot |

A statically linked, stripped 32-bit ARM build of `kexec` from kexec-tools, made by the hakchi
project for the Classic Mini's 3.4 kernel. hakchi2 CE uploads it to `/tmp/kexec/kexec` before
every memboot from a running hakchi kernel instead of trusting the `kexec` in the console's
ramdisk or rootfs (`MembootTasks.Memboot`); Cartload does the same under `/tmp/cartload/kexec`
(`docs/device-protocol.md`, "Memboot from a running hakchi kernel").

License: kexec-tools is GNU General Public License v2.0; the binary is redistributed unchanged
as hakchi2 CE redistributes it. Cartload's own code is GPL-3.0-or-later, which is compatible
with distributing a GPL-2 program alongside it. Do not modify it.

## detached-fallback

| | |
| --- | --- |
| Origin | hakchi2 CE source tree, `GeneratedData/tools/arm/detached-fallback` (same repository and commit); copied into CE's build output as `tools/arm/detached-fallback` |
| Size | 1 873 bytes |
| MD5 | `7a3607d5bc19bef626f03eb120819368` |
| SHA-256 | `c6d3d498fa400f1dc0f0ab021d1465347f5b9d9756c95a3b37c68363f644ed2f` |
| Embedded by | `crates/flow`, exposed as `cartload_flow::tools::detached_fallback()` and uploaded by the kexec memboot |

A POSIX shell script holding a standalone copy of hakchi's `memboot`, `reboot_kexec` and
`recovery` functions. Run as `detached-fallback recovery [boot.img]` it re-executes itself in
the background (`--detached`), sources `/etc/preinit`, prepends its own directory to `PATH` so
that `kexec` resolves to `kexec.static` uploaded next to it, loads the image (or the kernel in
NAND) with `hakchi-shell` on the command line, stops the UI, unmounts everything and runs
`kexec -e`, which brings the console back in the memboot shell.

License: part of hakchi (madmonkey1907/hakchi) as shipped in hakchi2 CE, GNU General Public
License v3.0. Cartload redistributes it unchanged on the same terms as the rest of this
repository (GPL-3.0-or-later). Do not modify it.

## cartload-system-0.1.0.hmod

| | |
| --- | --- |
| Origin | Built by this repository: `scripts/kernel/build.sh`, `scripts/userland/build.sh`, `scripts/uboot/build.sh`, then `scripts/userland/assemble.sh` (docs/userland.md, docs/uboot.md); every file's source is in the build's `report.txt` |
| Size | 8 631 421 bytes |
| SHA-256 | `5a8d8e35dd3f4f4d4b32bde4f78d358c2d608962130ee22fa48b4de36669b26f` |
| Embedded by | `crates/hmod` with the `bundled` feature, as `cartload_hmod::pinned::CARTLOAD` (the package the app installs by default) |

Cartload's own console system, version 0.1.0 (`cartloadVersion` in its `var/version`): our
kernel, u-boot from Nintendo's source, and the ramdisk and package programs built from source,
laid out like hakchi v1.0.4-126 with its scripts (from the hakchi fork). The builds are
reproducible; CI rebuilds this file from the pinned sources and fails if the bytes differ.

License: the parts' own (GPL-2.0 for the kernel and u-boot, GPL-3.0 for hakchi's scripts and
sntool, and the licenses of BusyBox, glibc and the other programs listed in docs/userland.md),
with Realtek's RTL8188FU firmware from linux-firmware under its redistribution license.

