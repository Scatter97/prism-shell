//! Linux preview controller.
//!
//! Owns the explicit mock platform and the portable dock state, translates
//! Slint callbacks into semantic actions, and feeds authoritative snapshots
//! back into the view. No function here performs a real system operation.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use chrono::Local;
use prism_core::{
    AppId, MediaState, PowerAction, SettingsPage, SystemAction, SystemSnapshot, ToggleId, WindowId,
    format_clock,
};
use prism_platform::{
    ActionOutcome, MockPlatform, PlatformError, PlatformEvent, PreviewScenario, SystemServices,
    WindowServices,
};
use prism_ui::model::{
    app_items, audio_output_items, capability_str, format_duration, launcher_items,
    quick_toggle_items, window_id_to_i32, window_items,
};
use prism_ui::{AppItem, AppWindow, ScenarioItem, UiState, WindowItem};
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::config::PrismConfig;
use crate::preview::catalog;

pub type ControllerHandle = Rc<RefCell<PreviewController>>;

pub struct PreviewController {
    platform: MockPlatform,
    dock: prism_core::DockState,
    scenario: PreviewScenario,
    config: PrismConfig,
    stage_width: f32,
    self_weak: Weak<RefCell<PreviewController>>,

    cc_open: bool,
    launcher_open: bool,
    launcher_query: String,
    audio_picker_open: bool,
    confirm_open: bool,

    hovered_app: Option<String>,
    hover_generation: u64,
    hover_suppressed_until: Option<Instant>,
    preview_app: Option<String>,
    preview_open: bool,
    preview_hovered: bool,
    preview_close_generation: u64,

    toast_generation: u64,
    last_clock_minute: Option<(u32, u32)>,
}

impl PreviewController {
    pub fn new(
        ui: &AppWindow,
        scenario: PreviewScenario,
        config: PrismConfig,
        stage_width: f32,
    ) -> ControllerHandle {
        let reduced_motion = config.accessibility.reduced_motion;
        let controller = Rc::new(RefCell::new(Self {
            platform: MockPlatform::new(scenario),
            dock: catalog::build_dock(scenario),
            scenario,
            config,
            stage_width,
            self_weak: Weak::new(),
            cc_open: false,
            launcher_open: false,
            launcher_query: String::new(),
            audio_picker_open: false,
            confirm_open: false,
            hovered_app: None,
            hover_generation: 0,
            hover_suppressed_until: None,
            preview_app: None,
            preview_open: false,
            preview_hovered: false,
            preview_close_generation: 0,
            toast_generation: 0,
            last_clock_minute: None,
        }));
        controller.borrow_mut().self_weak = Rc::downgrade(&controller);

        let state = ui.global::<UiState>();
        state.set_development_preview(true);
        state.set_platform_label("Linux preview · mock data".into());
        state.set_version_label(env!("CARGO_PKG_VERSION").into());
        state.set_footer_label("Preview backend · no system changes".into());
        state.set_launcher_footer_label(
            "Preview launcher · the Windows Start menu is untouched".into(),
        );
        state.set_show_unsafe_power(true);
        state.set_show_audio_status(true);
        state.set_show_wifi_status(true);
        state.set_scenarios(ModelRc::new(VecModel::from(scenario_items())));
        state.set_scenario(scenario.id().into());
        state.set_reduced_motion(reduced_motion);

        // Consume the fixture-loaded notice so the first screenshot is clean.
        {
            let mut borrowed = controller.borrow_mut();
            let _ = borrowed.platform.poll();
            borrowed.apply(ui);
        }
        controller
    }

    pub fn stage_width(&self) -> f32 {
        self.stage_width
    }

    pub fn apply(&mut self, ui: &AppWindow) {
        let snapshot = self.platform.snapshot();
        let state = ui.global::<UiState>();

        let stage_width = current_stage_width(ui, self.stage_width);
        self.stage_width = stage_width;
        let all_apps = app_items(&self.dock);
        let fit = prism_core::fit_dock_apps(all_apps.len(), stage_width);
        let visible: Vec<AppItem> = all_apps.into_iter().take(fit.visible_apps).collect();
        state.set_apps(ModelRc::new(VecModel::from(visible)));
        state.set_hidden_app_count(i32::try_from(fit.hidden_apps).unwrap_or(i32::MAX));
        state.set_launcher_apps(ModelRc::new(VecModel::from(launcher_items(
            &self.dock,
            &self.launcher_query,
        ))));
        state.set_quick_toggles(ModelRc::new(VecModel::from(quick_toggle_items(&snapshot))));
        state.set_audio_outputs(ModelRc::new(VecModel::from(audio_output_items(
            &snapshot.audio,
        ))));

        let audio = &snapshot.audio;
        let default_output = audio
            .outputs
            .iter()
            .find(|output| audio.default_output.as_deref() == Some(output.id.as_str()));
        state.set_audio_device(
            default_output
                .map(|output| output.name.as_str())
                .unwrap_or("No output device")
                .into(),
        );
        state.set_audio_available(audio.capability.can_attempt() && default_output.is_some());
        state.set_audio_state_detail(
            audio
                .phase
                .failure()
                .or_else(|| (!audio.capability.can_attempt()).then_some("No audio output"))
                .unwrap_or_default()
                .into(),
        );
        state.set_volume(f32::from(audio.volume));
        state.set_muted(audio.muted);

        state.set_brightness(f32::from(snapshot.brightness.level));
        state.set_brightness_available(snapshot.brightness.capability.can_attempt());
        state.set_brightness_note(
            snapshot
                .brightness
                .phase
                .failure()
                .or(snapshot.brightness.detail.as_deref())
                .unwrap_or_default()
                .into(),
        );

        let wifi = &snapshot.connectivity.wifi;
        state.set_wifi_on(wifi.enabled);
        state.set_wifi_name(wifi.ssid.as_deref().unwrap_or("Wi-Fi").into());
        state.set_wifi_detail(if wifi.connected {
            "Connected, secured".into()
        } else if wifi.enabled {
            "On · Not connected".into()
        } else {
            "Off".into()
        });
        state.set_wifi_capability(capability_str(wifi.capability).into());

        let bluetooth = &snapshot.connectivity.bluetooth;
        state.set_bluetooth_on(bluetooth.enabled);
        state.set_bluetooth_name(
            bluetooth
                .connected_device
                .as_deref()
                .unwrap_or("Bluetooth")
                .into(),
        );
        state.set_bluetooth_detail(if bluetooth.connected_device.is_some() {
            "Connected".into()
        } else if bluetooth.enabled {
            "On".into()
        } else {
            "Off".into()
        });
        state.set_bluetooth_capability(capability_str(bluetooth.capability).into());
        state.set_wired(snapshot.connectivity.wired);

        apply_battery(&state, &snapshot);
        self.apply_media(&state, &snapshot.media);
        apply_clock(&mut self.last_clock_minute, &state, &self.config);

        state.set_cc_open(self.cc_open);
        state.set_launcher_open(self.launcher_open);
        state.set_audio_picker_open(self.audio_picker_open);
        state.set_confirm_open(self.confirm_open);
        state.set_scenario(self.scenario.id().into());
        state.set_reduced_motion(self.config.accessibility.reduced_motion);

        let preview_has_windows = self
            .preview_app
            .as_deref()
            .and_then(|id| self.dock.app(&AppId::new(id)))
            .is_some_and(|app| !app.windows.is_empty());
        if !preview_has_windows {
            self.preview_open = false;
            self.preview_app = None;
        }
        state.set_preview_open(self.preview_open);
        // Keep the last preview contents in the view while its close animation
        // runs; only replace them when a preview is actually being opened.
        if self.preview_open
            && let Some(id) = self.preview_app.as_deref()
        {
            state.set_preview_app_id(id.into());
            let preview_name = self
                .dock
                .app(&AppId::new(id))
                .map(|app| app.definition.name.as_str())
                .unwrap_or_default()
                .to_owned();
            state.set_preview_app_name(preview_name.into());
            let windows: Vec<WindowItem> = window_items(&self.dock, id);
            state.set_preview_windows(ModelRc::new(VecModel::from(windows)));
        }
    }

    pub fn app_clicked(&mut self, ui: &AppWindow, id: &str) {
        // A click is a committed action. Cancel any hover-preview timer that
        // was armed before the click so a minimized app does not immediately
        // reopen its preview under the pointer.
        self.hover_generation = self.hover_generation.wrapping_add(1);
        self.hovered_app = None;
        self.hover_suppressed_until = Some(Instant::now() + Duration::from_millis(650));
        self.close_preview(ui);
        self.hide_tooltip(ui);
        self.cc_open = false;
        self.audio_picker_open = false;
        if self.launcher_open {
            self.launcher_open = false;
        }

        let app_id = AppId::new(id);
        let Some(intent) = self.dock.click_intent(&app_id) else {
            return;
        };
        match intent {
            prism_core::AppClickIntent::Launch(app_id) => {
                self.launch_app(ui, &app_id);
            }
            prism_core::AppClickIntent::Activate(window)
            | prism_core::AppClickIntent::Restore(window) => {
                self.activate_window_by_id(ui, window);
            }
            prism_core::AppClickIntent::Minimize(window) => {
                if let Err(error) = self.platform.minimize(window) {
                    self.show_toast(ui, error.to_string(), "error");
                } else {
                    self.dock.record_minimized(window);
                }
            }
        }
        self.apply(ui);
    }

    pub fn app_hovered(&mut self, ui: &AppWindow, id: &str) {
        self.hover_generation = self.hover_generation.wrapping_add(1);
        let generation = self.hover_generation;
        let hovered_id = id.to_owned();
        self.hovered_app = Some(hovered_id.clone());
        self.hide_tooltip(ui);

        // Do not pop a preview back open immediately after a click. This is
        // particularly important when the click minimized the active app.
        if self
            .hover_suppressed_until
            .is_some_and(|until| Instant::now() < until)
        {
            return;
        }

        if self.preview_app.as_deref() != Some(id) {
            self.close_preview(ui);
        }

        let Some(app) = self.dock.app(&AppId::new(id)) else {
            return;
        };
        let running = app.is_running();
        let app_name = app.definition.name.clone();

        let preview_id = hovered_id.clone();
        self.schedule(ui, Duration::from_millis(360), move |controller, ui| {
            if controller.hover_generation != generation
                || controller.hovered_app.as_deref() != Some(preview_id.as_str())
                || controller.cc_open
                || controller.launcher_open
                || !running
            {
                return;
            }
            controller.open_preview(ui, &preview_id);
        });

        if !running {
            let tooltip_id = hovered_id.clone();
            self.schedule(ui, Duration::from_millis(520), move |controller, ui| {
                if controller.hover_generation != generation
                    || controller.hovered_app.as_deref() != Some(tooltip_id.as_str())
                {
                    return;
                }
                ui.global::<UiState>()
                    .set_tooltip_text(app_name.clone().into());
                ui.global::<UiState>().set_tooltip_visible(true);
            });
        }
    }

    pub fn app_unhovered(&mut self, ui: &AppWindow, id: &str) {
        if self.hovered_app.as_deref() != Some(id) {
            return;
        }
        self.hover_generation = self.hover_generation.wrapping_add(1);
        self.hovered_app = None;
        self.hover_suppressed_until = None;
        self.hide_tooltip(ui);
        self.schedule_preview_close(ui, 300);
    }

    pub fn keep_preview_open(&mut self, ui: &AppWindow, hovered: bool) {
        self.preview_hovered = hovered;
        if !hovered {
            self.schedule_preview_close(ui, 220);
        }
    }

    pub fn close_preview(&mut self, ui: &AppWindow) {
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
        self.preview_open = false;
        self.preview_hovered = false;
        self.preview_app = None;
        ui.global::<UiState>().set_preview_open(false);
    }

    pub fn window_activated(&mut self, ui: &AppWindow, window_id: i32) {
        let Some(window) = self.window_by_ui_id(window_id) else {
            return;
        };
        if let Err(error) = self.platform.activate(window) {
            self.show_toast(ui, error.to_string(), "error");
            return;
        }
        self.dock.record_activated(window);
        self.close_preview(ui);
        self.apply(ui);
    }

    pub fn window_closed(&mut self, ui: &AppWindow, window_id: i32) {
        let Some(window) = self.window_by_ui_id(window_id) else {
            return;
        };
        let title = self
            .dock
            .apps()
            .iter()
            .flat_map(|app| app.windows.iter())
            .find(|entry| entry.id == window)
            .map(|entry| entry.title.clone())
            .unwrap_or_else(|| "Window".to_owned());
        if let Err(error) = self.platform.close(window) {
            self.show_toast(ui, error.to_string(), "error");
            return;
        }
        self.dock.record_closed(window);
        let preview_should_close = self.preview_app.as_deref().is_some_and(|id| {
            self.dock
                .app(&AppId::new(id))
                .is_none_or(|app| app.windows.is_empty())
        });
        if preview_should_close {
            self.close_preview(ui);
        }
        self.apply(ui);
        self.show_toast(ui, format!("Closed “{title}” · preview only"), "info");
    }

    pub fn toggle_launcher(&mut self, ui: &AppWindow) {
        self.launcher_open = !self.launcher_open;
        if self.launcher_open {
            self.launcher_query.clear();
            self.cc_open = false;
            self.audio_picker_open = false;
            self.hover_generation = self.hover_generation.wrapping_add(1);
            self.hovered_app = None;
            self.close_preview(ui);
        }
        self.apply(ui);
    }

    pub fn close_launcher(&mut self, ui: &AppWindow) {
        if !self.launcher_open {
            return;
        }
        self.launcher_open = false;
        self.apply(ui);
    }

    pub fn launcher_search(&mut self, ui: &AppWindow, query: &str) {
        self.launcher_query = query.to_owned();
        let state = ui.global::<UiState>();
        state.set_launcher_query(query.into());
        state.set_launcher_apps(ModelRc::new(VecModel::from(launcher_items(
            &self.dock,
            &self.launcher_query,
        ))));
    }

    pub fn launcher_app_clicked(&mut self, ui: &AppWindow, id: &str) {
        self.launcher_open = false;
        self.activate_or_launch(ui, id);
    }

    pub fn toggle_control_center(&mut self, ui: &AppWindow) {
        self.cc_open = !self.cc_open;
        if self.cc_open {
            self.launcher_open = false;
            self.audio_picker_open = false;
            self.hover_generation = self.hover_generation.wrapping_add(1);
            self.hovered_app = None;
            self.close_preview(ui);
            self.hide_tooltip(ui);
        } else {
            self.audio_picker_open = false;
        }
        self.apply(ui);
    }

    pub fn close_control_center(&mut self, ui: &AppWindow) {
        if !self.cc_open && !self.audio_picker_open {
            return;
        }
        self.cc_open = false;
        self.audio_picker_open = false;
        self.apply(ui);
    }

    pub fn quick_toggle_clicked(&mut self, ui: &AppWindow, id: &str) {
        let Some(toggle_id) = toggle_id_from_str(id) else {
            return;
        };
        let snapshot = self.platform.snapshot();
        let Some(toggle) = snapshot.toggle(toggle_id) else {
            return;
        };
        let target = !toggle.active;
        match self.platform.execute(SystemAction::SetToggle {
            id: toggle_id,
            active: target,
        }) {
            Ok(ActionOutcome::Pending) => {
                self.apply(ui);
                self.schedule(ui, Duration::from_millis(700), |controller, ui| {
                    controller.finish_pending(ui);
                });
            }
            Ok(ActionOutcome::Applied) => {
                self.apply(ui);
            }
            Ok(ActionOutcome::Simulated(message)) => {
                self.show_toast(ui, message, "info");
                self.apply(ui);
            }
            Err(error) => {
                self.show_platform_error(ui, &error);
                if toggle_id == ToggleId::NightLight {
                    self.open_settings(ui, "display");
                } else if toggle_id == ToggleId::Bluetooth
                    && matches!(error, PlatformError::PermissionRequired { .. })
                {
                    self.open_settings(ui, "bluetooth");
                }
                self.apply(ui);
            }
        }
        self.drain_events(ui);
    }

    pub fn volume_changed(&mut self, ui: &AppWindow, value: f32) {
        let volume = value.round().clamp(0.0, 100.0) as u8;
        match self.platform.execute(SystemAction::SetVolume(volume)) {
            Ok(_) => {
                let snapshot = self.platform.snapshot();
                let state = ui.global::<UiState>();
                state.set_volume(value.clamp(0.0, 100.0));
                state.set_muted(snapshot.audio.muted);
            }
            Err(error) => {
                self.show_platform_error(ui, &error);
                self.apply(ui);
            }
        }
    }

    pub fn toggle_mute(&mut self, ui: &AppWindow) {
        match self.platform.execute(SystemAction::ToggleMute) {
            Ok(_) => self.apply(ui),
            Err(error) => self.show_platform_error(ui, &error),
        }
    }

    pub fn toggle_audio_picker(&mut self, ui: &AppWindow) {
        if !self.cc_open {
            return;
        }
        self.audio_picker_open = !self.audio_picker_open;
        self.apply(ui);
    }

    pub fn audio_output_selected(&mut self, ui: &AppWindow, id: &str) {
        let name = self
            .platform
            .snapshot()
            .audio
            .outputs
            .iter()
            .find(|output| output.id == id)
            .map(|output| output.name.clone())
            .unwrap_or_else(|| "Audio output".to_owned());
        match self
            .platform
            .execute(SystemAction::SelectAudioOutput(id.to_owned()))
        {
            Ok(_) => {
                self.audio_picker_open = false;
                self.apply(ui);
                self.show_toast(ui, format!("Output set to {name} · preview only"), "info");
            }
            Err(error) => self.show_platform_error(ui, &error),
        }
    }

    pub fn brightness_changed(&mut self, ui: &AppWindow, value: f32) {
        let level = value.round().clamp(0.0, 100.0) as u8;
        match self.platform.execute(SystemAction::SetBrightness(level)) {
            Ok(_) => {
                ui.global::<UiState>()
                    .set_brightness(value.clamp(0.0, 100.0));
            }
            Err(error) => {
                self.show_platform_error(ui, &error);
                self.apply(ui);
            }
        }
    }

    pub fn media_play_pause(&mut self, ui: &AppWindow) {
        self.media_action(ui, SystemAction::MediaPlayPause);
    }

    pub fn media_next(&mut self, ui: &AppWindow) {
        self.media_action(ui, SystemAction::MediaNext);
    }

    pub fn media_previous(&mut self, ui: &AppWindow) {
        self.media_action(ui, SystemAction::MediaPrevious);
    }

    pub fn open_settings(&mut self, ui: &AppWindow, page: &str) {
        let page = match page {
            "sound" => SettingsPage::Sound,
            "network" => SettingsPage::Network,
            "bluetooth" => SettingsPage::Bluetooth,
            "display" => SettingsPage::Display,
            "power" => SettingsPage::Power,
            _ => SettingsPage::Home,
        };
        match self.platform.execute(SystemAction::OpenSettings(page)) {
            Ok(ActionOutcome::Simulated(message)) => self.show_toast(ui, message, "info"),
            Ok(_) => {}
            Err(error) => self.show_platform_error(ui, &error),
        }
    }

    pub fn power_requested(&mut self, ui: &AppWindow, action: &str) {
        match action {
            "power" => {
                self.confirm_open = true;
                let state = ui.global::<UiState>();
                state.set_confirm_title("Power options".into());
                state.set_confirm_detail(
                    "Restart or shut down this PC? This is a development preview: no power action will be taken."
                        .into(),
                );
                self.apply(ui);
            }
            "lock" => self.show_toast(ui, "Preview only: Lock would run", "info"),
            "sleep" => self.show_toast(ui, "Preview only: Sleep would run", "info"),
            "restart" => self.power_confirmed(ui, "restart"),
            "shutdown" => self.power_confirmed(ui, "shutdown"),
            _ => {}
        }
    }

    pub fn power_confirmed(&mut self, ui: &AppWindow, action: &str) {
        let power = match action {
            "restart" => PowerAction::Restart,
            "shutdown" => PowerAction::ShutDown,
            _ => return,
        };
        match self.platform.execute(SystemAction::Power(power)) {
            Ok(ActionOutcome::Simulated(message)) => {
                self.confirm_open = false;
                self.cc_open = false;
                self.audio_picker_open = false;
                self.apply(ui);
                self.show_toast(ui, message, "info");
            }
            Ok(_) => {
                self.confirm_open = false;
                self.cc_open = false;
                self.audio_picker_open = false;
                self.apply(ui);
            }
            Err(error) => self.show_platform_error(ui, &error),
        }
    }

    pub fn confirm_cancelled(&mut self, ui: &AppWindow) {
        self.confirm_open = false;
        self.apply(ui);
    }

    pub fn scenario_selected(&mut self, ui: &AppWindow, id: &str) {
        let Some(scenario) = PreviewScenario::from_id(id) else {
            return;
        };
        self.scenario = scenario;
        self.platform.set_scenario(scenario);
        self.dock = catalog::build_dock(scenario);
        self.launcher_open = false;
        self.launcher_query.clear();
        self.cc_open = false;
        self.audio_picker_open = false;
        self.confirm_open = false;
        self.close_preview(ui);
        self.hide_tooltip(ui);
        ui.global::<UiState>().set_launcher_query("".into());
        self.apply(ui);
        self.drain_events(ui);
    }

    pub fn simulate_display_change(&mut self, ui: &AppWindow) {
        self.platform.simulate_display_change();
        self.drain_events(ui);
    }

    pub fn simulate_audio_loss(&mut self, ui: &AppWindow) {
        self.platform.simulate_audio_loss();
        self.apply(ui);
        self.drain_events(ui);
    }

    pub fn dismiss_toast(&mut self, ui: &AppWindow) {
        self.toast_generation = self.toast_generation.wrapping_add(1);
        ui.global::<UiState>().set_toast_visible(false);
    }

    pub fn quit(&mut self) {
        slint::quit_event_loop().ok();
    }

    /// One-second presentation tick. Only properties that actually changed are
    /// written, so Slint's dependency-driven renderer stays idle when nothing
    /// moves. The clock only repaints once per minute.
    pub fn tick(&mut self, ui: &AppWindow) {
        let state = ui.global::<UiState>();
        apply_clock(&mut self.last_clock_minute, &state, &self.config);

        let snapshot = self.platform.snapshot();
        if snapshot.media.session.is_some() {
            self.platform.tick_media();
            let snapshot = self.platform.snapshot();
            self.apply_media_fields(&state, &snapshot.media);
        }

        let width = current_stage_width(ui, self.stage_width);
        if (width - self.stage_width).abs() > 1.0 {
            self.apply(ui);
        }

        self.drain_events(ui);
    }

    pub fn show_toast(&mut self, ui: &AppWindow, message: impl Into<String>, kind: &str) {
        self.toast_generation = self.toast_generation.wrapping_add(1);
        let generation = self.toast_generation;
        let state = ui.global::<UiState>();
        state.set_toast_message(message.into().into());
        state.set_toast_kind(kind.into());
        state.set_toast_visible(true);
        self.schedule(ui, Duration::from_millis(4200), move |controller, ui| {
            if controller.toast_generation == generation {
                ui.global::<UiState>().set_toast_visible(false);
            }
        });
    }

    pub fn open_control_center_for_artifact(&mut self, ui: &AppWindow) {
        if !self.cc_open {
            self.toggle_control_center(ui);
        }
    }

    pub fn open_launcher_for_artifact(&mut self, ui: &AppWindow) {
        if !self.launcher_open {
            self.toggle_launcher(ui);
        }
    }

    pub fn open_preview_for_artifact(&mut self, ui: &AppWindow, app_id: &str) {
        if self
            .dock
            .app(&AppId::new(app_id))
            .is_some_and(|app| app.is_running())
        {
            self.cc_open = false;
            self.launcher_open = false;
            self.open_preview(ui, app_id);
            self.apply(ui);
        }
    }

    pub fn show_power_confirmation_for_artifact(&mut self, ui: &AppWindow) {
        self.power_requested(ui, "power");
    }

    fn launch_app(&mut self, ui: &AppWindow, app_id: &AppId) {
        let Some(app) = self.dock.app(app_id) else {
            return;
        };
        let definition = app.definition.clone();
        match self.platform.launch(&definition) {
            Ok(receipt) => {
                self.dock.record_window_opened(app_id, receipt.title);
                self.show_toast(
                    ui,
                    format!("Launched {} · preview only", definition.name),
                    "success",
                );
            }
            Err(error) => self.show_platform_error(ui, &error),
        }
    }

    fn activate_or_launch(&mut self, ui: &AppWindow, id: &str) {
        let app_id = AppId::new(id);
        let Some(app) = self.dock.app(&app_id) else {
            return;
        };
        if let Some(window) = app.most_recent_window().map(|window| window.id) {
            self.activate_window_by_id(ui, window);
        } else {
            self.launch_app(ui, &app_id);
            self.apply(ui);
            self.drain_events(ui);
        }
    }

    fn activate_window_by_id(&mut self, ui: &AppWindow, window: WindowId) {
        if let Err(error) = self.platform.activate(window) {
            self.show_platform_error(ui, &error);
            return;
        }
        self.dock.record_activated(window);
        self.apply(ui);
    }

    fn open_preview(&mut self, ui: &AppWindow, app_id: &str) {
        let Some(app) = self.dock.app(&AppId::new(app_id)) else {
            return;
        };
        if app.windows.is_empty() {
            return;
        }
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
        self.preview_app = Some(app_id.to_owned());
        self.preview_open = true;
        self.preview_hovered = false;
        let anchor_slot = self
            .dock
            .apps()
            .iter()
            .position(|entry| entry.definition.id.as_str() == app_id)
            .and_then(|slot| i32::try_from(slot).ok())
            .unwrap_or(-1);
        let state = ui.global::<UiState>();
        state.set_preview_anchor_slot(anchor_slot);
        state.set_preview_app_id(app_id.into());
        state.set_preview_app_name(app.definition.name.as_str().into());
        state.set_preview_windows(ModelRc::new(VecModel::from(window_items(
            &self.dock, app_id,
        ))));
        state.set_preview_app_index(
            i32::try_from(
                self.dock
                    .apps()
                    .iter()
                    .position(|entry| entry.definition.id.as_str() == app_id)
                    .unwrap_or(0),
            )
            .unwrap_or(0),
        );
        state.set_preview_open(true);
        state.set_tooltip_visible(false);
    }

    fn schedule_preview_close(&mut self, ui: &AppWindow, delay_ms: u64) {
        if !self.preview_open {
            return;
        }
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
        let generation = self.preview_close_generation;
        self.schedule(
            ui,
            Duration::from_millis(delay_ms),
            move |controller, ui| {
                if controller.preview_close_generation == generation
                    && !controller.preview_hovered
                    && controller.hovered_app.is_none()
                {
                    controller.close_preview(ui);
                }
            },
        );
    }

    fn hide_tooltip(&mut self, ui: &AppWindow) {
        ui.global::<UiState>().set_tooltip_visible(false);
    }

    fn finish_pending(&mut self, ui: &AppWindow) {
        if !self.platform.toggle_is_pending() {
            return;
        }
        let event = self.platform.finish_pending();
        self.apply(ui);
        if let Some(event) = event {
            self.report_event(ui, event);
        }
    }

    fn media_action(&mut self, ui: &AppWindow, action: SystemAction) {
        match self.platform.execute(action) {
            Ok(_) => self.apply(ui),
            Err(error) => self.show_platform_error(ui, &error),
        }
    }

    fn drain_events(&mut self, ui: &AppWindow) {
        let events = self.platform.poll();
        for event in events {
            self.report_event(ui, event);
        }
    }

    fn report_event(&mut self, ui: &AppWindow, event: PlatformEvent) {
        match event {
            PlatformEvent::Notice(message) => self.show_toast(ui, message, "info"),
            PlatformEvent::Error(message) => self.show_toast(ui, message, "error"),
            PlatformEvent::StateChanged => self.apply(ui),
        }
    }

    fn show_platform_error(&mut self, ui: &AppWindow, error: &PlatformError) {
        self.show_toast(ui, error.to_string(), "error");
    }

    fn apply_media(&self, state: &UiState, media: &MediaState) {
        if media.phase.failure().is_some() {
            state.set_media_state("error".into());
        } else if media.session.is_some() {
            state.set_media_state("ready".into());
        } else if media.capability.can_attempt() {
            state.set_media_state("empty".into());
        } else {
            state.set_media_state("unsupported".into());
        }
        state.set_media_detail(
            media
                .phase
                .failure()
                .unwrap_or("No compatible media session is active.")
                .into(),
        );
        self.apply_media_fields(state, media);
    }

    fn apply_media_fields(&self, state: &UiState, media: &MediaState) {
        if let Some(session) = &media.session {
            state.set_media_title(session.title.as_str().into());
            state.set_media_artist(session.artist.as_str().into());
            state.set_media_album(session.album.as_str().into());
            state.set_media_playing(session.playing);
            state.set_media_progress(if session.duration_seconds == 0 {
                0.0
            } else {
                (session.position_seconds as f32 / session.duration_seconds as f32).clamp(0.0, 1.0)
            });
            state.set_media_position(format_duration(session.position_seconds).into());
            state.set_media_duration(format_duration(session.duration_seconds).into());
            state.set_media_art(session.artwork_key.as_str().into());
        }
    }

    fn window_by_ui_id(&self, ui_id: i32) -> Option<WindowId> {
        self.dock
            .apps()
            .iter()
            .flat_map(|app| app.windows.iter())
            .find(|window| window_id_to_i32(window.id) == ui_id)
            .map(|window| window.id)
    }

    fn schedule<F>(&self, ui: &AppWindow, delay: Duration, callback: F)
    where
        F: FnOnce(&mut PreviewController, &AppWindow) + 'static,
    {
        let Some(controller) = self.self_weak.upgrade() else {
            return;
        };
        let ui_weak = ui.as_weak();
        slint::Timer::single_shot(delay, move || {
            if let (Some(ui), Ok(mut controller)) = (ui_weak.upgrade(), controller.try_borrow_mut())
            {
                callback(&mut controller, &ui);
            }
        });
    }
}

fn apply_battery(state: &UiState, snapshot: &SystemSnapshot) {
    state.set_battery_present(snapshot.battery.present);
    state.set_battery_percent(i32::from(snapshot.battery.percent));
    state.set_battery_charging(snapshot.battery.charging);
    let detail = if snapshot.battery.present {
        if snapshot.battery.charging {
            "Charging · 1h 12m to full"
        } else {
            "On battery · 4h 20m remaining"
        }
    } else {
        ""
    };
    state.set_battery_detail(detail.into());
}

fn apply_clock(last: &mut Option<(u32, u32)>, state: &UiState, config: &PrismConfig) {
    let now = Local::now().naive_local();
    let clock = format_clock(now, config.dock.use_24_hour_clock);
    let key = (clock.hour, clock.minute);
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    state.set_clock_time(clock.time.into());
    state.set_clock_date(clock.date.into());
    state.set_clock_show_date(config.dock.show_date);
}

fn current_stage_width(ui: &AppWindow, fallback: f32) -> f32 {
    let width = ui.get_stage_width();
    if width > 1.0 { width } else { fallback }
}

fn scenario_items() -> Vec<ScenarioItem> {
    PreviewScenario::ALL
        .into_iter()
        .map(|scenario| ScenarioItem {
            id: scenario.id().into(),
            label: scenario.label().into(),
        })
        .collect()
}

fn toggle_id_from_str(id: &str) -> Option<ToggleId> {
    ToggleId::ALL
        .into_iter()
        .find(|toggle| toggle.as_str() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_ids_round_trip_through_the_view_layer() {
        for id in ToggleId::ALL {
            assert_eq!(toggle_id_from_str(id.as_str()), Some(id));
        }
        assert_eq!(toggle_id_from_str("unknown"), None);
    }

    #[test]
    fn phase_failure_is_visible_to_the_tile_mapping() {
        let phase = prism_core::ControlPhase::failed("nope");
        assert_eq!(phase.failure(), Some("nope"));
    }
}
