//! Cartload desktop entry point.
// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    prepare_webkit();
    cartload_app_lib::run()
}

/// WebKitGTK's DMA-BUF renderer dies with a Wayland protocol error on some compositor and
/// driver combinations before the first frame is drawn. Turning it off costs nothing visible,
/// so it is off unless the user has set the variable themselves (to `0` to turn it back on).
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn prepare_webkit() {
    const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if std::env::var_os(VAR).is_none() {
        // SAFETY: this runs first thing in `main`, before any other thread exists, so nothing
        // can read the environment while it is written.
        unsafe { std::env::set_var(VAR, "1") };
    }
}

#[cfg(not(target_os = "linux"))]
fn prepare_webkit() {}
