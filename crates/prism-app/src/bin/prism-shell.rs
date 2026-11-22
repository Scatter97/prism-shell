#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Production shell bootstrap.
//!
//! The Windows path creates real taskbar/Control Center/launcher/preview
//! surfaces that run alongside Explorer. Taskbar replacement and standalone
//! Windows-key handling are opt-in experiments and remain disabled by default.

fn main() {
    #[cfg(windows)]
    {
        let args = match prism_app::shell_args::parse(std::env::args().skip(1)) {
            Ok(args) => args,
            Err(error) => {
                eprintln!("{error}\n\n{}", prism_app::shell_args::usage());
                std::process::exit(2);
            }
        };
        if args.help {
            println!("{}", prism_app::shell_args::usage());
            return;
        }

        let _log_guard = prism_app::logging::init_with_name(false, "prism-shell");
        let loaded = prism_app::config::load(args.config.as_deref());
        if let Some(warning) = &loaded.warning {
            tracing::warn!("{warning}");
        }
        let mut config = loaded.config;
        if args.taskbar_replacement {
            config.experimental.taskbar_replacement = true;
        }
        if args.windows_key_launcher {
            config.experimental.windows_key_launcher = true;
        }
        if args.windows_key_fallback_hotkey {
            config.experimental.windows_key_fallback_hotkey = true;
        }
        if let Err(error) = prism_windows::run_shell(config) {
            tracing::error!(%error, "Prism Windows shell exited with an error");
            prism_windows::show_error_message(
                "Prism Shell failed to start",
                &format!(
                    "{error}\n\nThe Windows taskbar should be visible. If Prism was in experimental \
                     replacement mode, run prism-recovery.exe --restore. Check the Prism log for \
                     details."
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
