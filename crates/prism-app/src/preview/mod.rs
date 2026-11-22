//! Development preview executable implementation.
//!
//! The preview is intentionally a separate binary from the production shell.
//! It uses `MockPlatform` for every system value and never performs a real
//! system operation.

pub mod catalog;
pub mod controller;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use prism_ui::{AppWindow, UiState};
use slint::ComponentHandle;

use crate::args::PreviewArgs;
use crate::config;
use crate::preview::controller::{ControllerHandle, PreviewController};

type BoxError = Box<dyn Error>;

pub fn run(args: PreviewArgs) -> Result<(), BoxError> {
    let loaded = config::load(args.config.as_deref());
    if let Some(warning) = &loaded.warning {
        tracing::warn!("{warning}");
    }
    let mut config = loaded.config;
    if args.twelve_hour {
        config.dock.use_24_hour_clock = false;
    }
    if args.no_date {
        config.dock.show_date = false;
    }
    if args.reduced_motion {
        config.accessibility.reduced_motion = true;
    }

    tracing::info!(
        scenario = args.scenario.id(),
        width = args.width,
        height = args.height,
        screenshot = args.screenshot.is_some(),
        "starting Prism Shell Linux development preview"
    );
    tracing::info!(
        config = loaded
            .path
            .as_deref()
            .map(|path| path.display().to_string())
            .as_deref()
            .unwrap_or("<defaults>"),
        "configuration source"
    );

    let ui = AppWindow::new()?;
    ui.set_stage_width(args.width as f32);
    ui.set_stage_height(args.height as f32);
    if args.fullscreen {
        ui.window().set_fullscreen(true);
    }

    let controller = PreviewController::new(&ui, args.scenario, config, args.width as f32);
    wire_callbacks(&ui, &controller);
    controller.borrow_mut().apply(&ui);

    if let Some(warning) = loaded.warning {
        controller.borrow_mut().show_toast(&ui, warning, "error");
    }
    if args.open_cc {
        controller
            .borrow_mut()
            .open_control_center_for_artifact(&ui);
    }
    if args.open_launcher {
        controller.borrow_mut().open_launcher_for_artifact(&ui);
    }
    if let Some(app_id) = args.preview_app.as_deref() {
        controller
            .borrow_mut()
            .open_preview_for_artifact(&ui, app_id);
    }
    if args.confirm {
        controller
            .borrow_mut()
            .show_power_confirmation_for_artifact(&ui);
    }
    if let Some(message) = args.toast.as_deref() {
        controller.borrow_mut().show_toast(&ui, message, "info");
    }

    let _tick_timer = start_tick_timer(&ui, &controller);

    if let Some(path) = args.screenshot.as_ref() {
        let expected = path.clone();
        ui.show()?;
        schedule_snapshot(&ui, path.clone());
        slint::run_event_loop()?;
        if !expected.exists() {
            return Err(Box::new(std::io::Error::other(format!(
                "screenshot was not written to {}",
                expected.display()
            ))));
        }
        tracing::info!(path = %expected.display(), "screenshot written");
    } else {
        ui.run()?;
        tracing::info!("preview event loop exited");
    }

    Ok(())
}

fn wire_callbacks(ui: &AppWindow, controller: &ControllerHandle) {
    macro_rules! wire0 {
        ($setter:ident, $method:ident) => {{
            let controller = controller.clone();
            let weak = ui.as_weak();
            ui.global::<UiState>().$setter(move || {
                if let (Some(ui), Ok(mut controller)) =
                    (weak.upgrade(), controller.try_borrow_mut())
                {
                    controller.$method(&ui);
                }
            });
        }};
    }

    macro_rules! wire_str {
        ($setter:ident, $method:ident) => {{
            let controller = controller.clone();
            let weak = ui.as_weak();
            ui.global::<UiState>()
                .$setter(move |value: slint::SharedString| {
                    if let (Some(ui), Ok(mut controller)) =
                        (weak.upgrade(), controller.try_borrow_mut())
                    {
                        controller.$method(&ui, value.as_str());
                    }
                });
        }};
    }

    macro_rules! wire_f32 {
        ($setter:ident, $method:ident) => {{
            let controller = controller.clone();
            let weak = ui.as_weak();
            ui.global::<UiState>().$setter(move |value: f32| {
                if let (Some(ui), Ok(mut controller)) =
                    (weak.upgrade(), controller.try_borrow_mut())
                {
                    controller.$method(&ui, value);
                }
            });
        }};
    }

    macro_rules! wire_i32 {
        ($setter:ident, $method:ident) => {{
            let controller = controller.clone();
            let weak = ui.as_weak();
            ui.global::<UiState>().$setter(move |value: i32| {
                if let (Some(ui), Ok(mut controller)) =
                    (weak.upgrade(), controller.try_borrow_mut())
                {
                    controller.$method(&ui, value);
                }
            });
        }};
    }

    wire_str!(on_app_clicked, app_clicked);
    wire_str!(on_app_hovered, app_hovered);
    wire_str!(on_app_unhovered, app_unhovered);
    wire_i32!(on_window_activated, window_activated);
    wire_i32!(on_window_closed, window_closed);
    wire0!(on_toggle_launcher, toggle_launcher);
    wire0!(on_close_launcher, close_launcher);
    wire_str!(on_launcher_search, launcher_search);
    wire_str!(on_launcher_app_clicked, launcher_app_clicked);

    wire0!(on_toggle_control_center, toggle_control_center);
    wire0!(on_close_control_center, close_control_center);
    wire_str!(on_quick_toggle_clicked, quick_toggle_clicked);
    wire_f32!(on_volume_changed, volume_changed);
    wire0!(on_toggle_mute, toggle_mute);
    wire0!(on_toggle_audio_picker, toggle_audio_picker);
    wire_str!(on_audio_output_selected, audio_output_selected);
    wire_f32!(on_brightness_changed, brightness_changed);
    wire_str!(on_open_settings, open_settings);
    wire0!(on_media_play_pause, media_play_pause);
    wire0!(on_media_next, media_next);
    wire0!(on_media_previous, media_previous);
    wire_str!(on_power_requested, power_requested);
    wire_str!(on_power_confirmed, power_confirmed);
    wire0!(on_confirm_cancelled, confirm_cancelled);

    wire0!(on_close_preview, close_preview);
    {
        let controller = controller.clone();
        let weak = ui.as_weak();
        ui.global::<UiState>()
            .on_keep_preview_open(move |value: bool| {
                if let (Some(ui), Ok(mut controller)) =
                    (weak.upgrade(), controller.try_borrow_mut())
                {
                    controller.keep_preview_open(&ui, value);
                }
            });
    }

    wire_str!(on_scenario_selected, scenario_selected);
    wire0!(on_simulate_display_change, simulate_display_change);
    wire0!(on_simulate_audio_loss, simulate_audio_loss);
    wire0!(on_dismiss_toast, dismiss_toast);
    {
        let controller = controller.clone();
        ui.global::<UiState>().on_quit_preview(move || {
            if let Ok(mut controller) = controller.try_borrow_mut() {
                controller.quit();
            }
        });
    }
    {
        let controller = controller.clone();
        ui.global::<UiState>().on_quit_shell(move || {
            if let Ok(mut controller) = controller.try_borrow_mut() {
                controller.quit();
            }
        });
    }
}

fn start_tick_timer(ui: &AppWindow, controller: &ControllerHandle) -> slint::Timer {
    let timer = slint::Timer::default();
    let controller = controller.clone();
    let weak = ui.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            if let (Some(ui), Ok(mut controller)) = (weak.upgrade(), controller.try_borrow_mut()) {
                controller.tick(&ui);
            }
        },
    );
    timer
}

fn schedule_snapshot(ui: &AppWindow, path: PathBuf) {
    let weak = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(900), move || {
        if let Some(ui) = weak.upgrade() {
            match save_snapshot(&ui, &path) {
                Ok(()) => tracing::info!(path = %path.display(), "screenshot captured"),
                Err(error) => tracing::error!(path = %path.display(), %error, "screenshot failed"),
            }
        }
        slint::quit_event_loop().ok();
    });
}

pub fn save_snapshot(ui: &AppWindow, path: &Path) -> Result<(), BoxError> {
    let buffer = ui.window().take_snapshot()?;
    let width = buffer.width();
    let height = buffer.height();
    let mut pixels = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for pixel in buffer.as_slice() {
        pixels.extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
    }
    let image = image::RgbaImage::from_raw(width, height, pixels).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot buffer dimensions did not match its pixel data",
        )
    })?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    Ok(())
}
