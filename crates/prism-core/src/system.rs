use std::fmt;

/// Whether a system capability can be exercised on this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// The control is backed by a supported API and the device is present.
    Available,
    /// Windows does not expose a supported API for this operation.
    Unsupported,
    /// The operation requires a user consent or privacy permission.
    PermissionRequired,
    /// The API exists but the relevant hardware/service is absent or disabled.
    Unavailable,
}

impl Capability {
    pub const fn can_attempt(self) -> bool {
        matches!(self, Self::Available)
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Available => "available",
            Self::Unsupported => "unsupported",
            Self::PermissionRequired => "permission required",
            Self::Unavailable => "unavailable",
        })
    }
}

/// Runtime state of one operation. This is intentionally separate from
/// `Capability`: a supported control can still be mid-flight or have failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ControlPhase {
    #[default]
    Idle,
    Pending,
    Failed {
        message: String,
    },
}

impl ControlPhase {
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn failure(&self) -> Option<&str> {
        match self {
            Self::Failed { message } => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleId {
    Wifi,
    Bluetooth,
    Focus,
    NightLight,
}

impl ToggleId {
    pub const ALL: [Self; 4] = [Self::Wifi, Self::Bluetooth, Self::Focus, Self::NightLight];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wifi => "wifi",
            Self::Bluetooth => "bluetooth",
            Self::Focus => "focus",
            Self::NightLight => "night-light",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleState {
    pub id: ToggleId,
    pub active: bool,
    pub capability: Capability,
    pub phase: ControlPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioOutput {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioState {
    pub outputs: Vec<AudioOutput>,
    pub default_output: Option<String>,
    /// 0..=100.
    pub volume: u8,
    pub muted: bool,
    pub capability: Capability,
    pub phase: ControlPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrightnessState {
    /// 0..=100.
    pub level: u8,
    pub capability: Capability,
    pub phase: ControlPhase,
    /// Short user-facing explanation for unavailable/hardware-dependent state.
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiState {
    pub enabled: bool,
    pub connected: bool,
    pub ssid: Option<String>,
    pub capability: Capability,
    pub phase: ControlPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BluetoothState {
    pub enabled: bool,
    pub connected_device: Option<String>,
    pub capability: Capability,
    pub phase: ControlPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectivityState {
    pub wifi: WifiState,
    pub bluetooth: BluetoothState,
    pub wired: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatteryState {
    pub present: bool,
    pub percent: u8,
    pub charging: bool,
    pub capability: Capability,
}

impl BatteryState {
    pub const fn desktop() -> Self {
        Self {
            present: false,
            percent: 0,
            charging: false,
            capability: Capability::Unavailable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaSession {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_seconds: u32,
    pub duration_seconds: u32,
    /// Stable token used by the presentation layer to choose a generated
    /// placeholder cover until real artwork is available.
    pub artwork_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaState {
    pub session: Option<MediaSession>,
    pub capability: Capability,
    pub phase: ControlPhase,
}

impl MediaState {
    pub const fn empty() -> Self {
        Self {
            session: None,
            capability: Capability::Unavailable,
            phase: ControlPhase::Idle,
        }
    }
}

/// Complete snapshot of the system values shown by the dock and Control Center.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemSnapshot {
    pub toggles: Vec<ToggleState>,
    pub audio: AudioState,
    pub brightness: BrightnessState,
    pub connectivity: ConnectivityState,
    pub battery: BatteryState,
    pub media: MediaState,
}

impl SystemSnapshot {
    pub fn toggle(&self, id: ToggleId) -> Option<&ToggleState> {
        self.toggles.iter().find(|toggle| toggle.id == id)
    }
}

/// System-side commands. The UI never invokes an OS API directly; it emits
/// these semantic actions and reconciles with the next snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemAction {
    SetToggle { id: ToggleId, active: bool },
    SetVolume(u8),
    ToggleMute,
    SelectAudioOutput(String),
    SetBrightness(u8),
    MediaPlayPause,
    MediaNext,
    MediaPrevious,
    OpenSettings(SettingsPage),
    Power(PowerAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPage {
    Home,
    Sound,
    Network,
    Bluetooth,
    Display,
    Power,
}

impl SettingsPage {
    /// Windows Settings URIs. These are documented shell entry points and are
    /// only invoked when the Windows backend runs on Windows.
    pub const fn windows_uri(self) -> &'static str {
        match self {
            Self::Home => "ms-settings:",
            Self::Sound => "ms-settings:sound",
            Self::Network => "ms-settings:network",
            Self::Bluetooth => "ms-settings:bluetooth",
            Self::Display => "ms-settings:display",
            Self::Power => "ms-settings:powersleep",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    Sleep,
    Restart,
    ShutDown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_is_not_the_same_as_phase() {
        assert!(Capability::Available.can_attempt());
        assert!(!Capability::Unsupported.can_attempt());
        assert!(!Capability::PermissionRequired.can_attempt());
        assert!(ControlPhase::Pending.is_pending());
        assert_eq!(
            ControlPhase::failed("device disappeared").failure(),
            Some("device disappeared")
        );
    }

    #[test]
    fn settings_uris_are_stable() {
        assert_eq!(SettingsPage::Sound.windows_uri(), "ms-settings:sound");
        assert_eq!(
            SettingsPage::Bluetooth.windows_uri(),
            "ms-settings:bluetooth"
        );
    }

    #[test]
    fn toggle_lookup_returns_semantic_state() {
        let snapshot = SystemSnapshot {
            toggles: vec![ToggleState {
                id: ToggleId::Wifi,
                active: true,
                capability: Capability::Available,
                phase: ControlPhase::Idle,
            }],
            audio: AudioState {
                outputs: Vec::new(),
                default_output: None,
                volume: 0,
                muted: true,
                capability: Capability::Unavailable,
                phase: ControlPhase::Idle,
            },
            brightness: BrightnessState {
                level: 0,
                capability: Capability::Unsupported,
                phase: ControlPhase::Idle,
                detail: None,
            },
            connectivity: ConnectivityState {
                wifi: WifiState {
                    enabled: false,
                    connected: false,
                    ssid: None,
                    capability: Capability::Unavailable,
                    phase: ControlPhase::Idle,
                },
                bluetooth: BluetoothState {
                    enabled: false,
                    connected_device: None,
                    capability: Capability::Unavailable,
                    phase: ControlPhase::Idle,
                },
                wired: false,
            },
            battery: BatteryState::desktop(),
            media: MediaState::empty(),
        };
        assert!(snapshot.toggle(ToggleId::Wifi).unwrap().active);
        assert!(snapshot.toggle(ToggleId::Focus).is_none());
    }
}
