# Task 03: crates/console, talking to a booted console

Read `docs/device-protocol.md` (network, hakchi script, connect sequence) and
`docs/reference/hakchi-ce-sync-trace.log`. Reference C#: `hakchi_gui/hakchi.cs`,
`hakchi_gui/SshClient/`, `hakchi_gui/Tasks/SyncTask.cs` under `~/Documents/Hakchi2-CE`.

Implement:

- `console::discover()`: enumerate network interfaces, treat operstate `unknown` as up, and for
  every interface with a 169.254.x.x address try TCP connect to `169.254.13.37:22` with a short
  timeout. Return the reachable address. mDNS (`_ssh._tcp`, via the `mdns-sd` crate) as a second
  strategy behind a flag, for Wi‑Fi consoles.
- `console::Session`: SSH (the `russh` crate) as `root` with an empty password; `exec(cmd)`
  returning exit code, stdout, stderr; `upload(bytes, remote_path)` via the shell (`cat > path`)
  for small files; an FTP client (port 21, same credentials, the `suppaftp` crate or similar) for
  bulk transfer with a progress callback.
- `console::Hakchi`: typed wrappers over the device script: `find_game_sync_storage`, `get(var)`,
  `eval(shell)`, `current_firmware`, `hwid`, `mmc_used`, `mount_base`/`umount_base`,
  `overmount_games`, `ui_pause`/`ui_resume`/`ui_start`, plus `info()` returning a struct with
  console type and region (derive from firmware/hwid the way hakchi2 CE does), kernel version,
  storage path, total/used/free bytes, and the list of original game IDs.
- CLI binary `cartload-console` with `discover`, `info`, `exec <cmd>`, `ls <remote path>`.
- Unit tests for parsing the `df`/`du` output formats and firmware strings using the exact
  strings in the trace log. No hardware in tests.

Verify: tests and clippy. Add a "Console connect" checklist to `docs/hardware-tests.md`.
If the console is reachable during your run (`ping -c1 169.254.13.37` answers), you may run
`discover` and `info`, which are read-only. Nothing else.

Commit.
