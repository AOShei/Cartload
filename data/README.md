# data

Binary blobs that the Rust crates embed at build time with `include_bytes!`.

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
