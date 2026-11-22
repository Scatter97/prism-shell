//! Independent Windows taskbar recovery utility and watchdog.
//!
//! This binary deliberately has no dependency on the Prism shell process. It
//! is started before experimental taskbar hiding, restores the native taskbar
//! when the shell dies or stops its heartbeat, and can also be run manually:
//! `prism-recovery.exe --restore`.

fn main() {
    #[cfg(windows)]
    {
        std::process::exit(prism_windows::run_recovery_cli());
    }
    #[cfg(not(windows))]
    {
        eprintln!("prism-recovery is Windows-only.");
        eprintln!(
            "It restores Shell_TrayWnd/Shell_SecondaryTrayWnd state; there is nothing to restore on Linux."
        );
        std::process::exit(3);
    }
}
