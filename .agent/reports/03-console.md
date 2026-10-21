# Report: 03-console

## What was done

`crates/console` now talks to a booted hakchi kernel. russh 0.64 (ring backend) for SSH,
suppaftp 12 (tokio client) for FTP, mdns-sd 0.21 for mDNS, if-addrs for interface enumeration.
Layout:

- `src/discover.rs`: `discover()` enumerates interfaces with a 169.254.x.x address, reads the
  Linux `operstate` from sysfs (falling back to if-addrs on other platforms) and accepts `up`
  and `unknown`, then TCP-connects to `169.254.13.37:22` from each such interface with a 500 ms
  timeout, reading the SSH banner when it arrives. mDNS (`_ssh._tcp.local.`, instance `hakchi`,
  TXT `hwid`/`type`/`region`) is behind `DiscoverOptions::mdns`; `discover_all()` runs both and
  merges the mDNS metadata into the USB-link entry. Also `link_local_interfaces()`, `probe()`,
  `discover_mdns()`.
- `src/session.rs`: `Session::connect[_with]` (root, empty password; connect, kex and auth each
  under a timeout), `exec(cmd)` / `exec_with_stdin` returning `ExecOutput { exit_code, stdout,
  stderr }` (one channel per command, stdin sent concurrently with reading, per-command
  timeout, default 60 s), `exec_checked`, `upload(bytes, path)` and `upload_executable` through
  `cat > path`, `run_temporary_script` (CE `RunTemporaryScript`), `ftp()`, `close()`. The host
  key is recorded (algorithm + SHA-256 fingerprint) and accepted: it is regenerated in RAM at
  every boot. Algorithm preferences are russh's defaults plus `ecdh-sha2-nistp256`,
  `diffie-hellman-group14-sha1` and `hmac-sha1` at the lowest priority for older kernels.
- `src/ftp.rs`: `Ftp` over suppaftp's async tokio client: `connect[_as]`, `mkdir` (550 =
  already exists, like CE), `mkdir_p`, `upload` / `upload_reader` / `upload_file` with a
  `Progress { done, total }` callback every 256 KiB, `list`, `names`, `size`, `cwd`, `pwd`,
  `remove_file`, `remove_dir`, `quit`.
- `src/hakchi.rs`: `Shell` trait (implemented by `Session`, mocked in tests) and
  `Hakchi<'_, S>` with `find_game_sync_storage`, `get`, `eval`, `eval_script` (stdin),
  `current_firmware`, `hwid`, `mmc_used`, `is_md_partitioning`, `is_minimal_memboot`,
  `mount_base`, `umount_base`, `overmount_games`, `ui_pause`, `ui_resume`, `ui_start`,
  `ui_stop`, `versions`, `system_code`, `config`, `set_config`, `storage`, `storage_report`,
  `original_games` and `info()`. `info()` runs CE's `Shell_OnConnected` + `MemoryStats.Refresh`
  + `GameCacheTask.UpdateLocal` sequence in the trace's order and returns `ConsoleInfo`
  (hwid, md partitioning, minimal memboot, versions, can_interact, console type, system code,
  firmware, the four paths, config map, `StorageReport` with total/used/free bytes plus the CE
  games/saves breakdown, original game ids). The exact command strings live in
  `commands::*`.
- `src/parse.rs`: `ConsoleType` (CE's ten types + Unknown) with family, region, system code,
  pixel-art and squashfs-games-path tables; `Versions` with CE's `ConvertVersion` extraction and
  minimum versions (`requires_reflash`, `requires_rootfs_update`); `Firmware`; `df`/`du`
  parsers; `StorageReport::summary()` reproducing CE's eight log lines; shell assignment /
  `p0000_config` parsing and formatting; hwid, directory listing, YES/NO; `sh_quote`,
  identifier and device-path checks.
- `src/main.rs`: `cartload-console discover [--mdns] [--all] [--json] [--timeout-ms]`,
  `info [--host] [--json]`, `exec [--stdin] <cmd...>` (remote exit code passed through),
  `ls [--ftp] <path>`. Logs go to stderr; `--host` skips discovery; behind a default `cli`
  feature like the fel crate.
- `docs/hardware-tests.md`: "Console connect" checklist with the real outputs and a results
  row. `docs/device-protocol.md`: the dropbear `none`-auth behaviour, FTP server details, the
  mDNS record, the script's exit-code quirks and the identification half of the connect
  sequence, all verified on the console.

## Verified

- `cargo test -p cartload-console`: 40 tests, no hardware. The parsers are tested with the
  trace's strings (version triple, hwid, system code, `_nand_`, the storage summaries before
  and after the sync reproduced byte for byte, the 21 SNES EUR game ids). A scripted `Shell`
  replays the trace's connect sequence and asserts every command string verbatim and in order,
  plus the minimal-memboot and too-old-kernel early exits, the squashfs fallback, the wrapper
  command strings (including stdin for `set_config`) and error paths. Network tests use
  loopback listeners and TEST-NET-1 only.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`,
  `cargo test --workspace`, `cargo doc -p cartload-console --no-deps`: clean.
- Hardware (SNES Classic Mini EUR, hakchi v1.0.4-126, reachable during the run; only the
  read-only commands the brief allows): `discover` found the console on `hakchi0` (operstate
  `unknown`) in ~10 ms with banner `SSH-2.0-dropbear_2018.76`; `discover --all --mdns` also
  resolved the mDNS announcement (hwid, `type=snes`, `region=eur`) and merged it. `info`
  negotiated curve25519-sha256 / ecdsa-sha2-nistp256 / aes256-ctr / hmac-sha2-256 and printed
  the same values as the trace: hwid `0461872a872802079d2818c76c118000`, boot 1.0.3, kernel
  3.4.113.29-madmonkey, script v1.0.4-126, `snes-eur`, `_nand_`, `/var/lib/hakchi/games`, the
  post-sync storage block (`352.6MB` / `31.2MB` / `310.5MB`, 3.0MB in `snes-eur`), the config
  and the 21 original games. `exec` and `ls` were not run on hardware.

## Decisions

- Authentication: the first `info` run was refused. dropbear (`-B`) grants a blank password
  only through the SSH `none` method and rejects the `password` method for the same account.
  `Session::connect` now tries `none` first and falls back to `password`. Documented in
  `device-protocol.md`; this is the one thing the protocol notes had wrong by omission.
- Non-zero exits are errors only where CE throws: `ui_*` ignore the status (the script's last
  command is a `killall` of the Mega Drive UI process and fails on NES/SNES), `overmount_games`
  returns `bool`, `eval` returns the raw `ExecOutput`.
- Device paths are interpolated inside double quotes exactly as CE does (so the trace strings
  match) but are rejected if they contain quotes, `$`, backquotes, backslashes or control
  characters; caller-supplied strings elsewhere go through `sh_quote`.
- `original_games` is skipped on Mega Drive (CE does the same) and falls back to CE's
  `GamesSquashFsPath` when `$squashfs$gamepath` is not a directory.
- russh uses the `ring` backend instead of the default `aws-lc-rs` to avoid the cmake/NASM
  build dependency on Windows/macOS. Compression is off.
- The TCP probe binds to each link-local interface's address; the kernel still routes by
  destination, so with several 169.254 interfaces the first route wins. One interface is the
  normal case.
- The scratch extraction of the hmod/ramdisk for reading the device script was done under
  `/tmp`, nothing under `~/Documents/Hakchi2-CE` was modified.

## Left / uncertain

- `exec` and `ls` (SSH and FTP) have not been exercised on hardware; the FTP upload path
  (`mkdir_p`, `upload_*`, progress) is only compile- and unit-tested until task 06 syncs a
  library. busybox `ftpd` replies for `MKD` on an existing directory are assumed to be 550.
- mDNS was verified only on the USB link (the console announces on it too); a Wi-Fi console was
  not available.
- Windows/macOS: interface operstate falls back to if-addrs' `IFF_RUNNING`-based status, not
  tested.
