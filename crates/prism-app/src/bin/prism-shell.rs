#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Production shell bootstrap.
//!
//! The Windows path creates real taskbar/Control Center/launcher surfaces that
//! run alongside Explorer. It does not replace Explorer, hide the Windows
//! taskbar, register a shell, or auto-start.

fn main() {
    #[cfg(windows)]
    {
        let _log_guard = prism_app::logging::init_with_name(false, "prism-shell");
        let loaded = prism_app::config::load(None);
        if let Some(warning) = &loaded.warning {
            tracing::warn!("{warning}");
        }
        if let Err(error) = prism_windows::run_shell(loaded.config) {
            tracing::error!(%error, "Prism Windows shell exited with an error");
            prism_windows::show_error_message(
                "Prism Shell failed to start",
                &format!(
                    "{error}\n\nExplorer and the Windows taskbar were not modified. Check the Prism log for details."
                ),
            );
            std::process::exit(1);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("The native shell is Windows-only.");
        eprintln!("Use the Linux development preview: cargo run -p prism-app --bin prism-preview");
        std::process::exit(3);
    }
}
