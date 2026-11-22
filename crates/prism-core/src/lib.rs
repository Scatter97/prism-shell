//! Portable Prism Shell domain logic.
//!
//! This crate contains no Win32, no Slint, and no I/O. It defines the dock
//! model, system capability/state model, layout arithmetic, configuration
//! validation, and clock formatting. Both the Linux mock backend and the
//! Windows production backend feed the same types.

pub mod clock;
pub mod config;
pub mod dock;
pub mod ids;
pub mod layout;
pub mod system;

pub use clock::{ClockText, format_clock};
pub use config::{ConfigError, PrismConfig};
pub use dock::{AppClickIntent, AppDefinition, AppEntry, AppWindow, DockState};
pub use ids::{AppId, IconKey, WindowId};
pub use layout::{DockFit, fit_dock_apps};
pub use system::{
    AudioOutput, AudioState, BatteryState, BluetoothState, BrightnessState, Capability,
    ConnectivityState, ControlPhase, MediaSession, MediaState, PowerAction, SettingsPage,
    SystemAction, SystemSnapshot, ToggleId, ToggleState, WifiState,
};
