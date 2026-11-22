#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! Production shell bootstrap.
//!
//! This milestone deliberately does not implement Windows shell integration.
//! The binary exists so the cross-compilation path is exercised, and it fails
//! safely and visibly instead of pretending mock data is real Windows state.

fn main() {
    #[cfg(windows)]
    {
        windows_bootstrap();
    }
    #[cfg(not(windows))]
    {
        eprintln!("Prism Shell production integration is not implemented in this milestone.");
        eprintln!("Use the Linux development preview: cargo run -p prism-app --bin prism-preview");
        std::process::exit(3);
    }
}

#[cfg(windows)]
fn windows_bootstrap() -> ! {
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MessageBoxW,
    };
    use windows::core::w;

    // DPI awareness must be set before any window is created. This bootstrap
    // creates only a diagnostic message box; it does not touch Explorer.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        MessageBoxW(
            None,
            w!(
                "Windows integration is not implemented in this milestone.\n\nNo Explorer, taskbar, registry, or system settings were changed. The Linux development preview is available through the `prism-preview` executable."
            ),
            w!("Prism Shell — integration not available"),
            MB_OK | MB_ICONWARNING | MB_SETFOREGROUND,
        );
    }
    std::process::exit(2);
}
