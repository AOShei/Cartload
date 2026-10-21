# Roadmap

1. Scaffold: Tauri 2 + Svelte 5 + TypeScript frontend, Rust workspace of small crates.
2. FEL crate: Allwinner USB boot protocol over nusb; CLI memboot of the console with no UI.
3. Console crate: discovery by direct probe of 169.254.13.37 (mDNS second), SSH session,
   FTP transfer, typed wrappers for the device-side `hakchi` script.
4. Hmod crate: parse hakchi.hmod, extract boot.img and u-boot, fetch the latest package.
5. Kernel flow: memboot, grow partition, install hmod, reboot, reconnect. Guided, resumable.
6. Library: games on disk, per-core launch entries, art from TheGamesDB plus a hash index,
   visible folder tree, sync as a diff.
7. Ship: AppImage and AUR, macOS bundle, Windows with the WinUSB driver step.
