# data

Binary blobs that the Rust crates embed at build time with `include_bytes!`. None of them is
part of `hakchi.hmod`; all three come from the hakchi2 CE tree and are redistributed unchanged.

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
