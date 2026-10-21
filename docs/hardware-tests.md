# Hardware tests

Manual checklists for steps that need a real console. Automated tests never touch hardware;
anything that does lives here instead.

Checklists land per crate as each one gains hardware-facing functionality:

- `crates/fel`: FEL memboot (console in FEL mode, `1f3a:efe8`).
- `crates/console`: discovery, SSH and FTP against a booted hakchi kernel (`04e8:6863`).
- `crates/hmod`: kernel package install and version check on the console.
- `crates/library`: sync of a small library (compressed games included) and verification on the
  console menu.
- `crates/flow`: the kernel install, memboot, dump, uninstall and base mods flows end to end.

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
6. `reboot-grown`: `uploading hakchi2 CE's kexec.static (722876 bytes) and detached-fallback
   (1873 bytes)`, `kexec.static: ... / 722876 bytes (100%)`, `uploading the boot image for kexec
   (4585472 bytes)`, progress, `kexec: the console drops off the network now and comes back in
   the memboot shell`. With `-v` a `uistop exited with ... (ignored, as hakchi2 CE does)` line
   may precede the uploads: the memboot shell has no UI to stop. The step does what hakchi2
   CE's `GrowTask` does with `rebootToRecovery` (`MembootTasks.Memboot`): `uistop`, `mkdir -p
   /tmp/cartload`, upload `kexec.static` as `/tmp/cartload/kexec` and `detached-fallback`,
   `chmod +x` both, upload the package's `boot.img`, then run `cd /tmp/cartload; /bin/sh
   /tmp/cartload/detached-fallback recovery /tmp/cartload/boot.img` through a launcher that is
   detached from the SSH session. On the host `hakchi0` disappears and comes back. **Untested
   on hardware**: the test console's data partition was already grown, so this step was
   skipped (section "Results"). If the console hangs here (power LED off, no USB device),
   power-cycle it and rerun the command: step 1 then goes through FEL and the grow is already
   done.
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
    `flash md5 mismatch!` or `flash boot0 failed!` line stops the flow. `invalid image on nand at
    sector: 68` on stderr is normal: the installer checking for an existing stock kernel backup.
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
running kernel`, `uploading hakchi2 CE's kexec.static (722876 bytes) and detached-fallback
(1873 bytes)`, `kexec.static: ... (100%)`, `uploading the boot image for kexec (4585472
bytes)`, progress, `kexec: the console drops off the network now ...`; no Reset button, no
FEL. The console's menu goes away at `uistop` before the uploads. The sequence is the one of
section 2 step 6 (hakchi2 CE's `Memboot`: its own `kexec.static` and `detached-fallback`
under `/tmp/cartload`, not the console's `hakchi recovery`). **Untested on hardware** in this
form: the first run (2026-10-21) used the console's own `hakchi recovery` and hung the console
(power LED off, no USB device, a power cycle recovered it); the switch to CE's tools has not
been run since. If it hangs again, power-cycle the console and record the `-v` output. Step 2
reports `the console left the USB network` first. Step 5 is `already done` (no UDISK), step 6
too. Steps 9 to 13 as in section 2; the installer prints `updating hakchi...` lines instead of
`installing`.

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

### Base mods

```
cargo run -p cartload-hmod -- base-mods
cargo run -p cartload-flow -- --dry-run base-mods
cargo run -p cartload-flow -- base-mods
```

Installs hakchi2 CE's `tiny7zx` mod (the 7z extractor compressed games need) on a console
booted into its hakchi kernel; `--mods tiny7zx,font_remount` adds CE's extended title font. The
files are downloaded from the CE repository at the pinned commit into the cache
(`~/.cache/cartload/basemods/<commit>/<name>.hmod/`) the first time, checked against the MD5s
in the catalog, and reused afterwards (`--offline` refuses to download). This **writes to the
hakchi rootfs overlay** (`/var/lib/hakchi/rootfs/bin/tiny7zx`, `/var/lib/hakchi/hmod/
uninstall-tiny7zx`) and reboots the console; the kernel partitions are untouched.

Expected: `cartload-hmod base-mods` lists both mods with sizes and MD5s and `3 of 3 files
missing` until fetched (`--fetch` downloads 30 116 bytes for tiny7zx in under a second; verified
2026-10-21 against the live repository, every file's MD5 matching the local CE checkout). The
dry run prints `mods:     tiny7zx from ... (https://github.com/TeamShinkansen/Hakchi2-CE at
4fb3092c29cd)`, `tiny7zx: cached`, `flow:     base-mods (6 steps)` and the steps
`wait-console`, `detect`, `base-mods`, `wait-console`, `detect`, `verify-mods`. The real run,
on a console with CE's base mods already installed: step 3 `=> already done: tiny7zx: 1 file(s)
in /var/lib/hakchi/rootfs with the expected MD5, registered in /var/lib/hakchi/hmod` and no
reboot; step 6 `tiny7zx is on the console's PATH at /bin/tiny7zx`. On a console without the mod:
step 3 `uploaded tiny7zx.hmod (3 files, 30116 bytes)`, `console: installing tiny7zx...`,
`console: creating uninstall for tiny7zx...`, `console: package tiny7zx installed`,
`installed tiny7zx; rebooting the console so the new files are seen`, then the console drops off
the network and comes back (step 4), is identified (step 5) and step 6 reports the path. After
that, `cargo run -p cartload-console -- exec -- hakchi pack_list` lists `tiny7zx`. Negative:
`--mods clovercon` is refused with the list of known mods; `--offline` with an empty cache fails
with `is not cached; download it first`; in the memboot shell step 2 fails with `expected a
console booted into its installed hakchi kernel`.

### Results

| Date | Console | Host | Install | Resume | Repair | Memboot | Dump | Uninstall | Base mods | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-10-21 | SNES Classic Mini EUR, hakchi v1.0.4-126 already installed | EndeavourOS, Linux 6.18 | ok (FEL) | | | kexec from running kernel through the console's own `hakchi recovery`: console hung, LED off, power-cycle recovered it. Replaced since (task 13) by hakchi2 CE's sequence with its own `kexec.static` and `detached-fallback`; that sequence was verified on 2026-10-21 evening with `memboot` from the running kernel: console back in the memboot shell in 8.7 s, flow 13.7 s total (c4e9609). FEL memboot also OK a second time. kexec from the memboot shell after a grow still untested | | | | first run panicked opening the FEL device (nusb needed its tokio feature, fixed 16fffd4); second run through FEL ran all 13 steps, installer printed "invalid image on nand at sector: 68" which the flow wrongly took as fatal (fixed 77e83e5); the console came back installed and `cartload-console info` verified it. grow/reboot-grown were skipped (partition already grown), so kexec from the memboot shell is still untested. haveBackup2 and isOriginalBoot2 both fail on this console: no stock kernel backup at sector 68 |

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
sync after a CE sync re-uploads every file because the timestamps differ. SNES and NES games are
compressed by default (the ROM goes up as `<rom>.7z`, like CE's "Compress games"), so the
console must have CE's `tiny7zx` mod ("Base mods" above); `sync` checks `which tiny7zx` before
touching anything and `cartload-library compress --off <CODE>` turns compression off per game.

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
cache, then `CLV-U-ZJSQU  Chrono Trigger  [Super Nintendo Entertainment System] matched,
4194400 bytes packed to 2850539 (67%)` (about a second for the packing). The game directory
holds `Chrono_Trigger.sfrom` (4 194 400 bytes = 48 + 4 194 304 + 48), `Chrono_Trigger.sfrom.7z`
(2 850 539 bytes; hakchi2 CE's own archive of the same file is 2 850 560), `CLV-U-ZJSQU.desktop`
(its `Exec` names the `.7z`), `CLV-U-ZJSQU.png`, `CLV-U-ZJSQU_small.png` and `game.json`
(`"compress": true`). `list` shows `Chrono_Trigger.sfrom.7z`. `menu` prints the game alone (no
firmware entries are cached yet) and `1 menu directories on the console`.

```
cargo run -p cartload-library -- compress --off CLV-U-ZJSQU
cargo run -p cartload-library -- compress CLV-U-ZJSQU
```

Expected: `uncompressed: 4194400 bytes` with the `.7z` removed and the entry naming the `.sfrom`,
then `compressed: 4194400 bytes packed to 2850539 (67%)` with the archive rebuilt (identical
bytes: the packing is deterministic). `compress --all` reports `(unchanged)`.

### 2. Plan against the console

```
cargo run -p cartload-library -- plan
```

Expected on a European SNES Classic Mini: `console: SNES Classic Mini (EUR) [snes-eur],
storage /var/lib/hakchi/games`, `cached the firmware's launch entries count=21`, then
`1 games (1 compressed), 21 firmware games, 2 folders in 31 files (2973696 bytes on the
console)`, `upload 31 files (2867841 bytes)` and the free space line. The upload list has the
21 `./000/CLV-P-*/CLV-P-*.desktop` entries with the same sizes hakchi2 CE uploads (CLV-P-SAAAE
600 bytes, CLV-P-SABTE 796 bytes, ...), the `More games...` folder `CLV-S-00001` in `000`, the
game's files under `./.storage/CLV-U-ZJSQU/` (`Chrono_Trigger.sfrom.7z`, 2850539 bytes) and its
entry (578 bytes, the size of CE's own entry for the compressed game plus or minus the
`country` line) plus the `Original games` back folder `CLV-S-00000` in `001`. On a console
hakchi2 CE synced before, the plan also removes CE's `_mdmini.png` and `metadata.json` files and
the 21 `autoplay` symlinks (recreated by the link script); CE's `.7z` at the same path is
replaced by Cartload's, which differs in size and time. With compression off the game's line is
`Chrono_Trigger.sfrom` (4194400 bytes) and the total `4321280 bytes on the console`. Verified
2026-10-21 on the console of the trace, read-only, in both forms.

### 3. Sync

```
cargo run -p cartload-library -- sync
```

Confirm with `y`. Expected: first `1 compressed games; the console extracts them with
/bin/tiny7zx` (the `which tiny7zx` check; a console without the mod stops here with
`1 compressed game(s) need the tiny7zx mod on the console` and nothing changed), then the nine
steps `stop-ui`, `umount`, `diff`, `cleanup`, `upload` (progress lines, about 2.9 MB in a
second over USB), `link`, `config`, `finish`, `done`, then `uploaded 31 files (2867841 bytes),
removed ... games mounted: true`. The TV shows the stock menu with the 21 original games and a
"More games..." folder on the right; inside it Chrono Trigger (placeholder cover) and an
"Original games" folder leading back. Starting the game must work (the wrapper extracts the
archive to `/tmp/rom` first; the console takes a moment longer than with a raw ROM) and offer 3
save slots. `cargo run -p cartload-console -- info` then reports about 2.9 MB used by the
current games collection. `compress --off CLV-U-ZJSQU` followed by `sync` must replace the
archive with the 4.2 MB `.sfrom` and the game must still start.

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
| 2026-10-21 | same console | OK, compressed: 2 850 539-byte .7z in 0.9 s; compress --off/on round-trips | OK, read-only: 31 uploads, 2 973 696 bytes total, CE's .7z at the same path replaced | not run (user absent) | not run | `which tiny7zx` on the console answers /bin/tiny7zx (read-only check) |

## Console page (desktop app)

Crate: `src-tauri`, binary `cartload-app`; headless equivalent `cargo run -p cartload-app
--example watch`. The app's watcher looks at the USB bus and the USB network link every 2 s,
opens one SSH session when a hakchi kernel answers and runs the hakchi2 CE identification
sequence once (`hakchi` script calls, `cat`, `df`, `du`, `ls`; see "Console connect" above for
the `save_config` side effect). A device with the FEL ids is opened once for the FEL version
query (read-only) when it appears. Nothing else is sent unless the build has the `dev-exec`
feature and you type a command yourself.

### Prerequisites

- The udev rule and NetworkManager profile from "FEL memboot".
- `pnpm install`, then `pnpm build` and `cargo build -p cartload-app`, or `pnpm tauri dev`.
- On Wayland hosts where the window dies with `Gdk-Message: Error 71 (Protocol error)
  dispatching to Wayland display`, run with `WEBKIT_DISABLE_DMABUF_RENDERER=1` (a WebKitGTK
  issue with some GPU drivers; seen on the 2026-10-21 host).

### 1. Nothing attached

Start the app with no console attached (or with it switched off). Expected: within 2 s the
status card reads "Nothing found. Connect the console with USB and turn it on." with the
hint about holding Reset, the status bar shows the same line with a grey dot, and the log
panel (Ctrl+L) shows "console watcher started". No toast.

### 2. Installed hakchi kernel

Switch the console on. Expected, in order and without any click: "hakchi kernel found on USB:
waiting for its network link…" (blue, cartridge pulsing) as soon as `04e8:6863` enumerates,
"Console found at 169.254.13.37:22: connecting…", "… identifying…", then the green card
"SNES Classic Mini, EUR, hakchi v1.0.4-126, 311 MB free" (model, region, versions and free
space as the console reports them), "Ready. Open the Library page to add and sync games.",
the primary button "Open Library", and the details list (model, region, hardware id, hakchi,
kernel, boot, firmware, games storage, storage sizes, games on console, save states, original
games, address, link). The log shows the SSH session line and, with "Show details" on, each
command of the identification sequence. "Refresh" (or F5) re-runs the sequence and keeps the
card green.

Unplug the USB cable: the card goes back to "Nothing found" within 2 s and the log says
"console no longer reachable; closing the session". Plug it back in: the sequence above runs
again by itself.

### 3. FEL mode

With the console off, hold Reset and switch it on. Expected: "Console attached over USB:
checking for FEL mode…" for a moment, then the green "Console in FEL mode: ready to install
the kernel." with the chip "Allwinner R16" in the details and the primary button "Install
kernel". Power-cycle the console into normal boot: "Console left FEL mode: waiting for the
hakchi kernel to come up…" and then, if hakchi is installed, the connected card as in step 2;
otherwise "Nothing found" after 45 s.

### 4. Stock firmware in normal boot

A console without hakchi, switched on normally, enumerates with the FEL ids too. Expected:
"Console attached over USB, but not in FEL mode." (amber) with the instruction to hold Reset,
and "Check FEL mode" as the primary button, which repeats the query and reports the result in
a toast.

### Results

| Date | Console | Host | 1 | 2 | 3 | 4 | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-10-21 | SNES Classic Mini (EUR), hakchi v1.0.4-126 | Linux 6.18.57, Wayland, hakchi0 169.254.68.74 | not run (console attached all day) | OK: connected card and details in 2.4 s, both in the app and the `watch` example | not run | not run | native Wayland needed `WEBKIT_DISABLE_DMABUF_RENDERER=1`; GDK_BACKEND=x11 also works |

## Kernel page (desktop app)

Crate: `src-tauri`, binary `cartload-app`. The Kernel page runs the flows of "Install kernel"
above through the app's `FlowRunner`: the same `cartload-flow` steps, the same time limits,
observed as a step list with timing, the log of the run beneath, and a failure card with the
likely cause and the next action. While a flow runs the console watcher is paused and the
status bar reads "Installing hakchi: step n of 13, …". Everything in "Install kernel" about
what is written to NAND applies. The headless equivalent of each section is the
`cartload-flow` command named there; the app adds nothing to the device-side sequence.

### Prerequisites

- Everything from "Console page" (udev rule, link-local profile, the Wayland variable).
- No `~/.cache/cartload/hakchi.hmod` for section 1 (move it aside); the package from
  "Install kernel" for the others, either downloaded in section 1 or pointed at with
  `CARTLOAD_HMOD=/path/to/hakchi.hmod` when the host is offline.
- Console connected over USB, powered off, stock firmware, for section 2; the states named in
  the other sections.

### 1. Package download (no console needed)

Open the Kernel page. Expected: stage 1 "Waiting for the console… Nothing found on USB yet.",
stage 2 "No kernel package yet. It is a 9 MB download from the hakchi project's servers." with
the accent "Download package" button, stage 3 "Install hakchi" disabled with "Waiting for the
console (stage 1)." and the 13 pending steps, stage 4 a dashed "Result" placeholder. Click
"Download package": the button turns into "x MB of 8.9 MB", the log (Ctrl+L) shows
`downloaded kernel package` with the GitHub URL, a toast "Kernel package v1.0.4-126 is ready."
appears and stage 2 shows "hakchi v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)",
"8.9 MB, downloaded <today>" and a quiet "Update package" button. The file is at
`~/.cache/cartload/hakchi.hmod` (md5 `2e163a25c98b4c05bed274817abb38da`). Pull the network
cable and click "Update package": an error toast "The package could not be downloaded: …"
with the detail under its disclosure; the cached copy stays.

### 2. Install from stock firmware

Hold Reset and switch the console on. Expected: stage 1 turns green "Console in FEL mode.
Ready." (the status bar agrees), "Install hakchi" becomes enabled. Click it. Expected, in
order: the header shows the running time, the status bar reads "Installing hakchi: step 1 of
13, boot from memory…" (blue dot), the Console page's card says the same with "Leave the
console and the USB cable alone until it finishes.", and the step list follows the sequence
of "Install kernel" section 2: "Boot from memory" (spinner, its last log line beneath, the
boot image's progress bar), "Wait for the console", "Identify the console", "Mega Drive
check" marked skipped with "not a Mega Drive partition layout", "Grow the data partition",
"Reboot into the grown partition", the wait and identify again, "Prepare the install",
"Upload the package" with the 8.9 MB progress bar, "Run the installer" (a few minutes; the
installer's lines appear when it ends), "Wait for the console", "Verify the install". Each
finished step shows its time on the right. The log beneath the list shows only this run's
lines. At the end stage 3 turns green with the total time and "Install again" (quiet), stage 4
reads "hakchi is installed." over the green console card "SNES Classic Mini, EUR, hakchi
v1.0.4-126, N MB free" and the details list, exactly as on the Console page, with "Open
Library" as the only accent button. The status bar is back to the connected sentence.

Click "Install again" with the console now running hakchi: stage 1 reads "hakchi v1.0.4-126 is
installed. Installing again updates or repairs it from the running kernel; no Reset needed.",
the first step logs the kexec instead of FEL ("uploading hakchi2 CE's kexec.static (722876
bytes) and detached-fallback (1873 bytes)" with a progress bar for `kexec.static`, then the
boot image's, then "kexec: the console drops off the network now and comes back in the
memboot shell"; the console's menu disappears at `uistop` before the uploads), "Grow the data
partition" is skipped ("no UDISK partition left; the data partition is already grown"), the
rest runs as above. The kexec sequence is "Install kernel" section 4's and is untested on
hardware; if the console hangs (power LED off), power-cycle it and the failure card for the
wait step appears, then "Try again" goes through FEL.

### 3. Stop

Start "Install hakchi" with the console switched off. "Boot from memory" waits (its line reads
"waiting up to 120 s for a console in FEL mode…"). Click "Stop": within a second stage 3 shows
"Boot from memory" pending again with "Stopped before it finished.", stage 4 "Stopped. Nothing
more was changed." with "Continue", the status bar goes back to "Nothing found". "Continue"
starts the same flow again. Clicking "Stop" during "Run the installer" instead shows "Stopping
after this step…" and the amber note; the flow stops once the installer has finished.

### 4. Failure

Start "Install hakchi" with the console switched off and wait 120 s. Expected: "Boot from
memory" turns red with the raw error beneath, stage 4 shows the red card "The console did not
show up in FEL mode within 120 s.", "Failed at: Boot from memory", "Likely cause: It is not in
FEL mode, or the USB link is not working.", "What to do: Power the console off, hold Reset,
switch Power on, keep Reset held for two seconds and release it; check the USB cable and, on
Linux, the udev rule for 1f3a:efe8.", the raw error under "Raw error", and "Try again". Put
the console in FEL mode and click "Try again": the steps reset and the install runs.

### 5. More menu: memboot, save the stock kernel, return to the original system

With an installed console running hakchi, open "More" (closes on Escape or a click elsewhere):

- "Boot from memory only": three steps; ends with stage 4 "The console runs hakchi from memory.
  Nothing was written to it." and the green card "Console running the hakchi recovery kernel
  from memory." Power-cycle
  the console afterwards.
- "Save the stock kernel…": the native save dialog ("Save the stock kernel", `kernel.img`).
  Cancelling it starts nothing. Saving runs the six steps of "Dump stock kernel"; stage 4 reads
  "The stock kernel was saved." with the path, and the file's md5 matches the one the log
  prints. Running it again onto the same file skips "Save the stock kernel" as already done.
- "Return to the original system…": nothing starts until the red confirmation "Return the
  console to its original system" is clicked ("Keep hakchi" or Escape cancels). Then the eight
  steps of "Uninstall"; stage 4 "The console is back on its original system." and the amber
  Console card "Console attached over USB, but not in FEL mode." once the stock firmware has
  booted. On a console without a backup the failure card reads "The console has no backup of
  its original kernel." with "Choose a kernel dump…", which opens the native open dialog and
  retries with the chosen file; a file with an unknown md5 gives "The chosen kernel file is
  not a known stock kernel." with "Flash it anyway".

### Results

| Date | Console | Host | 1 | 2 | 3 | 4 | 5 | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| | | | | | | | | not yet run on hardware (a console running hakchi was attached during the task; no flow was started) |

## Library page (desktop app)

Crate: `src-tauri` (the app). Needs a console running hakchi on USB and a few ROM files. The
headless check first: `cargo run -p cartload-app --example plan` lists the library, waits for
the console, caches its firmware launch entries and prints the sync plan without changing
anything on the console. Run on 2026-10-21 against the SNES Classic Mini (EUR) with one game
in the library: 21 firmware entries cached, plan of 31 uploads (4 236 052 bytes), 5 removals
and 21 stale links, fits; the same numbers task 06's CLI produced.

Then the app (`WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo tauri dev` on this host), Library page:

1. **Tree and bar.** With the console connected, the tree reads "Console menu" with the
   firmware games under it (21 on a SNES Classic) once the first plan has run, the sync bar
   shows the console's used/total bytes from the status, the library size, and "N files to
   upload" or "The console is up to date".
2. **Import.** Drop one `.sfc` and one folder of ROMs onto the window: the overlay appears
   while dragging, the import panel lists every file with its outcome (code, hash database
   match or "named after the file", warnings), the new games are selected in the grid, and the
   bar's pending count grows. Repeat with "Add games…" (native dialog).
3. **Edit.** Select a game, change the name and press Enter: "Save" disappears, the tile
   updates, the plan gains one upload (the launch entry). Try an invalid date: the toast says
   YYYY-MM-DD and nothing is saved.
4. **Art.** Store a TheGamesDB key on the Settings page, select a game, "Fetch art": the
   progress line runs through searching/fetching, the cover replaces the placeholder,
   publisher and date fill in. "Choose image…" with a PNG replaces the cover without network.
5. **Folders.** "New folder", name it, drag a game from the grid onto it in the tree: the tree
   heading changes to "Folders edited by hand", the folder count updates. Rename (F2) and
   remove the folder: the game returns to the parent. Open the structure popover, apply
   "Folders A to Z": the edited tree is replaced.
6. **Sync.** "Sync N changes" opens the plan with the uploads and removals listed; "Sync now"
   runs it: the panel shows step 1 to 9 with the upload bar, the status bar reads "Syncing
   games: step N of 9, …", and at the end "Synced in X s" with the counts; the console's menu
   restarts with the games in the chosen folders, the bar reads "The console is up to date",
   and the storage numbers refresh.
7. **Remove.** Remove a game (confirmation worded "Remove 1 game"): the plan lists its files
   under "Remove"; sync again and check the console's menu.
8. **Keyboard.** In the grid: arrows move, Shift extends, Ctrl+A selects all, Enter opens a
   folder, Backspace goes up. In the tree: arrows move, Right/Left expand/collapse, F2 renames.

### Results

| Date | Console | Steps | Notes |
| --- | --- | --- | --- |
| 2026-10-21 | SNES Classic Mini (EUR) | headless plan only | app sync not run (user absent) |

## Window and polish (desktop app)

Checks that need the real WebKitGTK window rather than the browser preview. No console is
needed; they can run with one attached, nothing is written to it.

### Prerequisites

- A debug build: `pnpm tauri dev`.
- On Wayland, nothing extra: the app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` itself.

### 1. Window state

1. Resize the window to about 1200 × 800 and move it; close it with the window button.
2. Start it again: it opens at the same size (and position on X11; Wayland compositors place
   windows themselves).
3. Maximise, close, start again: it opens maximised.
4. Try to shrink the window below 900 × 600: it stops there and the Library page keeps its
   three columns with the sidebar collapsed to icons.

### 2. Motion and reduced motion

1. Switch pages: the new page fades in and rises slightly; nothing jumps.
2. Trigger a toast (Settings, store an empty key): it slides in from the top and fades out.
3. Set the desktop to reduce motion (GNOME: Accessibility → Reduced animation) and repeat:
   pages and toasts appear at once, the cartridge glyph and the FEL halo do not pulse.

### 3. Keyboard

1. Tab from the sidebar into the Library page: the folder tree takes one Tab stop (the folder
   being viewed), the grid one more (the focused or first tile), the detail panel after.
2. In the grid: arrows, Home, End, PageUp and PageDown move and select, Shift extends, Space
   toggles, Ctrl+A selects all, Enter opens a folder, Backspace goes up, Delete asks to
   remove the selected games in the detail panel, Escape clears the selection.
3. In the tree: arrows move, Right expands, Left collapses then goes up, F2 renames, Delete
   asks to remove the folder.
4. Open the Kernel page's More menu with the keyboard: the first item takes focus, arrows
   move, Escape closes and returns focus to the button.
5. Every icon button announces a name (check with Orca or the Accessibility Inspector).

### 4. Layout: panels scroll, the page never does

Do each step at 900 × 600, at about 1280 × 800 and with the window filling the screen, with
the activity log (Ctrl+L) closed and then open. The sidebar, the page header, the status bar
and the log never move; the log shrinks the page area; nothing is cut off, and the only
scrollbars are the ones listed.

1. Console: on a wide window the status card and the Detected table are on the left (that
   column scrolls when the table is long) and "Everything detected, as data" is a panel on
   the right that scrolls inside itself. Under about 1100 px wide they stack in one scrolling
   column and the data sits in a 320 px box.
2. Kernel: on a wide window the three stage cards are on the left and the step list, the run
   log and the result on the right; start a memboot and watch the step list and the log scroll
   inside their column while the page stays put. Under about 1100 px they stack in one
   scrolling column. After a run the result card shows the outcome, the console's one-line
   summary with the elapsed time, and Open Console / Open Library.
3. Library: the folder tree, the grid and the detail panel are full height and only their
   lists scroll. The tree is at least 240 px wide ("Console menu" and "More games..." are not
   cut), the detail panel at least 340 px. Select a game: the description shows three
   read-only lines; click it (or press Enter on it) to open the editor; type, press Escape:
   the dialog closes, nothing is saved and focus is back on the preview. Open it again, Tab:
   focus stays inside the dialog. Save stores the text. "Advanced…" opens the launch command,
   arguments, compression and the facts in the same kind of dialog.
4. Library sync bar: the Changes block explains the count ("7 changes: 3 files to upload
   (6.0 MB), 2 files to remove, 1 link to reset, 1 empty folder to remove"); "Show changes"
   lists the paths in a dialog; when files are to be removed the bar says that a console
   synced by hakchi2 CE has its files replaced.
5. Settings: on a wide window the settings cards are on the left and the About card on the
   right, each column scrolling on its own; the About card is never cut off.

### Results

| Date | Build | Platform | Result | Notes |
| --- | --- | --- | --- | --- |
| 2026-10-21 | 33242c5+task 11, debug | Arch, GNOME 51 Wayland, WebKitGTK | partial | Native Wayland start without `GDK_BACKEND`: no DMA-BUF protocol error, console identified in 2.5 s (the app sets the variable itself). Window state, motion and keyboard checks not driven by hand (no input tool); layout at 900 × 600 checked in the browser preview only. |
| 2026-10-22 | 11acb49+task 14, preview | Arch, headless Chrome on the preview fixtures | partial | Section 4 checked in the browser preview: every page at 900 × 600, 1280 × 800 and 1990 × 1017, log open and closed (24 combinations measured: the document and the page area never overflow; the only scrolling elements are the listed lists, columns and panels), light and dark. Dialog focus path checked by script: focus lands in the editor, the page behind is inert, cancel closes and returns focus to the preview. Real WebKitGTK window (XWayland capture, 2560 × 1051, SNES Classic Mini attached, read only): the Console page's two columns render, the data panel scrolls inside itself. Not driven in the window: the other pages, the keyboard paths and the live step list and log during a run. |
