//! Platform boundary for Prism Shell.
//!
//! The UI and application controller speak only in the traits and semantic
//! actions defined here. The Linux development preview uses explicit mocks;
//! the Windows backend will implement the same contracts with documented
//! Win32/Core Audio/WinRT APIs.

pub mod mock;
pub mod traits;

pub use mock::{MockPlatform, PreviewScenario};
pub use traits::{
    ActionOutcome, LaunchReceipt, PlatformError, PlatformEvent, SystemServices, WindowServices,
};
