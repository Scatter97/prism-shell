//! Mapping from portable domain state to Slint view data.
//!
//! These mappings are deterministic and contain no platform calls, which makes
//! them straightforward to test and reuse in both the preview and the future
//! Windows controller.

use prism_core::{
    AppDefinition, AppEntry, AudioState, Capability, ControlPhase, DockState, IconKey,
    SystemSnapshot, ToggleId, WindowId,
};

use crate::{AppItem, AudioOutputItem, ToggleItem, WindowItem};

pub fn app_item(entry: &AppEntry, active_app: Option<&prism_core::AppId>, slot: usize) -> AppItem {
    AppItem {
        id: entry.definition.id.as_str().into(),
        name: entry.definition.name.as_str().into(),
        icon_kind: icon_kind(entry.definition.icon).into(),
        running: entry.is_running(),
        active: active_app.is_some_and(|id| id == &entry.definition.id),
        window_count: i32::try_from(entry.window_count()).unwrap_or(i32::MAX),
        pinned: entry.definition.is_pinned(),
        slot: i32::try_from(slot).unwrap_or(i32::MAX),
        has_icon: false,
        icon: slint::Image::default(),
    }
}

pub fn app_items(dock: &DockState) -> Vec<AppItem> {
    let active = dock.active_app();
    dock.apps()
        .iter()
        .enumerate()
        .map(|(slot, entry)| app_item(entry, active, slot))
        .collect()
}

pub fn launcher_items(dock: &DockState, query: &str) -> Vec<AppItem> {
    let active = dock.active_app();
    let query = query.trim().to_lowercase();
    dock.apps()
        .iter()
        .filter(|entry| {
            query.is_empty()
                || entry.definition.name.to_lowercase().contains(&query)
                || entry.definition.id.as_str().to_lowercase().contains(&query)
        })
        .enumerate()
        .map(|(slot, entry)| app_item(entry, active, slot))
        .collect()
}

pub fn window_items(dock: &DockState, app_id: &str) -> Vec<WindowItem> {
    let app_id = prism_core::AppId::new(app_id);
    let Some(entry) = dock.app(&app_id) else {
        return Vec::new();
    };
    entry
        .windows
        .iter()
        .map(|window| WindowItem {
            id: window_id_to_i32(window.id),
            title: window.title.as_str().into(),
            app_name: entry.definition.name.as_str().into(),
            icon_kind: icon_kind(entry.definition.icon).into(),
            minimized: window.minimized,
        })
        .collect()
}

pub fn quick_toggle_items(snapshot: &SystemSnapshot) -> Vec<ToggleItem> {
    ToggleId::ALL
        .into_iter()
        .map(|id| {
            let toggle = snapshot.toggle(id);
            let (label, detail) = toggle_copy(id, snapshot);
            let phase = toggle
                .map(|toggle| toggle.phase.clone())
                .unwrap_or_default();
            ToggleItem {
                id: id.as_str().into(),
                label: label.into(),
                detail: detail.into(),
                icon_kind: match id {
                    ToggleId::Wifi => "wifi",
                    ToggleId::Bluetooth => "bluetooth",
                    ToggleId::Focus => "focus",
                    ToggleId::NightLight => "night-light",
                }
                .into(),
                active: toggle.is_some_and(|toggle| toggle.active),
                capability: capability_str(
                    toggle
                        .map(|toggle| toggle.capability)
                        .unwrap_or(Capability::Unavailable),
                )
                .into(),
                phase: phase_str(&phase).into(),
                error: phase.failure().unwrap_or_default().into(),
            }
        })
        .collect()
}

pub fn audio_output_items(audio: &AudioState) -> Vec<AudioOutputItem> {
    audio
        .outputs
        .iter()
        .map(|output| AudioOutputItem {
            id: output.id.as_str().into(),
            name: output.name.as_str().into(),
            is_default: audio.default_output.as_deref() == Some(output.id.as_str()),
        })
        .collect()
}

pub fn definition_icon_kind(definition: &AppDefinition) -> &'static str {
    icon_kind(definition.icon)
}

pub fn icon_kind(icon: IconKey) -> &'static str {
    match icon {
        IconKey::Files => "files",
        IconKey::Browser => "browser",
        IconKey::Terminal => "terminal",
        IconKey::Mail => "mail",
        IconKey::Music => "music",
        IconKey::Notes => "notes",
        IconKey::Generic => "generic",
    }
}

pub fn capability_str(capability: Capability) -> &'static str {
    match capability {
        Capability::Available => "available",
        Capability::Unsupported => "unsupported",
        Capability::PermissionRequired => "permission",
        Capability::Unavailable => "unavailable",
    }
}

pub fn phase_str(phase: &ControlPhase) -> &'static str {
    match phase {
        ControlPhase::Idle => "idle",
        ControlPhase::Pending => "pending",
        ControlPhase::Failed { .. } => "failed",
    }
}

/// Slint window ids are 32-bit integers. Real HWNDs on 64-bit Windows must be
/// tracked in the Windows controller; the UI gets a stable compact handle.
pub fn window_id_to_i32(id: WindowId) -> i32 {
    i32::try_from(id.get()).unwrap_or(i32::MAX)
}

pub fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes}:{seconds:02}")
}

fn toggle_copy(id: ToggleId, snapshot: &SystemSnapshot) -> (&'static str, String) {
    match id {
        ToggleId::Wifi => {
            let wifi = &snapshot.connectivity.wifi;
            let detail = if !wifi.capability.can_attempt() {
                match wifi.capability {
                    Capability::PermissionRequired => "Permission required".to_owned(),
                    Capability::Unsupported => "Windows settings".to_owned(),
                    _ => "No wireless adapter".to_owned(),
                }
            } else if wifi.enabled && wifi.connected {
                wifi.ssid
                    .clone()
                    .map(|ssid| format!("{ssid} · Connected"))
                    .unwrap_or_else(|| "Connected".to_owned())
            } else if wifi.enabled {
                "On · Not connected".to_owned()
            } else {
                "Off".to_owned()
            };
            ("Wi-Fi", detail)
        }
        ToggleId::Bluetooth => {
            let bluetooth = &snapshot.connectivity.bluetooth;
            let detail = if !bluetooth.capability.can_attempt() {
                match bluetooth.capability {
                    Capability::PermissionRequired => "Permission required".to_owned(),
                    Capability::Unsupported => "Windows settings".to_owned(),
                    _ => "No Bluetooth radio".to_owned(),
                }
            } else if bluetooth.enabled {
                bluetooth
                    .connected_device
                    .clone()
                    .unwrap_or_else(|| "On".to_owned())
            } else {
                "Off".to_owned()
            };
            ("Bluetooth", detail)
        }
        ToggleId::Focus => {
            let toggle = snapshot.toggle(ToggleId::Focus);
            if !toggle.is_some_and(|toggle| toggle.capability.can_attempt()) {
                ("Focus", "Windows settings".to_owned())
            } else {
                (
                    "Focus",
                    if toggle.is_some_and(|toggle| toggle.active) {
                        "Notifications silenced".to_owned()
                    } else {
                        "Notifications allowed".to_owned()
                    },
                )
            }
        }
        ToggleId::NightLight => ("Night light", "Windows settings".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::{
        AudioOutput, BatteryState, BluetoothState, ConnectivityState, MediaState, ToggleState,
        WifiState,
    };

    fn snapshot() -> SystemSnapshot {
        SystemSnapshot {
            toggles: vec![
                ToggleState {
                    id: ToggleId::Wifi,
                    active: true,
                    capability: Capability::Available,
                    phase: ControlPhase::Idle,
                },
                ToggleState {
                    id: ToggleId::Bluetooth,
                    active: false,
                    capability: Capability::PermissionRequired,
                    phase: ControlPhase::Idle,
                },
                ToggleState {
                    id: ToggleId::Focus,
                    active: true,
                    capability: Capability::Available,
                    phase: ControlPhase::Idle,
                },
                ToggleState {
                    id: ToggleId::NightLight,
                    active: false,
                    capability: Capability::Unsupported,
                    phase: ControlPhase::Idle,
                },
            ],
            audio: AudioState {
                outputs: vec![AudioOutput {
                    id: "usb".into(),
                    name: "Studio Monitors".into(),
                }],
                default_output: Some("usb".into()),
                volume: 68,
                muted: false,
                capability: Capability::Available,
                phase: ControlPhase::Idle,
            },
            brightness: prism_core::BrightnessState {
                level: 62,
                capability: Capability::Available,
                phase: ControlPhase::Idle,
                detail: None,
            },
            connectivity: ConnectivityState {
                wifi: WifiState {
                    enabled: true,
                    connected: true,
                    ssid: Some("Prism-5G".into()),
                    capability: Capability::Available,
                    phase: ControlPhase::Idle,
                },
                bluetooth: BluetoothState {
                    enabled: false,
                    connected_device: None,
                    capability: Capability::PermissionRequired,
                    phase: ControlPhase::Idle,
                },
                wired: false,
            },
            battery: BatteryState::desktop(),
            media: MediaState::empty(),
        }
    }

    #[test]
    fn quick_toggle_copy_is_honest_about_permission_and_unsupported_states() {
        let items = quick_toggle_items(&snapshot());
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].detail.as_str(), "Prism-5G · Connected");
        assert_eq!(items[1].capability.as_str(), "permission");
        assert_eq!(items[2].detail.as_str(), "Notifications silenced");
        assert_eq!(items[3].capability.as_str(), "unsupported");
    }

    #[test]
    fn audio_outputs_mark_the_default() {
        let items = audio_output_items(&snapshot().audio);
        assert_eq!(items.len(), 1);
        assert!(items[0].is_default);
    }

    #[test]
    fn duration_formatting_is_stable() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(236), "3:56");
    }

    #[test]
    fn launcher_filter_matches_name_and_id() {
        let dock = DockState::new(vec![
            AppDefinition::pinned("files", "Files", IconKey::Files, 0),
            AppDefinition::pinned("browser", "Browser", IconKey::Browser, 1),
        ]);
        assert_eq!(launcher_items(&dock, "fil").len(), 1);
        assert_eq!(launcher_items(&dock, "brow").len(), 1);
        assert_eq!(launcher_items(&dock, "zzz").len(), 0);
    }
}
