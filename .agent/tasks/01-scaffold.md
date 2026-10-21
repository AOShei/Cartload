# Task 01: scaffold the repository

Create the project skeleton described in CLAUDE.md. Nothing here touches hardware.

1. Tauri 2 app with a Svelte 5 + TypeScript frontend using pnpm (`create-tauri-app` with the
   svelte-ts template, or an equivalent manual setup). App identifier `org.cartload.app`,
   product name "Cartload". Window title "Cartload".
2. Cargo workspace at the repo root with members `crates/fel`, `crates/console`, `crates/hmod`,
   `crates/library`, and `src-tauri`. Edition 2024. Shared `[workspace.dependencies]` for
   `thiserror`, `anyhow`, `tracing`, `serde`, `serde_json`, `tokio`. Each crate compiles with a
   `lib.rs` containing a one-line doc comment and a placeholder error enum. `src-tauri` depends
   on all four crates and exposes one Tauri command `app_version` the frontend calls and shows.
3. Frontend: replace the template page with a minimal shell: a left sidebar with three entries
   (Console, Library, Kernel), a main area showing the entry name, and a footer status bar that
   shows the `app_version` result. Plain CSS, light and dark via `prefers-color-scheme`.
4. Tooling: `rustfmt.toml`, `clippy` clean with `-D warnings`, `pnpm check` (svelte-check) clean,
   `.github/workflows/ci.yml` running fmt, clippy, cargo test and `pnpm check` on ubuntu-latest
   with the Tauri Linux prerequisites installed.
5. `docs/hardware-tests.md` created with a heading and a note that checklists land per crate.
6. Verify: `cargo build --workspace`, `cargo test --workspace`, `pnpm install && pnpm check`
   and `pnpm tauri build --debug` (or `cargo build -p cartload-app`) all succeed on this machine.
7. Commit.

Do not add UI features beyond the shell. Do not add a component library.
