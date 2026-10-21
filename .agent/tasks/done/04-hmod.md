# Task 04: crates/hmod, the kernel package

Read `docs/device-protocol.md` (hakchi.hmod section). A sample package is at
`~/Documents/Hakchi2-CE/hakchi_gui/bin/Release/net48/mods/hmods/hakchi.hmod` (read-only).

Implement:

- `hmod::Package::open(path)`: gzip tar reader (`flate2` + `tar`), listing entries, reading
  `var/version`, extracting `boot/boot.img` and `boot/uboot.bin` to memory, and a generic
  `read(path)`. Handle both `./path` and `path` entry names.
- `hmod::fetch_latest(dest)`: download from the two known URLs with a fallback, verify it opens
  and contains the boot files, write atomically, return the version string. `reqwest` with
  rustls.
- A `data/` directory at the repo root holding `fes1.bin` copied from the hakchi2 CE build tree,
  with a `data/README.md` noting its origin and GPL-3 license, and a `hmod::fes1()` accessor via
  `include_bytes!`.
- Update `cartload-fel memboot` to take `--hmod` only (no `--fes1`), using these.
- Tests: open the sample package by path from an env var `CARTLOAD_SAMPLE_HMOD` when set
  (skip otherwise), plus tests on a tiny synthetic tar.gz built in the test.

Verify, commit.
