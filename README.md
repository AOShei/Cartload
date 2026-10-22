<p align="center">
  <img src="docs/readme-banner.svg?v=4" alt="Cartload: install hakchi and manage games on the NES, SNES and Famicom Classic Mini" width="100%">
</p>

# Cartload

A desktop app for the NES, SNES and Famicom Classic Mini consoles. It installs the
hakchi kernel, keeps a library of games with cover art and folders, and syncs that library
to the console over the USB cable. Linux, macOS and Windows, built with Tauri 2, Rust and
Svelte.

Cartload is a from-scratch implementation. It uses the GPL-3 hakchi kernel package built
from madmonkey's hakchi sources and the device-side protocol documented in `docs/`, but
none of the hakchi2 CE application code. Cartload does not support the Mega Drive Mini.

**Status:** pre-release. The kernel install, kexec reboot, library and art fetching have
been verified on a SNES Classic Mini (EUR) running Linux. The first sync from the app, the
macOS build and the Windows build are still to be tested on hardware; see
[What has been tested](#what-has-been-tested).

## Install

Builds for each platform are attached to the
[GitHub releases](https://github.com/AOShei/Cartload/releases). They are not code-signed
and will not be: a free-software project is not going to pay Apple and a certificate
authority yearly for the privilege. Each desktop shows a warning the first time; the steps
below get past it.

### Linux

- **Debian / Ubuntu:** install the `.deb`. It puts the udev rule in
  `/usr/lib/udev/rules.d/70-cartload.rules`; reconnect the console afterwards.
- **Other distributions:** make the `.AppImage` executable and run it. On first run, open
  **Settings → Linux setup** and press **Install**: the app asks for your password and
  installs the udev rule (USB access to the console in FEL mode, and the `hakchi0` name for
  its network interface) and a NetworkManager profile that gives `hakchi0` a link-local
  address. Without NetworkManager, give `hakchi0` an address in 169.254.0.0/16 yourself.
- With firewalld, `hakchi0` must be in the trusted zone. The installed profile puts it
  there; the Linux setup card shows the zone once the console is connected and can move
  it. Other firewalls must allow traffic to 169.254.13.37 (SSH and FTP).
- On Wayland, Cartload sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` at startup because
  WebKitGTK's DMA-BUF renderer crashes on some compositor and driver combinations. Set the
  variable to `0` before starting to turn the renderer back on.

### macOS

Open the `.dmg` and drag Cartload to Applications. The app is not notarised, so macOS
reports it as damaged on first launch. Clear the quarantine flag once:

```sh
xattr -cr /Applications/Cartload.app
```

The console's network gadget appears as a USB Ethernet interface with a link-local address;
nothing to install. Not yet tested on hardware.

### Windows

Not yet tested on hardware. Two drivers are involved:

- **FEL mode** (the console held in Reset while powering on) shows up as USB device
  `1f3a:efe8`. Bind it to the **WinUSB** driver with [Zadig](https://zadig.akeo.ie/) once;
  Cartload talks to it through WinUSB.
- **The hakchi network link** (`04e8:6863`) uses RNDIS. Windows 10 installs the driver by
  itself. Windows 11 24H2 reportedly no longer installs it automatically; pick
  "USB RNDIS Adapter" by hand in Device Manager if the console does not get an address.

## Using it

1. **Console page.** Connect the console with its USB cable. The page shows what is
   attached: nothing, a console in FEL mode, stock firmware, or a running hakchi kernel with
   its version and free space.
2. **Kernel page.** Pick the console model, put the console in FEL mode as the drawing
   shows (hold Reset, switch Power on, release after two seconds; the screen stays black),
   and press **Install hakchi**. The thirteen steps run in order and are resumable: if
   anything stops, run it again and finished steps are skipped. The same page boots the
   console from memory without writing anything, saves the stock kernel to a file, and
   removes hakchi again (factory reset).
3. **Library page.** Add ROMs (`.sfc`, `.smc`, `.fig`, `.swc`, `.nes`, `.zip`), arrange
   them in the folders the console will show, give them covers, and press **Sync**. Sync is
   a diff: only changed files travel, and files already on the console are kept. If the
   cable is pulled mid-way, run the sync again once the console is back.
4. **Settings page.** Theme, the log, the Linux setup, the library location and the
   TheGamesDB key.

### Cover art

Covers and metadata come from [TheGamesDB](https://thegamesdb.net). Create a free account,
request an API key under your profile, and paste it in **Settings → Library**. The key is
stored in the library's settings file on your machine and never leaves it except in
requests to TheGamesDB. The console's own built-in covers are read from the console itself
and are never part of Cartload.

### What Cartload changes on the console

- **Install:** backs up the stock kernel on the console, grows the data partition, flashes
  u-boot and the hakchi kernel. The stock kernel backup is what factory reset restores.
- **Sync:** stops the console's menu, writes games under hakchi's games directory on the
  data partition, links the built-in games, writes the config values hakchi reads, and
  restarts the menu. Save states and the stock firmware are not touched.
- Nothing is written to the console at any other time: the app only reads its status.

## Bug reports

Open **Settings → Activity log → Copy diagnostics** and paste the text into a
[GitHub issue](https://github.com/AOShei/Cartload/issues). The report holds the build, the
system, the console as last seen (including its serial number), the last install and sync,
and the recent log at debug level. It never contains your TheGamesDB key. The same log is
written to a file (`~/.local/share/cartload/logs/cartload.log` on Linux, the application
data folder elsewhere) and kept across runs.

## What has been tested

`docs/hardware-tests.md` is the record. On a SNES Classic Mini (EUR) under Linux: FEL
memboot, the full kernel install (from stock and from a running hakchi kernel through
kexec), console detection and identification, art fetching, and a read-only sync plan.
Not yet on hardware: the first sync from the app, the base mods flow, installing on a unit
that was never modified, macOS, Windows.

## Building from source

Rust 1.93 or later, Node 24 and pnpm. On Linux, the WebKitGTK 4.1 development packages
(`libwebkit2gtk-4.1-dev`, `librsvg2-dev`, `libayatana-appindicator3-dev`, `patchelf`).

```sh
pnpm install
pnpm tauri dev      # debug build with live reload
pnpm tauri build    # the installers, under target/release/bundle/
```

`cargo test --workspace` runs every test without hardware. `scripts/version.sh 0.2.0` sets
the version in the three places it lives; pushing a `v0.2.0` tag builds the installers on
GitHub and attaches them to a draft release.

Each crate has a CLI (`cargo run -p cartload-fel -- --help`, and the same for
`cartload-console`, `cartload-hmod`, `cartload-library`, `cartload-flow`) for working
without the UI.

## License

GPL-3.0-or-later. `data/README.md` lists the binaries embedded from hakchi and hakchi2 CE
and their origin. Cartload is not affiliated with the console manufacturer; NES, SNES and
Famicom are the names of the consoles it works with.
