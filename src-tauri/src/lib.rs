//! Tauri shell for Cartload: thin commands over the `cartload-*` crates.

/// Version information reported to the frontend status bar.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppVersion {
    /// Application version from `Cargo.toml`.
    pub app: String,
    /// Versions of the library crates linked into this build.
    pub crates: Vec<String>,
}

/// Returns the application version and the versions of the linked crates.
#[tauri::command]
fn app_version() -> AppVersion {
    AppVersion {
        app: env!("CARGO_PKG_VERSION").to_owned(),
        crates: vec![
            cartload_fel::version().to_owned(),
            cartload_console::version().to_owned(),
            cartload_hmod::version().to_owned(),
            cartload_library::version().to_owned(),
        ],
    }
}

/// Builds and runs the Tauri application.
pub fn run() {
    if let Err(err) = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
    {
        eprintln!("error while running Cartload: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_lists_all_crates() {
        let v = app_version();
        assert_eq!(v.app, env!("CARGO_PKG_VERSION"));
        assert_eq!(v.crates.len(), 4);
        assert!(v.crates.iter().any(|c| c.starts_with("cartload-fel")));
    }
}
