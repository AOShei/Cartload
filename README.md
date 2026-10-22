<p align="center">
  <img src="docs/readme-banner.svg?v=2" alt="Cartload: install hakchi and manage games on the NES, SNES and Famicom Classic Mini" width="100%">
</p>

# Cartload

A desktop tool for the NES, SNES and Famicom Classic Mini consoles: install the
hakchi kernel, manage a games library, and sync it to the console. Linux, macOS and
Windows, built with Tauri 2, Rust and Svelte.

Cartload is a from-scratch implementation. It reuses the GPL-3 hakchi kernel package
(hmod) built from madmonkey's hakchi sources and the device-side protocol knowledge
documented in `docs/`, but none of the hakchi2 CE application code.

Status: early development. See `docs/roadmap.md`.

## Running on Linux

`pnpm install`, then `pnpm tauri dev` for a debug build or `pnpm tauri build` for the
bundles. The console needs a udev rule for the FEL device (`1f3a:efe8`) and a link-local
address on the hakchi network gadget (`04e8:6863`); see `docs/device-protocol.md`.

On Wayland, WebKitGTK's DMA-BUF renderer crashes with a protocol error on some compositor
and driver combinations before the window appears. Cartload therefore sets
`WEBKIT_DISABLE_DMABUF_RENDERER=1` at startup unless the variable is already set, so
`WEBKIT_DISABLE_DMABUF_RENDERER=0 cartload` turns the renderer back on. The window size,
position and maximised state are remembered between runs.

License: GPL-3.0-or-later.
