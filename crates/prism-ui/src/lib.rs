//! Prism Shell presentation layer.
//!
//! Slint owns rendering and input. This crate maps portable snapshots into
//! Slint data and exposes callbacks; it contains no Win32 calls and no
//! operating-system side effects.

slint::include_modules!();

pub mod model;
