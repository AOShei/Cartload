# Roadmap

Done: the Tauri shell, the FEL, console, hmod, library and flow crates, the guided and
resumable kernel install, the library with folders, art and a diff-based sync, the Linux
setup installer, a log file with a diagnostics report, the release workflow
(`.github/workflows/release.yml`), the emulator module installer with cores for NES, SNES,
Mega Drive / Genesis, the Game Boy family and N64, and SNES games on NES-family consoles.
The first sync from the app ran on a SNES Classic Mini (EUR) on 2026-10-23. `v0.1.0` was
released on 2026-10-23. `v0.2.0` adds the merged Console page, emulator updates and removal,
7z support installed with hakchi and added by the sync when missing, and no dependence on the
hakchi servers: the kernel package is built in and the emulators are mirrored, both pinned by
SHA-256. `v0.2.1` (2026-10-28) replaces the unpublished v0.2.0 draft: it shows the 7z step in
the sync panel and stops counting the built-in games' links as changes on every sync.

Next, in order:

1. Test reports from people with the hardware this project does not have
   (`docs/help-testing.md`): NES and Famicom Minis, never-modified units, macOS, Windows,
   the module installer and the new cores.
2. Windows: the WinUSB (Zadig) and RNDIS driver steps written up from a real machine.
3. An update check against the GitHub releases.
4. More systems through RetroArch where the console runs them well (Master System, Game
   Gear, PC Engine, PlayStation): a core module and a launcher each.

Later, larger projects (not scheduled):

- **Our own hakchi kernel build.** The shipped hakchi v1.0.4-126 has no published source (see
  `data/README.md`). Pick up from the public tree where it stops (`AOShei/hakchi` at `fd525ac`,
  `AOShei/sun-nontendocm-kernel` at `f8c6e0b`), rebuild the kernel and ramdisk with a pinned
  toolchain in CI, bring it level with v1.0.4-126's behaviour on the NES, SNES and Famicom Minis,
  and ship that with its full source.
- **Emulators built from official libretro source.** Today the emulator modules are
  KMFDManic's builds, mirrored and pinned. libretro publishes no RetroArch binary for 32-bit ARM
  Linux and its armhf cores stopped in 2020, so this means building RetroArch (with the
  `mali_fbdev` context) and the cores (`platform=classic_armv7_a7` where a core has it) in CI
  against a glibc 2.23 armhf toolchain, plus our own console launchers and config to replace
  KMFD's. Needs hardware testing of video, audio and controllers.
- The update check (item 3 above) must only consider `v*` tags: the release
  `emulators-km-2022-10-31` holds mirrored files, not an app version.

Not planned: code signing and notarisation. The builds stay unsigned by choice (no yearly
Apple developer account or Windows certificate); the README explains the one-time warning
on each desktop.
