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

**Status:** pre-release. The kernel install, the library, art fetching and syncing from the
app have been verified on a SNES Classic Mini (EUR) running Linux. Other consoles, macOS and
Windows have not been run on hardware yet. If you have one of them,
[help test](docs/help-testing.md).

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
   attached: nothing, a console in FEL mode, stock firmware, or a console running a custom
   system (Cartload's or hakchi's) with its version and free space.
2. **Install the system** (Console page, on a console without it). Pick the console model,
   put the console in FEL mode as the drawing shows (hold Reset, switch Power on, release
   after two seconds; the screen stays black), and press **Install**. The kernel package is
   **Cartload system** by default: Cartload's own console system, built from public source
   and reproducible (see below); **hakchi v1.0.4-126**, the package hakchi2 CE installs, is
   the alternative in the same list. The progress opens in a window over the app; the
   steps run in order and are resumable: if anything stops, run it again and finished steps
   are skipped. Once the system is installed the same page manages the emulators and offers
   reinstall, saving the stock kernel to a file and removing the custom system again
   (factory reset).
3. **Library page.** Add ROMs (SNES `.sfc`, `.smc`, `.fig`, `.swc`; NES `.nes`; and, with
   their RetroArch core installed, Mega Drive / Genesis `.md`, `.gen`, `.smd`, Game Boy
   `.gb`, `.gbc`, Game Boy Advance `.gba`, N64 `.z64`, `.n64`, `.v64`; or a `.zip` of any of
   them), arrange them in the folders the console will show, give them covers, and press
   **Sync**. Sync is
   a diff: only changed files travel, and files already on the console are kept. If the
   cable is pulled mid-way, run the sync again once the console is back.
4. **Settings page.** Theme, the log, the Linux setup, the library location and the
   TheGamesDB key.

### The Cartload system

What Cartload installs on the console by default: a kernel, u-boot and every program of the boot
ramdisk and the package, built from public source (Nintendo's open-source archive, hakchi's
public tree and the fork that holds its v1.0.4-126 scripts, the upstream projects), laid out like
hakchi v1.0.4-126 and running its scripts. Every file's source is listed in the build's report;
the builds are reproducible, and CI rebuilds the package the app embeds and fails unless it is
byte for byte the same. The only binaries without source are Realtek's Wi-Fi firmware (from
linux-firmware) and, outside the package, the DRAM setup in the console's boot0 and in the FEL
loader. Releases are tagged `system-v<version>`. Details: `docs/kernel.md`, `docs/userland.md`,
`docs/uboot.md`. Not in it yet: hakchi's USB gamepad driver (Nintendo controllers work as
before).

### Emulator modules

Each console has only its own system's emulator: the SNES Mini cannot play NES games, the
NES and Famicom Minis cannot play SNES games, and neither plays anything else. **Settings →
Emulators** installs RetroArch and one core per system from KMFD's Mod Hub, the same modules
hakchi2 CE offers:

| System | Core | Notes |
|---|---|---|
| NES | FCEUmm | Used automatically on SNES-family consoles |
| SNES | Snes9x | Used automatically on NES-family consoles (untested on hardware) |
| Mega Drive / Genesis | Genesis Plus GX | Full speed |
| Game Boy, Game Boy Color | Gambatte | Full speed |
| Game Boy Advance | mGBA | Full speed |
| N64 | Mupen64Plus | **Runs poorly.** The console's chip is too slow for most N64 games; a few lighter ones are playable. The app asks before installing it. Use at your own risk. |

Cartload downloads the module, checks it against the hub's checksum, installs it with the
console's own tools and reboots the console once. NES and SNES games can also be switched
to RetroArch on a console that has its own emulator, with the Emulator choice in a game's
details or for several games at once from the multi-selection panel; the other systems
always run through RetroArch. SNES games need a SNES controller on an NES Mini for the
X, Y, L and R buttons. Modules live in hakchi's overlay on
the data partition: the console's own files and its menu are untouched, and a factory reset
removes them with everything else.
When a sync needs a module the console lacks, the plan says so and offers the install.

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
kexec), console detection and identification, art fetching, and syncing from the app,
including NES games through RetroArch. Not yet on hardware: the NES and Famicom Minis, SNES
games on them, installing on a unit that was never modified, the emulator module installer,
the Mega Drive, Game Boy and N64 cores, macOS and Windows.
[docs/help-testing.md](docs/help-testing.md) lists them with what each test needs; reports
go in a [test report](https://github.com/AOShei/Cartload/issues/new?template=test-report.yml).

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
and their origin.

Cartload ships the hakchi kernel package v1.0.4-126 (GPL) unchanged. Its authors never
published the source of that exact build; the closest published source, archived here, is
[AOShei/hakchi](https://github.com/AOShei/hakchi) (fork of madmonkey1907/hakchi, last upstream
commit `fd525ac`) and [AOShei/sun-nontendocm-kernel](https://github.com/AOShei/sun-nontendocm-kernel)
(fork of madmonkey1907/sun-nontendocm-kernel, `f8c6e0b`). The emulator modules Cartload installs
are KMFDManic's builds, mirrored unchanged on the
[emulators-km-2022-10-31](https://github.com/AOShei/Cartload/releases/tag/emulators-km-2022-10-31)
release with their licences. Cartload is not affiliated with any console manufacturer; NES, SNES,
Famicom, Mega Drive, Genesis, Game Boy and N64 are the names of the systems it works
with.
