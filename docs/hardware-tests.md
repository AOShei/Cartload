# Hardware tests

Manual checklists for steps that need a real console. Automated tests never touch hardware;
anything that does lives here instead.

Checklists land per crate as each one gains hardware-facing functionality:

- `crates/fel`: FEL memboot (console in FEL mode, `1f3a:efe8`).
- `crates/console`: discovery, SSH and FTP against a booted hakchi kernel (`04e8:6863`).
- `crates/hmod`: kernel package install and version check on the console.
- `crates/library`: sync of a small library and verification on the console menu.
- `crates/flow`: the kernel install, memboot, dump and uninstall flows end to end.

Each checklist states the starting console state, the exact command to run, and the expected
output, so that a run can be reproduced and compared.

## FEL memboot

Crate: `crates/fel`, binary `cartload-fel`. Boots the hakchi kernel from RAM; nothing is written
to NAND, so a failed run is recovered by power-cycling the console.

### Prerequisites

- Linux host with the udev rule in place (this is what hakchi2 CE installs; same file works for
  Cartload):

  ```
  # /etc/udev/rules.d/99-hakchi.rules
  SUBSYSTEM=="usb", ATTR{idVendor}=="1f3a", ATTR{idProduct}=="efe8", MODE="0666", TAG+="uaccess"
  SUBSYSTEM=="net", ACTION=="add", ATTRS{idVendor}=="04e8", ATTRS{idProduct}=="6863", ATTRS{manufacturer}=="hakchi", NAME="hakchi0"
  ```

  then `sudo udevadm control --reload-rules`. A NetworkManager profile that gives `hakchi0` a
  link-local address (`ipv4.method=link-local`) is needed for the SSH check at the end.
- `hakchi.hmod` (hakchi2 CE build tree: `hakchi_gui/bin/Release/net48/mods/hmods/hakchi.hmod`,
  v1.0.4-126, or `cargo run -p cartload-hmod -- fetch --dest hakchi.hmod`, which downloads the
  same file: md5 `2e163a25c98b4c05bed274817abb38da`). `fes1.bin` (14048 bytes, md5
  `2d1c7b3b9fe5f88bc93aef662abe5cfe`) is built into the binaries from `data/fes1.bin`.
- Console connected over USB, powered off. Any console state is fine: memboot does not depend on
  hakchi being installed.

Set `HMOD` to the package path for the commands below.

### 1. Dry run (no console needed)

```
cargo run -p cartload-fel -- memboot --hmod "$HMOD" --dry-run
```

Expected: six summary lines and `dry run: images are valid, not touching USB`. For v1.0.4-126:
`version:   v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)`, `fes1.bin:  14048 bytes, built in`,
`uboot.bin: 761856 bytes, Sd variant, bootcmd value at offset 0x6a54b`,
`boot.img: 4585472 bytes on disk, 4585472 declared (kernel 1968736 + ramdisk 2612272, page 2048),
4587520 uploaded`, and a `cmdline:` line ending in `hakchi-shell`.

### 2. Probe

```
cargo run -p cartload-fel -- probe --wait 60
```

While it waits: hold Reset, switch Power on, keep Reset held for two seconds, release. The
console shows nothing on screen (the LED stays off); `lsusb` lists `1f3a:efe8 Allwinner`.

Expected output within a second of the device appearing:

```
FEL device 1f3a:efe8: bus 001 device NNN, high speed, manufacturer "", product "", serial ""
  bulk endpoints: IN 0x82, OUT 0x01
  AWUSBFEX soc=0x00166700 (A33 / R16) firmware=1 protocol=1 data_flag=0x44 data_length=0x08 scratchpad=0x00007e00
  SoC is the Allwinner R16 used by the Classic Mini consoles: OK
```

Record the actual bus/device, speed and endpoint numbers if they differ; the code only warns on
unexpected endpoints. Exit code 0.

### 3. Memboot

Leave the console in FEL mode after step 2 (probe does not change its state) and run:

```
cargo run -p cartload-fel -- memboot --hmod "$HMOD"
```

Expected log sequence (timestamps omitted):

```
INFO read boot files from hmod boot_img=4585472 uboot=761856 version=v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)
hmod:      ...
version:   v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)
...
FEL device 1f3a:efe8: ...
INFO preparing images
INFO verifying device
INFO AWUSBFEX soc=0x00166700 (A33 / R16) ...
INFO initialising DRAM
INFO uploading fes1 to 0x2000 and running it
INFO exec address=0x00002000
INFO uploading boot image
INFO uploading boot image to 0x47400000 len=4587520
INFO uploaded 458752 / 4587520 bytes (10%)
... up to 100%
INFO starting u-boot
INFO uploading u-boot to 0x47000000 len=761856
INFO running u-boot command command="boota 47400000"
INFO exec address=0x47000000
INFO kernel started; the console should come up as the hakchi network gadget shortly
memboot done: 4587520 bytes uploaded, ran `boota 47400000`
the console should enumerate as 04e8:6863 within about 10 s and answer SSH at 169.254.13.37 (root, empty password)
```

The whole run takes a few seconds (the DRAM init step waits two seconds on purpose). Then:

1. Within about 10 s `lsusb` shows `04e8:6863` with manufacturer `hakchi` and `1f3a:efe8` is
   gone. `ip -br addr show hakchi0` shows a `169.254.x.y/16` address.
2. `ssh root@169.254.13.37 'uname -r; cat /var/version'` (empty password) prints
   `3.4.113.29-madmonkey` and the version file. First SSH may take a few extra seconds while
   the ramdisk finishes starting.
3. The console screen stays black during a plain memboot of the stock hmod (no UI is started
   from RAM); that is expected.

Record: console model and region, host kernel, run time, and whether SSH answered.

### 4. Second memboot without power cycle (optional)

If the console drops back to FEL (for example after a `hakchi` script that reboots to FEL), rerun
step 3. Expected difference: `fes1 already resident in SRAM; DRAM is initialised` instead of the
fes1 upload. Everything else is identical.

### 5. Negative checks

- Nothing connected: `cargo run -p cartload-fel -- probe` exits 1 with
  `no FEL device found (USB 1f3a:efe8). Hold Reset while powering the console on, ...`.
- Console in normal boot with stock firmware (same USB id): `probe` exits 1 with either the
  `did not answer within 3s` message or the `no bulk IN/OUT endpoint pair` message, both of which
  say to hold Reset while powering on. Record which one the stock firmware produces.
- udev rule missing: `probe` exits 1 with `permission denied opening the FEL device ...`.

### Results

| Date | Console | Host | Probe | Memboot | SSH | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| | | | | | | not yet run on hardware |

## Console connect

Crate: `crates/console`, binary `cartload-console`. `discover` only opens TCP connections (and,
with `--mdns`, listens for mDNS). `info` runs the hakchi2 CE connect sequence: `hakchi` script
calls, `cat`, `df`, `du` and `ls`. Every `hakchi` invocation ends with the script's own
`save_config`, which rewrites `p0000_config` from the current values, exactly as every hakchi2 CE
connect does. `exec` and `ls` run what you give them.

### Prerequisites

- Console running the hakchi kernel, installed or membooted (see "FEL memboot"): `lsusb` lists
  `04e8:6863` with manufacturer `hakchi`, and `ip -br addr show hakchi0` shows a `169.254.x.y/16`
  address with state `UNKNOWN` (the gadget has no carrier detection; `unknown` is normal).
- `ping -c1 169.254.13.37` answers. If it does not, check firewalld / nftables on `hakchi0`.
- The udev rule and NetworkManager profile from the "FEL memboot" prerequisites.

### 1. Discover

```
cargo run -p cartload-console -- discover
```

Expected: log lines on stderr, the result on stdout, exit 0.

```
INFO link-local interface hakchi0 (169.254.68.74) operstate unknown
INFO console at 169.254.13.37:22 via USB link on hakchi0 (host 169.254.68.74), SSH-2.0-dropbear_2018.76
console at 169.254.13.37:22 via USB link on hakchi0 (host 169.254.68.74), SSH-2.0-dropbear_2018.76
```

The host address differs per boot (NetworkManager picks it). With `--all --mdns` the mDNS
announcement (instance `hakchi` on `madmonkey.local.`) is merged into the same line:
`..., SNES Classic Mini (EUR), hwid 0461872a872802079d2818c76c118000`. `--json` prints the
structure instead. Record the banner (dropbear version) and whether mDNS answered.

### 2. Info

```
cargo run -p cartload-console -- info
```

Expected for a European SNES Classic Mini with hakchi v1.0.4-126 and one game synced (the
storage block is byte for byte the one hakchi2 CE logs, see
`docs/reference/hakchi-ce-sync-trace.log`):

```
INFO console at 169.254.13.37:22 via USB link on hakchi0 (host 169.254.68.74), SSH-2.0-dropbear_2018.76
INFO SSH session established address=169.254.13.37:22 user="root"
INFO host key ecdsa-sha2-nistp256 SHA256:llATnWr35rHmZPcvq9oT9u3IZfo/CV5lO61RMHp+ws4
hwid:            0461872a872802079d2818c76c118000
versions:        boot 1.0.3, kernel 3.4.113.29-madmonkey, script v1.0.4-126
console:         SNES Classic Mini (EUR) [snes-eur]
firmware:        stock NAND firmware
md partitioning: no
games storage:   /var/lib/hakchi/games
paths:           gamepath /usr/share/games, rootfs /var/lib/hakchi/rootfs, squashfs /var/squashfs
Storage size: 352.6MB, used: 31.2MB, free: 310.5MB
    Used by all games: 3.0MB
    Used by non multi-boot games: 0.0MB
    Used by current games collection: 3.0MB
    Used by save-states: 0.1MB
    Used by other files (mods, configs, etc.): 28.1MB
    Reserved memory: 0.0MB
    Available for games: 313.5MB
config:          boot_logo=, boot_stock=n, clovercon_autofire=0, ... usb_host=y, usb_rw=n, wood=n
original games:  21 (CLV-P-SAAAE, CLV-P-SAABE, ..., CLV-P-SADKE)
```

The host key fingerprint changes at every boot (dropbear generates it in RAM). With `-v` the
negotiated algorithms are logged: `curve25519-sha256`, host key `ecdsa-sha2-nistp256`,
`aes256-ctr`, `hmac-sha2-256`. `--json` prints the `ConsoleInfo` structure. Record console type,
versions, the storage line and the number of original games (21 on the SNES Classic and Super
Famicom Mini, 30 on the NES Classic and Famicom Mini, 20 on the Shonen Jump edition; none are
listed on the Mega Drive Mini).

### 3. Exec and ls

```
cargo run -p cartload-console -- exec -- uname -r
cargo run -p cartload-console -- exec -- hakchi nope
cargo run -p cartload-console -- ls /var/lib/hakchi/games
cargo run -p cartload-console -- ls --ftp /var/lib/hakchi/games
```

Expected: `3.4.113.29-madmonkey` and exit 0; `no such function: nope` and exit 1 (the remote
exit code is passed through); an `ls -la` listing with the `snes-eur` directory; the same
directory as busybox `ftpd` LIST lines after `INFO FTP welcome: ...`. All four are read-only.

### 4. Negative checks

- Console unplugged or powered off: `discover` exits 1 with
  `no network interface has a 169.254.x.x address ...` once `hakchi0` is gone, or
  `no console answered at 169.254.13.37:22 (probed from hakchi0 (...))` while the interface
  still exists. Both run in about a second.
- Console in FEL mode or stock firmware: same as above (no network gadget).
- `info --host 192.0.2.1 --timeout-ms 1000`: exits 1 with
  `connecting to 192.0.2.1:22 did not complete within 1s`.
- An SSH server that is not a hakchi kernel (for example `--host` pointing at a Linux box):
  `SSH authentication as "root" with an empty password was refused ...`.

### Results

| Date | Console | Host | discover | info | exec / ls | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-21 | SNES Classic Mini (EUR), hakchi v1.0.4-126, kernel 3.4.113.29-madmonkey | Linux 6.18.57, hakchi0 169.254.68.74 | OK, USB link and mDNS, dropbear_2018.76 | OK, output above | not run (brief allowed discover and info only) | first attempt failed auth until the `none` method was tried before `password` |

## Install kernel

Crate: `crates/flow`, binary `cartload-flow`. Runs hakchi2 CE's "Install / Repair" sequence
end to end: memboot, grow the data partition, upload `hakchi.hmod`, run the device-side
installer (`boot`), wait for the reboot, verify. This **writes to NAND**: u-boot, the kernel
partition (after the installer backs the stock kernel up to its backup sectors) and the data
partition. The flow must be attended: it waits for the console to be put in FEL mode and prints
what to do. A failed or interrupted run is resumed by rerunning the same command; finished steps
are reported as "already done".

### Prerequisites

- Everything from "FEL memboot" (udev rule, NetworkManager link-local profile for `hakchi0`)
  and "Console connect" (firewall).
- `HMOD` set to `hakchi.hmod` (v1.0.4-126, md5 `2e163a25c98b4c05bed274817abb38da`).
- Console connected over USB. Powered off for a first install (stock firmware); any hakchi
  state for a repair.
- Nothing else answering at `169.254.13.37` (one console at a time).

### 1. Dry run (no console needed)

```
cargo run -p cartload-flow -- --dry-run install --hmod "$HMOD"
```

Expected: `package:` (path, 9340382 bytes, the md5 above), `version:   v1.0.4-126 (boot 1.0.3,
kernel 3.4.113.29-madmonkey)`, `flow:     install (13 steps)`, the 13 steps `memboot`,
`wait-console`, `detect`, `check-md`, `grow`, `reboot-grown`, `wait-console`, `detect`,
`prepare`, `upload`, `install`, `wait-console`, `verify`, each with its description, then
`dry run: inputs are valid, not touching USB`. Exit 0. `--dry-run uninstall` and
`--dry-run dump-kernel kernel.img` list 8 and 6 steps and need no package.

### 2. Install from stock firmware

```
cargo run -p cartload-flow -- install --hmod "$HMOD"
```

Every step prints `[ n/13] name: description`, its log lines indented, then `=> done (time)`,
`=> already done: reason` or `=> not needed: reason`. Expected per step on a NES or SNES
Classic Mini (`-v` adds the debug lines, among them the `sunxi-part` output):

1. `memboot`: `waiting up to 120 s for a console in FEL mode: hold Reset, switch Power on,
   release Reset after two seconds`. Do that now. Within a second of the device appearing:
   `FEL device 1f3a:efe8 on bus ...`, `membooting hakchi v1.0.4-126 (...) through FEL`,
   `preparing the boot images`, `verifying the FEL device`, `initialising DRAM with fes1.bin`,
   `uploading the boot image`, `boot image: ... / 4587520 bytes (...%)` up to 100 %,
   `starting u-boot (boota)`, `uploaded 4587520 bytes and ran boota 47400000; ...`. The
   console screen stays black. A few seconds.
2. `wait-console`: `waiting up to 120 s for the console on the USB network`, then within
   about 10 to 15 s `the console answers SSH at 169.254.13.37:22`. On the host `lsusb` shows
   `04e8:6863` and `1f3a:efe8` is gone.
3. `detect`: `console: minimal memboot shell (hwid 0461872a872802079d2818c76c118000)`.
4. `check-md`: `=> not needed: not a Mega Drive partition layout`.
5. `grow`: `growing the data partition (nandX) from A to B` (or `growing the data partition
   (sunxi-part grow)` when the dry-run output is not parseable), then `data partition nandX is
   now B`. Record the sizes and, with `-v`, the raw `sunxi-part` lines: the parser follows
   hakchi2 CE's `(mbr|nand[a-z]):label:sectors` and has not seen real output yet.
   If `UDISK is still listed` appears, the grow did not take and the step ends `not needed`.
6. `reboot-grown`: `uploading the boot image for kexec (4585472 bytes)`, progress, `kexec: the
   console drops off the network now and comes back in the memboot shell`. On the host
   `hakchi0` disappears and comes back.
7. `wait-console`: `the console left the USB network`, then `the console answers SSH at ...`
   within about 20 s.
8. `detect`: the memboot shell again, same hwid.
9. `prepare`: `=> done` with no log lines (three commands: the two `echo >> /hakchi/config`
   and `mkdir -p /hakchi/transfer/`).
10. `upload`: `uploading .../hakchi.hmod (9340382 bytes, hakchi v1.0.4-126 (...))` and
    `hakchi.hmod: ... / 9340382 bytes (...%)` up to 100 %. A few seconds over USB.
11. `install`: `running the installer on the console; it takes a few minutes and the
    connection drops when the console reboots`, then `console: ...` lines echoing the
    installer (`installing hakchi...`, `package hakchi installed`, ...). The console shows
    hakchi's default image during the install and then reboots into the normal menu. Any
    `flash md5 mismatch!`, `invalid image` or `flash boot0 failed!` line stops the flow.
12. `wait-console`: offline, then back within 180 s.
13. `verify`: `console: hakchi v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey) installed
    on SNES Classic Mini (EUR) (hwid ...)`, `installed: ...`, `games storage
    /var/lib/hakchi/games: ...`. Then `install finished: 13 steps in N min`.

Afterwards `cargo run -p cartload-console -- info` prints the "Console connect" output and
`cargo run -p cartload-flow -- dump-kernel kernel.img` can read the stock kernel the installer
backed up. Record: console model and region, total time, the step 5 sizes, the installer lines
from step 11.

### 3. Resume

Interrupt the run with Ctrl-C during step 10 (`upload`), then rerun the same command. Expected
first lines: `[ 1/13] memboot` `=> already done: the console is already in the memboot shell`,
`[ 2/13] wait-console` `=> already done: the console is already reachable`, `detect` done,
`check-md` not needed, `grow` `=> already done: no UDISK partition left; ...`, `reboot-grown`
`=> already done: the partition table did not change in this run`, `wait-console` already,
`detect` done, `prepare` `=> already done: cf_install and cf_update are set and
/hakchi/transfer exists`, then `upload` runs (or is already done when the interrupted upload
had completed and the md5 matches) and the install continues as in section 2.

### 4. Repair from a running hakchi kernel

Same command with the console booted normally. Expected difference in step 1: `found hakchi
v1.0.4-126 (...) installed on ...`, `membooting the package's boot.img with kexec from the
running kernel`, `uploading the boot image for kexec (4585472 bytes)`, progress, `kexec: the
console drops off the network now ...`; no Reset button, no FEL. Step 2 reports `the console
left the USB network` first. Step 5 is `already done` (no UDISK), step 6 too. Steps 9 to 13 as
in section 2; the installer prints `updating hakchi...` lines instead of `installing`.

### 5. Negative checks

Only with **no console attached** (an attached hakchi console would be membooted):

- `cargo run -p cartload-flow -- --wait-fel 5 install --hmod "$HMOD"`: step 1 fails after
  5 s with `a console in FEL mode (USB 1f3a:efe8) appearing did not happen within 5 s; power
  the console off, hold Reset, ...`, then `rerun the same command to resume`, exit 1.
- `install --hmod Cargo.toml`: `reading Cargo.toml: hakchi.hmod: Cargo.toml is not a readable
  gzip tar archive: invalid gzip header`, exit 1, nothing else runs.

### Memboot only

```
cargo run -p cartload-flow -- memboot --hmod "$HMOD"
```

Steps 1 to 3 of the install (`memboot`, `wait-console`, `detect`), console in FEL mode as in
section 2. Ends with `console: minimal memboot shell (hwid ...)` and `memboot finished: 3 steps
in ...`; `cargo run -p cartload-console -- exec -- cat /hakchi/config` then shows
`export cf_memboot='y'`. Nothing is written to NAND; power-cycle the console to leave the shell.
From a running hakchi kernel the first step kexecs instead (section 4).

### Dump stock kernel

```
cargo run -p cartload-flow -- dump-kernel kernel.img
```

On an installed console booted normally (no `--hmod` needed): step 1 `membooting the kernel from
NAND with kexec from the running kernel`, steps 2 and 3 as above, step 4 `wrote kernel.img:
N bytes, kernel-dp-sneseur-release-v2.0.14-0-gd8b65c6.img (SNES Classic Mini (EUR))` (the image
name follows the console's stock firmware version; `not a known stock kernel image (md5 ...)`
means a dump that hakchi2 CE would not accept either: record the md5), step 5 `rebooting the
console`, step 6 `the console left the USB network`. The console then boots hakchi normally.
Rerunning with the same file: step 4 `=> already done: kernel.img already holds ...`.

### Uninstall

```
cargo run -p cartload-flow -- uninstall
```

On an installed console booted normally: steps 1 to 3 as for the dump, 4 `stock kernel from
the console's backup: N bytes, kernel-dp-...`, 5 `uploading the stock kernel (...)`, progress,
`flashing the stock kernel (hakchi flashBoot2, verified by MD5)`, 6 `console: uninstalling
hakchi...`, `console: done.`, 7 `rebooting the console`, 8 `the console left the USB network`.
The console boots the stock firmware: its menu appears and `lsusb` shows `1f3a:efe8` (normal
boot) and no `04e8:6863`. `install` from section 2 puts hakchi back. A console with no backup
(`no stock kernel backup on the console: ...`) needs `--stock-kernel kernel.img` from an earlier
dump; a file whose md5 is unknown is refused unless `--force-stock-kernel` is given.

### Results

| Date | Console | Host | Install | Resume | Repair | Memboot | Dump | Uninstall | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| | | | | | | | | | not yet run on hardware |

## Sync one game

Crate: `crates/library`, binary `cartload-library`. `import`, `list` and `menu` never touch the
console. `plan` connects, runs the hakchi2 CE connect sequence, reads the firmware's launch
entries (`cat` of each `CLV-P-*.desktop` in the squashfs, cached under `originals/` in the
library) and lists the games storage (`find`/`stat`); it changes nothing except that
`mkdir -p` of the per-console games directory, which already exists on a synced console. `sync`
**modifies the games storage**: it stops the menu, unmounts the games overlay, removes the files,
symlinks and emptied directories named in the plan, uploads the plan's files over FTP, sets their
times, runs the link script, writes the config and runs `hakchi overmount_games; uistart`. Save
states and everything outside `<storage>/<system code>/` are untouched. A console synced with
hakchi2 CE keeps working after a Cartload sync and vice versa (same layout); the first Cartload
sync after a CE sync re-uploads every file because the timestamps differ.

### Prerequisites

- Everything from "Console connect".
- A SNES ROM (`.sfc`/`.smc`) or NES ROM (`.nes`). The run below uses Chrono Trigger (USA),
  CRC-32 `2D206BF7` after copier header removal, which hakchi2 CE's trace also used.
- A scratch library so the default one is not touched: `export CARTLOAD_LIBRARY=/tmp/cartload-lib`.

### 1. Import and inspect (no console needed)

```
cargo run -p cartload-library -- import "Chrono Trigger.smc"
cargo run -p cartload-library -- list
cargo run -p cartload-library -- menu --console snes-eur
```

Expected: the first download of `romfiles.xml.gz` (518 698 bytes, 19 932 records) into the
cache, then `CLV-U-ZJSQU  Chrono Trigger  [Super Nintendo Entertainment System] matched`. The
game directory holds `Chrono_Trigger.sfrom` (4 194 400 bytes = 48 + 4 194 304 + 48),
`CLV-U-ZJSQU.desktop`, `CLV-U-ZJSQU.png`, `CLV-U-ZJSQU_small.png` and `game.json`. `menu` prints
the game alone (no firmware entries are cached yet) and `1 menu directories on the console`.

### 2. Plan against the console

```
cargo run -p cartload-library -- plan
```

Expected on a European SNES Classic Mini: `console: SNES Classic Mini (EUR) [snes-eur],
storage /var/lib/hakchi/games`, `cached the firmware's launch entries count=21`, then
`1 games, 21 firmware games, 2 folders in 31 files (4321280 bytes on the console)` and the
free space line. The upload list has the 21 `./000/CLV-P-*/CLV-P-*.desktop` entries with the
same sizes hakchi2 CE uploads (CLV-P-SAAAE 600 bytes, CLV-P-SABTE 796 bytes, ...), the
`More games...` folder `CLV-S-00001` in `000`, the game's files under `./.storage/CLV-U-ZJSQU/`
and its entry plus the `Original games` back folder `CLV-S-00000` in `001`. On a console
hakchi2 CE synced before, the plan also removes CE's `_mdmini.png`, `metadata.json` and `.7z`
files and the 21 `autoplay` symlinks (recreated by the link script). Verified 2026-10-21 on the
console of the trace, read-only.

### 3. Sync

```
cargo run -p cartload-library -- sync
```

Confirm with `y`. Expected: the nine steps `stop-ui`, `umount`, `diff`, `cleanup`, `upload`
(progress lines, about 4.2 MB in a second or two over USB), `link`, `config`, `finish`, `done`,
then `uploaded 31 files (4211699 bytes), removed ... games mounted: true`. The TV shows the
stock menu with the 21 original games and a "More games..." folder on the right; inside it
Chrono Trigger (placeholder cover) and an "Original games" folder leading back. Starting the
game must work and offer 3 save slots. `cargo run -p cartload-console -- info` then reports
about 4.2 MB used by the current games collection.

### 4. Second sync and removal

- `sync` again: the plan says `upload 0 files`, `keep 31`, nothing to remove; the sync still
  relinks and remounts. Expected to finish in a few seconds with the menu unchanged.
- `remove CLV-U-ZJSQU`, then `sync`: the plan removes the game's three files and its entry and
  the two folders (`More games...` no longer has content), and the menu shows only the 21
  original games.
- Negative: with the console off, `plan --console snes-eur` prints `no console answered;
  planning against an empty snes-eur console` and lists every file as an upload; `sync` exits 1
  with `no console answered`.

### Results

| Date | Console | Import | Plan | Sync | Second sync | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-21 | SNES Classic Mini (EUR), hakchi v1.0.4-126 | OK (Chrono Trigger, code and .sfrom size as in the CE trace) | OK, read-only, 21 firmware entries cached | not run (user absent) | not run | the entries the plan generates have the sizes of CE's uploads |
