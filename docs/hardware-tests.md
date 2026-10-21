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
  v1.0.4-126) and `fes1.bin` (`hakchi_gui/data/fes1.bin` in the hakchi2 CE source tree,
  14048 bytes, md5 `2d1c7b3b9fe5f88bc93aef662abe5cfe`).
- Console connected over USB, powered off. Any console state is fine: memboot does not depend on
  hakchi being installed.

Set `HMOD` and `FES1` to those two paths for the commands below.

### 1. Dry run (no console needed)

```
cargo run -p cartload-fel -- memboot --hmod "$HMOD" --fes1 "$FES1" --dry-run
```

Expected: five summary lines and `dry run: images are valid, not touching USB`. For v1.0.4-126:
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
cargo run -p cartload-fel -- memboot --hmod "$HMOD" --fes1 "$FES1"
```

Expected log sequence (timestamps omitted):

```
INFO extracted from hmod entry="boot/boot.img" len=4585472
INFO extracted from hmod entry="boot/uboot.bin" len=761856
INFO extracted from hmod entry="var/version" len=84
hmod: ... (bootVersion='1.0.3', hakchiVersion='v1.0.4-126', kernelVersion='3.4.113.29-madmonkey')
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
