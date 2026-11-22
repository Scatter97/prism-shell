use prism_core::{AppDefinition, SystemAction, SystemSnapshot, WindowId};
use thiserror::Error;

/// A platform operation failed. The UI maps these to honest control states and
/// non-blocking messages rather than silently swallowing errors.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PlatformError {
    #[error("{operation} is not supported by the current platform: {detail}")]
    Unsupported { operation: String, detail: String },
    #[error("{operation} requires a Windows user permission")]
    PermissionRequired { operation: String },
    #[error("{operation} is unavailable: {detail}")]
    Unavailable { operation: String, detail: String },
    #[error("{operation} failed: {detail}")]
    Failed { operation: String, detail: String },
}

impl PlatformError {
    pub fn unsupported(operation: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Unsupported {
            operation: operation.into(),
            detail: detail.into(),
        }
    }

    pub fn unavailable(operation: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Unavailable {
            operation: operation.into(),
            detail: detail.into(),
        }
    }

    pub fn failed(operation: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Failed {
            operation: operation.into(),
            detail: detail.into(),
        }
    }

    pub fn permission_required(operation: impl Into<String>) -> Self {
        Self::PermissionRequired {
            operation: operation.into(),
        }
    }
}

/// Result of a dispatched action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionOutcome {
    /// The platform accepted the request and the snapshot is authoritative.
    Applied,
    /// A slow operation is in flight. The controller must poll/reconcile.
    Pending,
    /// The preview backend intentionally did not perform a system operation.
    /// The message is suitable for a non-intrusive toast.
    Simulated(String),
}

/// Asynchronous notification produced by reconciliation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformEvent {
    Notice(String),
    Error(String),
    StateChanged,
}

/// System status and controls. On Windows this is backed by documented APIs.
/// Every method must be fast and must not block the UI thread; long-running
/// work belongs on a worker and reports back through `poll`.
pub trait SystemServices {
    fn snapshot(&self) -> SystemSnapshot;

    fn execute(&mut self, action: SystemAction) -> Result<ActionOutcome, PlatformError>;

    /// Drain notifications produced by background reconciliation.
    fn poll(&mut self) -> Vec<PlatformEvent> {
        Vec::new()
    }
}

/// Result of asking the platform to launch an application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchReceipt {
    /// Process id when it is known immediately. Window discovery is asynchronous
    /// on Windows, so a receipt without a window is still successful.
    pub process_id: Option<u32>,
    /// Title hint used by the preview. Production titles come from the tracker.
    pub title: String,
}

/// Window and application actions. The Windows implementation will map
/// `WindowId` to an HWND internally.
pub trait WindowServices {
    fn launch(&mut self, app: &AppDefinition) -> Result<LaunchReceipt, PlatformError>;
    fn activate(&mut self, window: WindowId) -> Result<(), PlatformError>;
    fn minimize(&mut self, window: WindowId) -> Result<(), PlatformError>;
    fn close(&mut self, window: WindowId) -> Result<(), PlatformError>;
}
