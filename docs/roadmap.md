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

- **Independence from unverifiable builds.** Everything Cartload puts on a console should come
  from public source, built reproducibly, with the source published next to it, so no one can
  be locked out by a vanished binary or has to trust one they cannot check. Nintendo's own
  system on the console (the menu, its emulators, Mali's userspace) stays as it is. In order:
  1. **Kernel.** Done: the public kernel brought level with v1.0.4-126's, reproducible
     (`scripts/kernel/build.sh`, `docs/kernel.md`), installed on a SNES (EUR) and play-tested
     (games, sound, controllers, saves, RetroArch). Untested: USB storage, Wi-Fi and Bluetooth
     dongles.
  2. **Userland.** Rebuild every binary of the ramdisk and the package from source (busybox,
     dropbear, e2fsprogs, cryptsetup, kexec, rsync, sunxi-tools, glibc 2.22 and the rest) with
     Buildroot 2015.11.1, the toolchain v1.0.4-126 was built with, plus `sntool` and
     `clovershell` from the hakchi fork; keep the shipped scripts in that fork as their source
     of truth, with their difference from public master documented.
  3. **u-boot from source.** Done: Nintendo's r16-uboot with hakchi's patch and two of
     Cartload's, reproducible (`scripts/uboot/build.sh`, `docs/uboot.md`), run from RAM through
     FEL and installed on a SNES (EUR), cold-started through it.
  4. **Own identity.** Done in code: the package is "Cartload system 0.1.0" (`cartloadVersion`
     in `var/version`, from `scripts/userland/SYSTEM_VERSION`; `hakchiVersion` stays for its
     scripts and hakchi2 CE), embedded in the app as the default with hakchi v1.0.4-126 as the
     alternative (a selector on the Console page), and shown in the console's status. CI
     (`.github/workflows/system.yml`) rebuilds it from source and fails unless it is byte for
     byte the committed `data/cartload-system-<version>.hmod`; a `system-v<version>` tag puts it,
     its report and checksums in a draft release. To do: the first CI run, the first release,
     the play test of the whole system.
  5. **NES and Famicom Minis**: the same chip and kernel; needs testers with the hardware.
  Firmware blobs stay blobs (ARISC, the rtl8188fu Wi-Fi firmware, `fes1.bin`), listed with
  their origin.
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
