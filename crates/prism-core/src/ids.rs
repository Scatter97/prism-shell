use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable identifier for an application entry shown in the dock.
///
/// For desktop applications this is derived from the executable path or
/// AppUserModelID. For preview fixtures it is a deterministic slug.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AppId(String);

impl AppId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AppId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Opaque handle for a top-level application window.
///
/// On Windows this will carry an `HWND` represented as `isize`. Keeping the
/// portable type opaque prevents UI code from depending on Win32 values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowId(u64);

impl WindowId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifies a designed application icon bundled with the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IconKey {
    Files,
    Browser,
    Terminal,
    Mail,
    Music,
    Notes,
    Generic,
}
