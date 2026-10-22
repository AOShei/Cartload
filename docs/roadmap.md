# Roadmap

Done: the Tauri shell, the FEL, console, hmod, library and flow crates, the guided and
resumable kernel install, the library with folders, art and a diff-based sync, the Linux
setup installer, a log file with a diagnostics report, and the release workflow
(`.github/workflows/release.yml`).

Next, in order:

1. First sync from the app on hardware, then the base mods flow and an install on a unit
   hakchi never touched (`docs/hardware-tests.md`).
2. macOS run on hardware (the universal build comes from the release workflow).
3. Windows run on hardware, with the WinUSB (Zadig) and RNDIS driver steps written up from
   a real machine.
4. An update check against the GitHub releases.

Not planned: code signing and notarisation. The builds stay unsigned by choice (no yearly
Apple developer account or Windows certificate); the README explains the one-time warning
on each desktop.
