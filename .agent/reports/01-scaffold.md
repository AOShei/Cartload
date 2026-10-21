# Report: 01-scaffold

## What was done

- **Cargo workspace** at the root (resolver 3, edition 2024, `rust-version = 1.90`) with members
  `crates/fel`, `crates/console`, `crates/hmod`, `crates/library`, `src-tauri`. Shared
  `[workspace.dependencies]` for `thiserror`, `anyhow`, `tracing`, `serde`, `serde_json`, `tokio`
  and path entries for the four `cartload-*` crates. Workspace lints: `missing_docs`,
  `unsafe_code`, `clippy::unwrap_used` as warnings (so `-D warnings` catches them).
- **Crates** `cartload-fel`, `cartload-console`, `cartload-hmod`, `cartload-library`: each has a
  one-line crate doc, a `#[non_exhaustive]` placeholder `Error` enum (`thiserror`), a `Result`
  alias, a `version()` const fn, two unit tests, and a placeholder `src/main.rs` binary of the
  same name that prints the crate version. Later tasks replace the binaries with real CLIs.
- **`src-tauri`** (`cartload-app`): Tauri 2.12 shell, lib + bin layout, depends on all four
  crates, exposes the `app_version` command returning the app version and the linked crate
  versions. Identifier `org.cartload.app`, product name and window title "Cartload".
  Capability `default` grants `core:default` to the `main` window. Icons generated with
  `pnpm tauri icon` from `src-tauri/icons/icon.svg` (a simple cartridge glyph); the mobile icon
  sets were removed and the 1024px intermediate PNG is gitignored.
- **Frontend** (Svelte 5 runes, TypeScript strict, Vite 8, pnpm): `App.svelte` with a sidebar
  (Console, Library, Kernel), a main area showing the current entry name, and a footer status
  bar showing the `app_version` result (or the error if not running inside Tauri). Plain CSS
  with light and dark palettes via `prefers-color-scheme`. No component library.
- **Tooling**: `rustfmt.toml`, `pnpm check` (svelte-check), `.github/workflows/ci.yml` on
  ubuntu-latest installing the Tauri Linux prerequisites and running fmt, clippy
  (`-D warnings`), `cargo test`, `pnpm check` and the Vite build.
- `docs/hardware-tests.md` created with a heading and the per-crate checklist plan.

## Verified

All on this machine (Rust 1.95, pnpm 11.1.2, Node 24, WebKitGTK 4.1):

- `cargo build --workspace`: ok.
- `cargo test --workspace`: 15 test binaries, all ok (9 unit tests pass).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all --check`: clean.
- `pnpm install && pnpm check`: 94 files, 0 errors, 0 warnings.
- `pnpm tauri build --debug`: built `target/debug/cartload-app` and bundled deb, rpm and
  AppImage.
- `cargo run -p cartload-fel` / `-p cartload-library`: print their versions.
- Smoke launch of `target/debug/cartload-app` for 6 s: runs under `GDK_BACKEND=x11` and under
  Wayland with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Decisions

- Manual setup instead of `create-tauri-app` (non-interactive run, and the template pulls in the
  opener plugin and demo UI we would have deleted anyway).
- Tauri crates pinned to `2.12` / `tauri-build 2.7` to match `@tauri-apps/cli` and
  `@tauri-apps/api` 2.12; the Tauri CLI refuses mismatched minor versions. Tauri 2.12 requires
  Rust 1.90, hence the workspace `rust-version`. Tauri 3 is alpha only and was not used.
- Each crate got a placeholder binary now so the "every crate has a CLI" rule holds from the
  start; names match the briefs (`cartload-fel`, `cartload-console`, ...).
- CSP allows inline styles (Svelte scoped styles are injected by Vite in production builds as a
  CSS file, so this could be tightened later once confirmed).

## Left or uncertain

- Not visually verified: no screenshot tool is available in this GNOME Wayland session. The
  bridge is type-checked on both sides and the command has a unit test, but a human should open
  the app once and confirm the footer reads "Cartload 0.1.0 · 4 crates".
- Launching natively on Wayland on this machine fails with `Error 71 (Protocol error)
  dispatching to Wayland display` (WebKitGTK GBM buffer creation). This is an environment /
  GPU driver issue, not app code; the X11 backend and the dmabuf-disabled Wayland path both
  work. Worth noting in docs if it reproduces on other machines.
- `.agent/pyproject.toml` and `.agent/uv.lock` were already untracked before this task and were
  left out of the commit (`.agent/` is off limits except for reports).
