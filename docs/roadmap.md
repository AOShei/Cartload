# Roadmap

Done: the Tauri shell, the FEL, console, hmod, library and flow crates, the guided and
resumable kernel install, the library with folders, art and a diff-based sync, the Linux
setup installer, a log file with a diagnostics report, the release workflow
(`.github/workflows/release.yml`), the emulator module installer with cores for NES, SNES,
Mega Drive / Genesis, the Game Boy family and N64, and SNES games on NES-family consoles.
The first sync from the app ran on a SNES Classic Mini (EUR) on 2026-10-23.

Next, in order:

1. The `v0.1.0` pre-release, which also runs the release workflow for the first time.
2. Test reports from people with the hardware this project does not have
   (`docs/help-testing.md`): NES and Famicom Minis, never-modified units, macOS, Windows,
   the module installer and the new cores.
3. Windows: the WinUSB (Zadig) and RNDIS driver steps written up from a real machine.
4. An update check against the GitHub releases.
5. More systems through RetroArch where the console runs them well (Master System, Game
   Gear, PC Engine, PlayStation): a core module and a launcher each.

Not planned: code signing and notarisation. The builds stay unsigned by choice (no yearly
Apple developer account or Windows certificate); the README explains the one-time warning
on each desktop.
