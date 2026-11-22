//! Deterministic, explicit Linux development backend.
//!
//! Nothing in this module touches Linux system APIs. It exists to exercise the
//! real UI state machine with realistic states and to support screenshots and
//! repeatable interaction tests. Every simulated action is labelled as such.

use std::collections::VecDeque;

use prism_core::{
    AppDefinition, AudioOutput, AudioState, BatteryState, BluetoothState, BrightnessState,
    Capability, ConnectivityState, ControlPhase, MediaSession, MediaState, SystemAction,
    SystemSnapshot, ToggleId, ToggleState, WifiState, WindowId,
};

use crate::traits::{
    ActionOutcome, LaunchReceipt, PlatformError, PlatformEvent, SystemServices, WindowServices,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewScenario {
    Laptop,
    Desktop,
    Minimal,
    Faults,
    ManyApps,
}

impl PreviewScenario {
    pub const ALL: [Self; 5] = [
        Self::Laptop,
        Self::Desktop,
        Self::Minimal,
        Self::Faults,
        Self::ManyApps,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Laptop => "laptop",
            Self::Desktop => "desktop",
            Self::Minimal => "minimal",
            Self::Faults => "faults",
            Self::ManyApps => "many",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Laptop => "Laptop",
            Self::Desktop => "Desktop",
            Self::Minimal => "Minimal",
            Self::Faults => "Faults",
            Self::ManyApps => "Many apps",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scenario| scenario.id() == id)
    }
}

#[derive(Clone, Debug)]
struct MockState {
    focus: bool,
    night_light: bool,
    wifi: WifiState,
    bluetooth: BluetoothState,
    wired: bool,
    audio: AudioState,
    brightness: BrightnessState,
    battery: BatteryState,
    media: MediaState,
    media_index: usize,
}

#[derive(Clone, Copy, Debug)]
struct PendingToggle {
    id: ToggleId,
    target: bool,
}

pub struct MockPlatform {
    scenario: PreviewScenario,
    state: MockState,
    pending: Option<PendingToggle>,
    events: VecDeque<PlatformEvent>,
    media_started: bool,
}

impl MockPlatform {
    pub fn new(scenario: PreviewScenario) -> Self {
        let mut platform = Self {
            scenario,
            state: state_for(scenario),
            pending: None,
            events: VecDeque::new(),
            media_started: false,
        };
        platform.events.push_back(PlatformEvent::Notice(format!(
            "Linux preview · {} fixture loaded · no system changes",
            scenario.label()
        )));
        platform
    }

    pub fn scenario(&self) -> PreviewScenario {
        self.scenario
    }

    pub fn set_scenario(&mut self, scenario: PreviewScenario) {
        self.scenario = scenario;
        self.state = state_for(scenario);
        self.pending = None;
        self.media_started = false;
        self.events.push_back(PlatformEvent::Notice(format!(
            "Preview fixture: {} · mock data only",
            scenario.label()
        )));
    }

    /// Complete a mocked asynchronous toggle. Returns the resulting event.
    pub fn finish_pending(&mut self) -> Option<PlatformEvent> {
        let pending = self.pending.take()?;
        self.set_toggle_phase(pending.id, ControlPhase::Idle);

        if self.scenario == PreviewScenario::Faults && pending.id == ToggleId::Bluetooth {
            self.state.bluetooth.phase = ControlPhase::failed("Radio rejected the change.");
            return Some(PlatformEvent::Error(
                "Bluetooth change failed · preview fixture".to_owned(),
            ));
        }

        self.apply_toggle(pending.id, pending.target);
        Some(PlatformEvent::Notice(format!(
            "{} {} · preview only",
            toggle_name(pending.id),
            if pending.target { "on" } else { "off" }
        )))
    }

    pub fn toggle_is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Advance mock media playback. The caller supplies the cadence.
    pub fn tick_media(&mut self) {
        if !self.media_started {
            return;
        }
        let Some(session) = self.state.media.session.as_mut() else {
            return;
        };
        if !session.playing {
            return;
        }
        session.position_seconds = session.position_seconds.saturating_add(1);
        let finished = session.position_seconds >= session.duration_seconds;
        if finished {
            self.next_track();
        }
    }

    pub fn simulate_display_change(&mut self) {
        self.events.push_back(PlatformEvent::Notice(
            "Simulated monitor change · taskbar geometry would be recomputed".to_owned(),
        ));
    }

    pub fn simulate_audio_loss(&mut self) {
        self.state.audio.outputs.clear();
        self.state.audio.default_output = None;
        self.state.audio.capability = Capability::Unavailable;
        self.state.audio.phase = ControlPhase::failed("Audio endpoint lost.");
        self.events.push_back(PlatformEvent::Error(
            "Audio endpoint lost · preview fixture".to_owned(),
        ));
    }

    fn next_track(&mut self) {
        self.state.media_index = (self.state.media_index + 1) % playlist().len();
        let track = playlist()[self.state.media_index];
        if let Some(session) = self.state.media.session.as_mut() {
            session.title = track.0.to_owned();
            session.artist = track.1.to_owned();
            session.album = track.2.to_owned();
            session.duration_seconds = track.3;
            session.position_seconds = 0;
            session.playing = true;
            session.artwork_key = format!("prism-cover-{}", self.state.media_index + 1);
        }
    }

    fn set_toggle_phase(&mut self, id: ToggleId, phase: ControlPhase) {
        match id {
            ToggleId::Wifi => self.state.wifi.phase = phase,
            ToggleId::Bluetooth => self.state.bluetooth.phase = phase,
            ToggleId::Focus | ToggleId::NightLight => {}
        }
    }

    fn apply_toggle(&mut self, id: ToggleId, target: bool) {
        match id {
            ToggleId::Wifi => {
                self.state.wifi.enabled = target;
                self.state.wifi.connected = target;
                self.state.wifi.ssid = target.then(|| "Prism-5G".to_owned());
                self.state.wifi.phase = ControlPhase::Idle;
            }
            ToggleId::Bluetooth => {
                self.state.bluetooth.enabled = target;
                self.state.bluetooth.connected_device = target.then(|| "MX Master 3S".to_owned());
                self.state.bluetooth.phase = ControlPhase::Idle;
            }
            ToggleId::Focus => self.state.focus = target,
            ToggleId::NightLight => self.state.night_light = target,
        }
    }

    fn toggle_capability(&self, id: ToggleId) -> Capability {
        match id {
            ToggleId::Wifi => self.state.wifi.capability,
            ToggleId::Bluetooth => self.state.bluetooth.capability,
            ToggleId::Focus => Capability::Available,
            ToggleId::NightLight => Capability::Unsupported,
        }
    }

    fn toggle_active(&self, id: ToggleId) -> bool {
        match id {
            ToggleId::Wifi => self.state.wifi.enabled,
            ToggleId::Bluetooth => self.state.bluetooth.enabled,
            ToggleId::Focus => self.state.focus,
            ToggleId::NightLight => self.state.night_light,
        }
    }

    fn toggle_phase(&self, id: ToggleId) -> ControlPhase {
        match id {
            ToggleId::Wifi => self.state.wifi.phase.clone(),
            ToggleId::Bluetooth => self.state.bluetooth.phase.clone(),
            ToggleId::Focus | ToggleId::NightLight => ControlPhase::Idle,
        }
    }

    fn media_action(&mut self, action: SystemAction) -> Result<ActionOutcome, PlatformError> {
        if self.state.media.capability != Capability::Available {
            return Err(PlatformError::unavailable(
                "Media controls",
                "No compatible media session is active.",
            ));
        }
        if self.state.media.session.is_none() {
            return Err(PlatformError::unavailable(
                "Media controls",
                "No compatible media session is active.",
            ));
        }
        match action {
            SystemAction::MediaPlayPause => {
                if let Some(session) = self.state.media.session.as_mut() {
                    session.playing = !session.playing;
                    self.media_started = session.playing;
                }
            }
            SystemAction::MediaNext => {
                self.next_track();
                self.media_started = true;
            }
            SystemAction::MediaPrevious => {
                if let Some(session) = self.state.media.session.as_mut() {
                    if session.position_seconds > 4 {
                        session.position_seconds = 0;
                    } else {
                        self.state.media_index = self
                            .state
                            .media_index
                            .checked_sub(1)
                            .unwrap_or(playlist().len() - 1);
                        let track = playlist()[self.state.media_index];
                        session.title = track.0.to_owned();
                        session.artist = track.1.to_owned();
                        session.album = track.2.to_owned();
                        session.duration_seconds = track.3;
                        session.position_seconds = 0;
                    }
                    session.playing = true;
                    self.media_started = true;
                }
            }
            _ => {}
        }
        Ok(ActionOutcome::Applied)
    }
}

impl SystemServices for MockPlatform {
    fn snapshot(&self) -> SystemSnapshot {
        let toggles = ToggleId::ALL
            .into_iter()
            .map(|id| ToggleState {
                id,
                active: self.toggle_active(id),
                capability: self.toggle_capability(id),
                phase: self.toggle_phase(id),
            })
            .collect();

        SystemSnapshot {
            toggles,
            audio: self.state.audio.clone(),
            brightness: self.state.brightness.clone(),
            connectivity: ConnectivityState {
                wifi: self.state.wifi.clone(),
                bluetooth: self.state.bluetooth.clone(),
                wired: self.state.wired,
            },
            battery: self.state.battery.clone(),
            media: self.state.media.clone(),
        }
    }

    fn execute(&mut self, action: SystemAction) -> Result<ActionOutcome, PlatformError> {
        match action {
            SystemAction::SetToggle { id, active } => {
                if !self.toggle_capability(id).can_attempt() {
                    return match self.toggle_capability(id) {
                        Capability::PermissionRequired => {
                            Err(PlatformError::permission_required(toggle_name(id)))
                        }
                        Capability::Unsupported => Err(PlatformError::unsupported(
                            toggle_name(id),
                            "Windows does not expose a supported toggle for this control.",
                        )),
                        _ => Err(PlatformError::unavailable(
                            toggle_name(id),
                            "The required hardware or service is not present.",
                        )),
                    };
                }
                if self.pending.is_some() {
                    return Err(PlatformError::failed(
                        toggle_name(id),
                        "Another device change is still in progress.",
                    ));
                }
                if id == ToggleId::Focus {
                    self.apply_toggle(id, active);
                    return Ok(ActionOutcome::Applied);
                }
                self.pending = Some(PendingToggle { id, target: active });
                self.set_toggle_phase(id, ControlPhase::Pending);
                Ok(ActionOutcome::Pending)
            }
            SystemAction::SetVolume(volume) => {
                if self.state.audio.capability != Capability::Available
                    || self.state.audio.default_output.is_none()
                {
                    return Err(PlatformError::unavailable(
                        "Volume control",
                        "No audio output endpoint is available.",
                    ));
                }
                self.state.audio.volume = volume.min(100);
                if volume > 0 {
                    self.state.audio.muted = false;
                }
                Ok(ActionOutcome::Applied)
            }
            SystemAction::ToggleMute => {
                if self.state.audio.capability != Capability::Available
                    || self.state.audio.default_output.is_none()
                {
                    return Err(PlatformError::unavailable(
                        "Mute",
                        "No audio output endpoint is available.",
                    ));
                }
                self.state.audio.muted = !self.state.audio.muted;
                Ok(ActionOutcome::Applied)
            }
            SystemAction::SelectAudioOutput(id) => {
                if !self
                    .state
                    .audio
                    .outputs
                    .iter()
                    .any(|output| output.id == id)
                {
                    return Err(PlatformError::unavailable(
                        "Audio output",
                        "The selected endpoint is no longer present.",
                    ));
                }
                self.state.audio.default_output = Some(id);
                Ok(ActionOutcome::Applied)
            }
            SystemAction::SetBrightness(level) => {
                if !self.state.brightness.capability.can_attempt() {
                    return Err(match self.state.brightness.capability {
                        Capability::Unsupported => PlatformError::unsupported(
                            "Brightness",
                            self.state
                                .brightness
                                .detail
                                .as_deref()
                                .unwrap_or("No brightness control on this display."),
                        ),
                        _ => PlatformError::unavailable("Brightness", "No brightness control."),
                    });
                }
                if self.scenario == PreviewScenario::Faults && level > 85 {
                    self.state.brightness.phase =
                        ControlPhase::failed("The display driver rejected the brightness change.");
                    return Err(PlatformError::failed(
                        "Brightness",
                        "The display driver rejected the brightness change.",
                    ));
                }
                self.state.brightness.level = level.min(100);
                self.state.brightness.phase = ControlPhase::Idle;
                Ok(ActionOutcome::Applied)
            }
            SystemAction::MediaPlayPause
            | SystemAction::MediaNext
            | SystemAction::MediaPrevious => self.media_action(action),
            SystemAction::OpenSettings(page) => Ok(ActionOutcome::Simulated(format!(
                "Preview only: Windows Settings would open ({})",
                page.windows_uri()
            ))),
            SystemAction::Power(action) => Ok(ActionOutcome::Simulated(format!(
                "Preview only: {} — no action taken",
                power_label(action)
            ))),
        }
    }

    fn poll(&mut self) -> Vec<PlatformEvent> {
        self.events.drain(..).collect()
    }
}

impl WindowServices for MockPlatform {
    fn launch(&mut self, app: &AppDefinition) -> Result<LaunchReceipt, PlatformError> {
        Ok(LaunchReceipt {
            process_id: None,
            title: format!("{} · New Window", app.name),
        })
    }

    fn activate(&mut self, _window: WindowId) -> Result<(), PlatformError> {
        Ok(())
    }

    fn minimize(&mut self, _window: WindowId) -> Result<(), PlatformError> {
        Ok(())
    }

    fn close(&mut self, _window: WindowId) -> Result<(), PlatformError> {
        Ok(())
    }
}

fn toggle_name(id: ToggleId) -> &'static str {
    match id {
        ToggleId::Wifi => "Wi-Fi",
        ToggleId::Bluetooth => "Bluetooth",
        ToggleId::Focus => "Focus",
        ToggleId::NightLight => "Night light",
    }
}

fn power_label(action: prism_core::PowerAction) -> &'static str {
    match action {
        prism_core::PowerAction::Lock => "Lock",
        prism_core::PowerAction::Sleep => "Sleep",
        prism_core::PowerAction::Restart => "Restart",
        prism_core::PowerAction::ShutDown => "Shut down",
    }
}

fn playlist() -> &'static [(&'static str, &'static str, &'static str, u32)] {
    &[
        ("Aurora Drift", "Lumen Fields", "Night Signals", 236),
        ("Glass Horizon", "Lumen Fields", "Night Signals", 198),
        ("Soft Static", "Mira Vale", "Low Orbit", 254),
    ]
}

fn audio_outputs() -> Vec<AudioOutput> {
    vec![
        AudioOutput {
            id: "usb-monitors".to_owned(),
            name: "Studio Monitors · USB".to_owned(),
        },
        AudioOutput {
            id: "headphones".to_owned(),
            name: "Headphones · Realtek".to_owned(),
        },
        AudioOutput {
            id: "hdmi".to_owned(),
            name: "Display Audio · HDMI".to_owned(),
        },
    ]
}

fn state_for(scenario: PreviewScenario) -> MockState {
    let active_media = || {
        let track = playlist()[0];
        MediaState {
            session: Some(MediaSession {
                title: track.0.to_owned(),
                artist: track.1.to_owned(),
                album: track.2.to_owned(),
                playing: true,
                position_seconds: 102,
                duration_seconds: track.3,
                artwork_key: "prism-cover-1".to_owned(),
            }),
            capability: Capability::Available,
            phase: ControlPhase::Idle,
        }
    };

    let laptop_audio = AudioState {
        outputs: audio_outputs(),
        default_output: Some("usb-monitors".to_owned()),
        volume: 68,
        muted: false,
        capability: Capability::Available,
        phase: ControlPhase::Idle,
    };

    let laptop_brightness = BrightnessState {
        level: 62,
        capability: Capability::Available,
        phase: ControlPhase::Idle,
        detail: None,
    };

    let mut state = MockState {
        focus: false,
        night_light: false,
        wifi: WifiState {
            enabled: true,
            connected: true,
            ssid: Some("Prism-5G".to_owned()),
            capability: Capability::Available,
            phase: ControlPhase::Idle,
        },
        bluetooth: BluetoothState {
            enabled: true,
            connected_device: Some("MX Master 3S".to_owned()),
            capability: Capability::Available,
            phase: ControlPhase::Idle,
        },
        wired: false,
        audio: laptop_audio,
        brightness: laptop_brightness,
        battery: BatteryState {
            present: true,
            percent: 76,
            charging: true,
            capability: Capability::Available,
        },
        media: active_media(),
        media_index: 0,
    };

    match scenario {
        PreviewScenario::Laptop | PreviewScenario::ManyApps => {}
        PreviewScenario::Desktop => {
            state.wired = true;
            state.wifi = WifiState {
                enabled: false,
                connected: false,
                ssid: None,
                capability: Capability::Available,
                phase: ControlPhase::Idle,
            };
            state.bluetooth.enabled = false;
            state.bluetooth.connected_device = None;
            state.battery = BatteryState::desktop();
            state.audio.volume = 42;
            state.brightness = BrightnessState {
                level: 0,
                capability: Capability::Unsupported,
                phase: ControlPhase::Idle,
                detail: Some("No brightness control.".to_owned()),
            };
            state.media = MediaState {
                session: None,
                capability: Capability::Available,
                phase: ControlPhase::Idle,
            };
        }
        PreviewScenario::Minimal => {
            state.wifi = WifiState {
                enabled: false,
                connected: false,
                ssid: None,
                capability: Capability::Unavailable,
                phase: ControlPhase::Idle,
            };
            state.bluetooth = BluetoothState {
                enabled: false,
                connected_device: None,
                capability: Capability::PermissionRequired,
                phase: ControlPhase::Idle,
            };
            state.battery = BatteryState::desktop();
            state.audio = AudioState {
                outputs: Vec::new(),
                default_output: None,
                volume: 0,
                muted: true,
                capability: Capability::Unavailable,
                phase: ControlPhase::Idle,
            };
            state.brightness = BrightnessState {
                level: 0,
                capability: Capability::Unsupported,
                phase: ControlPhase::Idle,
                detail: Some("No brightness control.".to_owned()),
            };
            state.media = MediaState::empty();
        }
        PreviewScenario::Faults => {
            state.bluetooth.phase = ControlPhase::Idle;
            state.audio = AudioState {
                outputs: Vec::new(),
                default_output: None,
                volume: 0,
                muted: true,
                capability: Capability::Unavailable,
                phase: ControlPhase::failed("Audio endpoint lost."),
            };
            state.media = MediaState {
                session: None,
                capability: Capability::Available,
                phase: ControlPhase::failed("The media session ended unexpectedly."),
            };
            state.brightness.phase =
                ControlPhase::failed("The display driver reported a transient failure.");
        }
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(scenario: PreviewScenario) -> MockPlatform {
        MockPlatform::new(scenario)
    }

    #[test]
    fn every_scenario_has_a_consistent_snapshot() {
        for scenario in PreviewScenario::ALL {
            let platform = platform(scenario);
            let snapshot = platform.snapshot();
            assert_eq!(snapshot.toggles.len(), 4, "{scenario:?}");
            if snapshot.battery.present {
                assert!(snapshot.battery.percent <= 100);
            }
            if let Some(session) = &snapshot.media.session {
                assert!(session.position_seconds <= session.duration_seconds);
            }
        }
    }

    #[test]
    fn wifi_toggle_is_pending_then_reconciled() {
        let mut platform = platform(PreviewScenario::Laptop);
        let outcome = platform
            .execute(SystemAction::SetToggle {
                id: ToggleId::Wifi,
                active: false,
            })
            .unwrap();
        assert_eq!(outcome, ActionOutcome::Pending);
        assert_eq!(
            platform.snapshot().toggle(ToggleId::Wifi).unwrap().phase,
            ControlPhase::Pending
        );
        platform.finish_pending().unwrap();
        let snapshot = platform.snapshot();
        assert!(!snapshot.connectivity.wifi.enabled);
        assert!(!snapshot.connectivity.wifi.connected);
        assert_eq!(
            snapshot.toggle(ToggleId::Wifi).unwrap().phase,
            ControlPhase::Idle
        );
    }

    #[test]
    fn unsupported_toggle_reports_unsupported_without_mutating_state() {
        let mut platform = platform(PreviewScenario::Laptop);
        let error = platform
            .execute(SystemAction::SetToggle {
                id: ToggleId::NightLight,
                active: true,
            })
            .unwrap_err();
        assert!(matches!(error, PlatformError::Unsupported { .. }));
        assert!(
            !platform
                .snapshot()
                .toggle(ToggleId::NightLight)
                .unwrap()
                .active
        );
    }

    #[test]
    fn permission_required_toggle_is_distinct_from_unsupported() {
        let mut platform = platform(PreviewScenario::Minimal);
        let error = platform
            .execute(SystemAction::SetToggle {
                id: ToggleId::Bluetooth,
                active: true,
            })
            .unwrap_err();
        assert!(matches!(error, PlatformError::PermissionRequired { .. }));
    }

    #[test]
    fn fault_scenario_reconciles_bluetooth_failure() {
        let mut platform = platform(PreviewScenario::Faults);
        platform
            .execute(SystemAction::SetToggle {
                id: ToggleId::Bluetooth,
                active: false,
            })
            .unwrap();
        let event = platform.finish_pending().unwrap();
        assert!(matches!(event, PlatformEvent::Error(_)));
        let snapshot = platform.snapshot();
        // A failed operation must not leave an optimistic UI state behind.
        assert!(snapshot.connectivity.bluetooth.enabled);
        assert_eq!(
            snapshot.connectivity.bluetooth.phase.failure(),
            Some("Radio rejected the change.")
        );
    }

    #[test]
    fn volume_is_clamped_and_mute_is_reflected() {
        let mut platform = platform(PreviewScenario::Laptop);
        platform.execute(SystemAction::SetVolume(0)).unwrap();
        assert_eq!(platform.snapshot().audio.volume, 0);
        platform.execute(SystemAction::SetVolume(130)).unwrap();
        let audio = platform.snapshot().audio;
        assert_eq!(audio.volume, 100);
        assert!(!audio.muted);
        platform.execute(SystemAction::ToggleMute).unwrap();
        assert!(platform.snapshot().audio.muted);
    }

    #[test]
    fn media_actions_do_not_panic_without_a_session() {
        let mut platform = platform(PreviewScenario::Desktop);
        let error = platform.execute(SystemAction::MediaPlayPause).unwrap_err();
        assert!(matches!(error, PlatformError::Unavailable { .. }));
    }

    #[test]
    fn every_scenario_has_a_stable_id_round_trip() {
        for scenario in PreviewScenario::ALL {
            assert_eq!(PreviewScenario::from_id(scenario.id()), Some(scenario));
        }
        assert_eq!(PreviewScenario::from_id("unknown"), None);
    }

    #[test]
    fn settings_and_power_are_explicitly_simulated() {
        let mut platform = platform(PreviewScenario::Laptop);
        let outcome = platform
            .execute(SystemAction::OpenSettings(prism_core::SettingsPage::Sound))
            .unwrap();
        assert!(matches!(outcome, ActionOutcome::Simulated(_)));
        let outcome = platform
            .execute(SystemAction::Power(prism_core::PowerAction::ShutDown))
            .unwrap();
        assert!(matches!(outcome, ActionOutcome::Simulated(_)));
    }
}
