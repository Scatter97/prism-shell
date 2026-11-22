//! Native Windows integration for Prism Shell.
//!
//! Portable layout and window-classification logic lives in the top-level
//! modules so it can be unit-tested on Linux. The actual Win32 host, window
//! tracker, icon extraction, and controller are compiled only on Windows.
//!
//! This crate never replaces Explorer or hides the Windows taskbar. It runs as
//! a set of tool windows alongside the normal Windows shell.

pub mod layout;
pub mod model;
pub mod naming;

#[cfg(windows)]
mod controller;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod icons;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod window;

#[cfg(windows)]
pub use controller::run_shell;
#[cfg(windows)]
pub use platform::show_error_message;
