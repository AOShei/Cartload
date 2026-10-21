# Report: 04-hmod

## What was done

`crates/hmod` reads, validates and downloads `hakchi.hmod`, and carries `fes1.bin`. Layout:

- `src/package.rs`: `Package::open(path)` streams the gzip tar once (`flate2` + `tar`) to build
  the listing (`Entry { name, size, kind, link_target }`, `EntryKind::{File, Directory, Symlink,
  HardLink, Other}`), rejecting non-gzip, truncated and empty archives. Names are normalised
  (`./x`, `/x`, trailing `/` → `x`) and every lookup accepts either form; when a name occurs
  twice the last one wins, as with tar extraction. `read(name)`, `version()`, `boot_image()`,
  `uboot()`, `boot_files()` (all three in one pass, stopping at the furthest wanted entry),
  `validate()` (non-empty `boot/boot.img` and `boot/uboot.bin`, parseable `var/version`),
  `entry()`, `contains()`, `entries()`, `path()`. Reads re-open the file; nothing holds the
  package in memory as a whole.
- `src/version.rs`: `Version { hakchi, boot, kernel, variables }` parsed from the shell
  assignments in `var/version` (single, double and no quotes, `export`, comments), with
  `Display` = `v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)`.
- `src/fetch.rs`: `fetch_latest(dest) -> Result<String>` (the `hakchiVersion`) over
  `fetch_from(urls, dest, progress) -> Result<Fetched { url, path, len, version }>`.
  Sources in order: the GitHub `supplemental` release asset hakchi2 CE ships, then
  `https://hakchi.net/hakchi/hmods/hakchi-latest.hmod`. Each source is tried in turn on any
  failure (request error, non-2xx, short body, or a download that does not `validate()`); the
  body streams to `<dest>.partial` in the destination directory, is fsynced, opened and
  validated in `spawn_blocking`, then renamed over `dest`. Partial files are removed on every
  error path; parent directories are created. reqwest 0.13 with `rustls-no-provider`, and
  `rustls` with the `ring` provider installed as process default (same backend as russh in
  `crates/console`; avoids aws-lc-rs and its cmake/NASM needs). 20 s connect, 60 s read timeout.
- `src/error.rs`: `thiserror` enum; `AllSourcesFailed(Failures)` lists every source's error,
  and `Download` includes reqwest's cause chain (the top-level message alone is just
  "error sending request").
- `src/lib.rs`: `fes1()` via `include_bytes!` of `data/fes1.bin`; `test_support` builds
  synthetic archives writing header names byte for byte (the `tar` builder would strip `./`).
- `src/main.rs`: `cartload-hmod info|list [--long]|extract [-o]|fetch [--dest] [--url ...]|fes1
  [-o]`. Features: `fetch` and `cli` (default on); `cartload-fel` uses the crate with
  `default-features = false`, so the FEL tool does not link HTTP.
- `data/fes1.bin` (14 048 B, md5 `2d1c7b3b…`, copied from `hakchi_gui/data/fes1.bin` in the
  hakchi2 CE source tree, the only place it exists; the build tree has no `data/` copy) and
  `data/README.md` with origin, checksums and the GPL-3 note. `CLAUDE.md` lists `data/`.
- `cartload-fel memboot --hmod PATH` only: `--fes1` is gone, the boot files come from
  `Package::boot_files()`, fes1 from `cartload_hmod::fes1()`. flate2/tar removed from fel.
- Docs: `hardware-tests.md` FEL memboot commands and expected output updated;
  `device-protocol.md` hmod section records the third CE URL (`HakchiUpdateURL`), the redirect
  targets, the file size and md5, the `var/version` format and the `eGON.BT0` fes1 header.

## Verified

- `cargo test -p cartload-hmod`: 31 tests, no hardware or network. Synthetic archives cover
  mixed `./name` / `name` entries, directories, a symlink, the root `./` entry, both read name
  forms, one-pass boot files, duplicate names, missing / non-file / empty entries, non-gzip,
  truncated and empty input, every `validate()` failure, and the `fes1()` blob (length,
  `eGON.BT0` magic, CRC-32 `0x93aada9f`). Fetch tests run a canned HTTP/1.1 server on
  loopback: 404 then success (fallback, atomic write into a new directory, no `.partial` left,
  progress 0 → total), replacing an existing file, garbage and boot-file-less downloads
  (`Unusable` with the inner reason, nothing left behind), HTTP 500 and a closed port, no
  sources, bad destination. `opens_the_sample_package_from_env` runs with
  `CARTLOAD_SAMPLE_HMOD` pointing at the CE package (239 entries, sizes 4 585 472 / 761 856,
  symlink target, version string) and prints "skipping" without it; both paths exercised.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, `cargo doc -p cartload-hmod --no-deps`, and `cargo check` of the
  crate with `--no-default-features` and with `--features fetch` only: all clean.
- CLI against the real package: `info` (version, 239 entries: 150 files, 77 dirs, 12 links,
  "valid kernel package"), `list --long | head` (no broken-pipe error), `extract ./var/version`
  and `boot/uboot.bin` (761 856 B), missing entry → exit 1, `info Cargo.toml` → "not a readable
  gzip tar archive: invalid gzip header", `fes1 -o` writes a file with the expected md5.
- `cartload-fel memboot --hmod "$HMOD" --dry-run` prints the same image facts as before
  (uboot 761 856 B, bootcmd at 0x6a54b, boot.img 4 585 472 B → 4 587 520 uploaded, cmdline
  ending in `hakchi-shell`); `--fes1` is rejected by clap. No FEL device was attached, so no
  memboot was run.
- Real downloads into a scratch directory under `/tmp` (removed afterwards): default sources
  → GitHub, 9 340 382 B in 0.6 s, md5 `2e163a25c98b4c05bed274817abb38da`, identical to the CE
  build-tree file; a dead first URL → hakchi.net fallback delivered the same bytes; only dead
  URLs → exit 1 with both causes listed (DNS failure chain, HTTP 404) and no file or directory
  created.

## Decisions

- Two sources as the brief says, GitHub first (pinned release asset) and hakchi.net second;
  CE's third URL is documented, not used.
- `fetch_latest` returns the version string per the brief; `fetch_from` is the richer API
  (progress callback, which source won, length) for the UI and the kernel flow.
- `boot_files()` and `validate()` require `var/version`: every hakchi kernel package since 2018
  has it, and the install flow needs the version anyway.
- `EntryKind` is not `#[non_exhaustive]` (tar's kinds are a closed set and the CLI matches on it).
- Validation of a download runs `Package::open` + `validate()` on the `.partial` file, so a
  truncated or corrupt transfer is rejected before the rename; the listing pass already
  decompresses the whole stream.
- The `.agent/tasks/03-console.md` move left by the driver stays uncommitted, as before.

## Left or uncertain

- Hardware memboot with the new `--hmod`-only command has not been run (no FEL device during
  this task); the image handling code is unchanged, only the file sources moved.
- hakchi.net's "latest" is still v1.0.4-126; if it ever moves ahead of the GitHub asset,
  callers wanting the newest package should put `HAKCHI_NET_URL` first in `fetch_from`.
- macOS/Windows: `rustls-platform-verifier` and the atomic rename are untested there.
