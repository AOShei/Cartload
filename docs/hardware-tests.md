# Hardware tests

Manual checklists for steps that need a real console. Automated tests never touch hardware;
anything that does lives here instead.

Checklists land per crate as each one gains hardware-facing functionality:

- `crates/fel`: FEL memboot (console in FEL mode, `1f3a:efe8`).
- `crates/console`: discovery, SSH and FTP against a booted hakchi kernel (`04e8:6863`).
- `crates/hmod`: kernel package install and version check on the console.
- `crates/library`: sync of a small library and verification on the console menu.

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
