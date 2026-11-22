//! Native Windows shell controller.
//!
//! This path is deliberately separate from the Linux preview. It uses real
//! window enumeration, real HWND activation, real shell icons, and native
//! Settings entry points. System controls that are not implemented are shown as
//! unavailable; nothing here is simulated.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Local;
use prism_core::fit_dock_apps;
use prism_core::{
    AppDefinition, AppId, DockState, IconKey, PowerAction, PrismConfig, SettingsPage, SystemAction,
    SystemSnapshot, WindowId, format_clock,
};
use prism_platform::{ActionOutcome, SystemServices, WindowServices};
use prism_ui::model::{
    app_items, launcher_items, quick_toggle_items, window_id_to_i32, window_items,
};
use prism_ui::{
    AppItem, ShellControlCenter, ShellLauncher, ShellTaskbar, ShellWindowPreview, UiState,
    WindowItem,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::HHOOK;
use windows::Win32::UI::WindowsAndMessaging::WINEVENT_OUTOFCONTEXT;
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CREATE, EVENT_OBJECT_NAMECHANGE, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, SetForegroundWindow,
};

use crate::host;
use crate::icons;
use crate::layout::{self, PhysicalRect};
use crate::naming::{friendly_app_name, normalized_exe_id};
use crate::platform::WindowsPlatform;
use crate::preview::{PreviewThumbnails, ThumbnailTarget};
use crate::recovery::ReplacementMarker;
use crate::taskbar_replacement::{self, Heartbeat};
use crate::window::{self, Foreground, TrackedWindow};

static EVENT_DIRTY: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    EVENT_DIRTY.store(true, Ordering::Release);
}

struct PinnedApp {
    definition: AppDefinition,
    icon_source: String,
}

struct WindowsShell {
    self_weak: Weak<RefCell<WindowsShell>>,
    taskbar: ShellTaskbar,
    control_center: ShellControlCenter,
    launcher: ShellLauncher,
    window_preview: ShellWindowPreview,
    platform: WindowsPlatform,
    config: PrismConfig,
    dock: DockState,
    pinned: Vec<PinnedApp>,
    icon_cache: HashMap<String, Option<slint::Image>>,
    taskbar_hwnd: Option<HWND>,
    cc_hwnd: Option<HWND>,
    launcher_hwnd: Option<HWND>,
    preview_hwnd: Option<HWND>,
    cc_visible: bool,
    launcher_visible: bool,
    preview_visible: bool,
    cc_open: bool,
    launcher_open: bool,
    preview_open: bool,
    cc_hide_at: Option<Instant>,
    launcher_hide_at: Option<Instant>,
    preview_hide_at: Option<Instant>,
    cc_opened_at: Option<Instant>,
    launcher_opened_at: Option<Instant>,
    launcher_query: String,
    work: PhysicalRect,
    scale: f32,
    dock_geometry: PhysicalRect,
    taskbar_bounds: Option<PhysicalRect>,
    cc_bounds: Option<PhysicalRect>,
    launcher_bounds: Option<PhysicalRect>,
    preview_bounds: Option<PhysicalRect>,
    startup_started_at: Option<Instant>,
    pending_cc_focus: bool,
    pending_launcher_focus: bool,
    fatal_error: Option<String>,
    visible_count: usize,
    hidden_count: usize,
    last_external_foreground: Option<u64>,
    last_clock_minute: Option<(u32, u32)>,
    hooks: Vec<HWINEVENTHOOK>,
    last_refresh: Option<Instant>,
    hovered_app: Option<String>,
    hover_generation: u64,
    hover_suppressed_until: Option<Instant>,
    preview_app_id: Option<String>,
    preview_anchor_slot: i32,
    preview_hovered: bool,
    preview_close_generation: u64,
    preview_thumbnails_enabled: bool,
    preview_window_ids: HashMap<i32, u64>,
    preview_thumbnails: PreviewThumbnails,
    replacement_active: bool,
    replacement_marker: Option<ReplacementMarker>,
    heartbeat: Option<Heartbeat>,
    keyboard_hook: Option<HHOOK>,
    fallback_hotkey_registered: bool,
    quit: bool,
}

pub fn run_shell(config: PrismConfig) -> Result<(), Box<dyn Error>> {
    // If a previous process left the native taskbar hidden, put it back before
    // creating any Prism surface.
    taskbar_replacement::recover_stale_marker();

    let taskbar = ShellTaskbar::new()?;
    let control_center = ShellControlCenter::new()?;
    let launcher = ShellLauncher::new()?;
    let window_preview = ShellWindowPreview::new()?;

    let shell = Rc::new(RefCell::new(WindowsShell::new(
        taskbar,
        control_center,
        launcher,
        window_preview,
        config,
    )));
    shell.borrow_mut().self_weak = Rc::downgrade(&shell);

    wire_callbacks(&shell);

    shell.borrow_mut().start()?;

    let timer = slint::Timer::default();
    let weak = Rc::downgrade(&shell);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            if let Some(shell) = weak.upgrade()
                && let Ok(mut shell) = shell.try_borrow_mut()
            {
                shell.tick();
            }
        },
    );

    // A dedicated, opt-in shortcut poll keeps standalone-Windows-key latency
    // low without making the idle shell wake up frequently. It is not started
    // unless the user enabled the integration.
    let shortcut_timer = slint::Timer::default();
    let shortcuts_enabled = {
        let config = &shell.borrow().config.experimental;
        config.windows_key_launcher || config.windows_key_fallback_hotkey
    };
    if shortcuts_enabled {
        let weak = Rc::downgrade(&shell);
        shortcut_timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.handle_launcher_shortcut();
                }
            },
        );
    }

    // The taskbar HWND is created by winit during the first event-loop
    // iteration, not by show(). Resolve it as soon as the loop starts.
    let weak = Rc::downgrade(&shell);
    slint::Timer::single_shot(Duration::ZERO, move || {
        if let Some(shell) = weak.upgrade()
            && let Ok(mut shell) = shell.try_borrow_mut()
        {
            shell.tick();
        }
    });

    slint::run_event_loop()?;
    shell.borrow_mut().shutdown();
    drop(timer);
    drop(shortcut_timer);

    if let Some(error) = shell.borrow().fatal_error.clone() {
        return Err(error.into());
    }
    Ok(())
}

impl WindowsShell {
    fn new(
        taskbar: ShellTaskbar,
        control_center: ShellControlCenter,
        launcher: ShellLauncher,
        window_preview: ShellWindowPreview,
        config: PrismConfig,
    ) -> Self {
        let work = host::primary_work_area();
        let scale = host::system_scale().max(0.5);
        let pinned = discover_pinned_apps();
        let mut icon_cache = HashMap::new();
        for pinned_app in &pinned {
            let image = icons::load_icon(&pinned_app.icon_source);
            icon_cache.insert(pinned_app.definition.id.as_str().to_owned(), image);
        }
        let definitions = pinned
            .iter()
            .map(|app| app.definition.clone())
            .collect::<Vec<_>>();
        let dock = DockState::new(definitions);
        let dock_geometry = layout::dock_rect(work, scale, 320.0);

        let mut shell = Self {
            self_weak: Weak::new(),
            taskbar,
            control_center,
            launcher,
            window_preview,
            platform: WindowsPlatform,
            config,
            dock,
            pinned,
            icon_cache,
            taskbar_hwnd: None,
            cc_hwnd: None,
            launcher_hwnd: None,
            preview_hwnd: None,
            cc_visible: false,
            launcher_visible: false,
            preview_visible: false,
            cc_open: false,
            launcher_open: false,
            preview_open: false,
            cc_hide_at: None,
            launcher_hide_at: None,
            preview_hide_at: None,
            cc_opened_at: None,
            launcher_opened_at: None,
            launcher_query: String::new(),
            work,
            scale,
            dock_geometry,
            taskbar_bounds: None,
            cc_bounds: None,
            launcher_bounds: None,
            preview_bounds: None,
            startup_started_at: None,
            pending_cc_focus: false,
            pending_launcher_focus: false,
            fatal_error: None,
            visible_count: 0,
            hidden_count: 0,
            last_external_foreground: None,
            last_clock_minute: None,
            hooks: Vec::new(),
            last_refresh: None,
            hovered_app: None,
            hover_generation: 0,
            hover_suppressed_until: None,
            preview_app_id: None,
            preview_anchor_slot: -1,
            preview_hovered: false,
            preview_close_generation: 0,
            preview_thumbnails_enabled: false,
            preview_window_ids: HashMap::new(),
            preview_thumbnails: PreviewThumbnails::new(),
            replacement_active: false,
            replacement_marker: None,
            heartbeat: None,
            keyboard_hook: None,
            fallback_hotkey_registered: false,
            quit: false,
        };
        shell.apply();
        shell
    }

    fn start(&mut self) -> Result<(), Box<dyn Error>> {
        // Set an initial logical size before requesting the native window. The
        // actual HWND is created by winit during the first event-loop iteration,
        // so it is resolved and configured in `tick`, not here.
        let initial_stage = self.work.width as f32 / self.scale;
        self.taskbar.set_stage_width(initial_stage);
        self.taskbar.show()?;
        self.startup_started_at = Some(Instant::now());
        tracing::info!(
            work = ?self.work,
            scale = self.scale,
            stage_width = self.taskbar.get_stage_width(),
            "Prism Windows taskbar window requested"
        );
        Ok(())
    }

    fn finish_taskbar_start(&mut self, hwnd: HWND) {
        host::configure_taskbar(hwnd);
        host::set_taskbar_dock_rect(Some(self.dock_geometry));
        self.update_layout();

        let foreground = window::foreground();
        tracing::info!(
            hwnd = ?hwnd,
            work = ?self.work,
            scale = self.scale,
            stage_width = self.taskbar.get_stage_width(),
            foreground_own = foreground.is_some_and(|f| f.own_process),
            "Prism Windows taskbar host created"
        );

        self.install_event_hooks();
        self.refresh();
        self.precreate_preview_window();
        self.start_experimental_integrations();
        self.startup_started_at = None;
    }

    fn start_experimental_integrations(&mut self) {
        if self.config.experimental.taskbar_replacement {
            self.enable_taskbar_replacement();
        }
        if self.config.experimental.windows_key_launcher
            || self.config.experimental.windows_key_fallback_hotkey
        {
            self.enable_windows_key_integration();
        }
        self.apply();
        self.update_layout();
    }

    fn resolve_native_windows(&mut self) {
        if self.taskbar_hwnd.is_none() {
            match host::hwnd_of(self.taskbar.window()) {
                Ok(hwnd) => {
                    self.taskbar_hwnd = Some(hwnd);
                    self.finish_taskbar_start(hwnd);
                }
                Err(error) => {
                    if self
                        .startup_started_at
                        .is_some_and(|started| started.elapsed() > Duration::from_secs(5))
                    {
                        let message = format!("the taskbar window was not created: {error}");
                        tracing::error!(%error, "taskbar HWND unavailable after startup timeout");
                        self.fatal_error = Some(message);
                        self.quit = true;
                        slint::quit_event_loop().ok();
                    }
                }
            }
        }

        if self.cc_visible && self.cc_hwnd.is_none() {
            self.try_attach_control_center();
        }
        if self.launcher_visible && self.launcher_hwnd.is_none() {
            self.try_attach_launcher();
        }
        if self.preview_visible && self.preview_hwnd.is_none() {
            self.try_attach_preview();
        }
    }

    fn schedule_tick(&self, delay: Duration) {
        let Some(shell) = self.self_weak.upgrade() else {
            return;
        };
        slint::Timer::single_shot(delay, move || {
            if let Ok(mut shell) = shell.try_borrow_mut() {
                shell.tick();
            }
        });
    }

    fn schedule<F>(&self, delay: Duration, callback: F)
    where
        F: FnOnce(&mut Self) + 'static,
    {
        let Some(shell) = self.self_weak.upgrade() else {
            return;
        };
        slint::Timer::single_shot(delay, move || {
            if let Ok(mut shell) = shell.try_borrow_mut() {
                callback(&mut shell);
            }
        });
    }

    fn shutdown(&mut self) {
        for hook in self.hooks.drain(..) {
            unsafe {
                let _ = UnhookWinEvent(hook);
            }
        }
        if let Some(hook) = self.keyboard_hook.take() {
            host::remove_keyboard_hook(hook);
        }
        if self.fallback_hotkey_registered
            && let Some(hwnd) = self.taskbar_hwnd
        {
            host::unregister_fallback_hotkey(hwnd);
            self.fallback_hotkey_registered = false;
        }
        self.close_preview();
        self.preview_thumbnails.clear();
        self.hide_preview_native();
        self.hide_cc_native();
        self.hide_launcher_native();
        if self.replacement_active && !self.restore_taskbar_replacement("clean shutdown") {
            tracing::error!(
                "native taskbar restore failed during shutdown; prism-recovery.exe will retry"
            );
        }
        tracing::info!(
            replacement_active = self.replacement_active,
            "Prism Windows shell stopped"
        );
    }

    fn install_event_hooks(&mut self) {
        let flags = WINEVENT_OUTOFCONTEXT
            | windows::Win32::UI::WindowsAndMessaging::WINEVENT_SKIPOWNPROCESS;
        unsafe {
            let foreground = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(event_proc),
                0,
                0,
                flags,
            );
            if !foreground.0.is_null() {
                self.hooks.push(foreground);
            }
            let objects = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_NAMECHANGE,
                None,
                Some(event_proc),
                0,
                0,
                flags,
            );
            if !objects.0.is_null() {
                self.hooks.push(objects);
            }
            let minimize = SetWinEventHook(
                EVENT_SYSTEM_MINIMIZESTART,
                EVENT_SYSTEM_MINIMIZEEND,
                None,
                Some(event_proc),
                0,
                0,
                flags,
            );
            if !minimize.0.is_null() {
                self.hooks.push(minimize);
            }
        }
        tracing::info!(
            hooks = self.hooks.len(),
            "window reconciliation hooks installed"
        );
    }

    fn enable_taskbar_replacement(&mut self) {
        let pid = std::process::id();
        let heartbeat = match Heartbeat::create(pid) {
            Ok(heartbeat) => heartbeat,
            Err(error) => {
                tracing::error!(%error, "taskbar replacement unavailable");
                return;
            }
        };
        if let Err(error) = taskbar_replacement::spawn_watchdog(pid) {
            tracing::error!(%error, "taskbar replacement disabled");
            return;
        }
        match taskbar_replacement::hide_native_taskbars() {
            Ok(marker) => {
                self.heartbeat = Some(heartbeat);
                self.replacement_marker = Some(marker);
                self.replacement_active = true;
                tracing::warn!(
                    "EXPERIMENTAL taskbar replacement active; explorer remains running and \
                     the recovery watchdog is armed"
                );
                self.update_layout();
                self.apply();
            }
            Err(error) => {
                tracing::error!(%error, "taskbar replacement aborted; native taskbar restored");
            }
        }
    }

    fn restore_taskbar_replacement(&mut self, reason: &str) -> bool {
        let Some(marker) = self.replacement_marker.clone() else {
            self.replacement_active = false;
            self.heartbeat = None;
            return true;
        };
        match taskbar_replacement::restore_from_marker(&marker) {
            Ok(()) => {
                tracing::info!(reason, "native Windows taskbar restored");
                self.replacement_active = false;
                self.replacement_marker = None;
                self.heartbeat = None;
                self.update_layout();
                self.apply();
                true
            }
            Err(error) => {
                tracing::error!(%reason, %error, "native Windows taskbar restore failed");
                false
            }
        }
    }

    fn check_taskbar_replacement_health(&mut self) {
        if !self.replacement_active {
            return;
        }
        if taskbar_replacement::native_primary_taskbar_visible() {
            // Explorer restarted or another tool restored the taskbar. Never
            // fight it by hiding again automatically; coexistence is the safe
            // state and the user can explicitly opt in again.
            tracing::warn!("native Windows taskbar became visible again; leaving replacement mode");
            if let Some(marker) = self.replacement_marker.clone() {
                let _ = taskbar_replacement::restore_from_marker(&marker);
            }
            self.replacement_active = false;
            self.replacement_marker = None;
            self.heartbeat = None;
            self.update_layout();
            self.apply();
        }
    }

    fn enable_windows_key_integration(&mut self) {
        let hook = if self.config.experimental.windows_key_launcher {
            match host::install_keyboard_hook() {
                Ok(hook) => {
                    tracing::info!(
                        "experimental standalone Windows-key hook installed; combinations pass through"
                    );
                    self.keyboard_hook = Some(hook);
                    true
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "standalone Windows-key hook failed; using the fallback shortcut"
                    );
                    false
                }
            }
        } else {
            false
        };

        let register_fallback = !hook || self.config.experimental.windows_key_fallback_hotkey;
        if register_fallback && let Some(hwnd) = self.taskbar_hwnd {
            let shortcut = &self.config.experimental.windows_key_fallback_shortcut;
            let hotkey = crate::keyboard::parse_hotkey(shortcut).unwrap_or_else(|error| {
                tracing::warn!(
                    %error,
                    "invalid windows_key_fallback_shortcut; using Ctrl+Alt+Space"
                );
                crate::keyboard::parse_hotkey("Ctrl+Alt+Space")
                    .expect("built-in fallback hotkey is valid")
            });
            match host::register_fallback_hotkey(hwnd, hotkey) {
                Ok(()) => {
                    self.fallback_hotkey_registered = true;
                    tracing::info!(
                        shortcut = %shortcut,
                        "fallback launcher shortcut registered"
                    );
                }
                Err(error) => tracing::warn!(%error, "fallback launcher shortcut unavailable"),
            }
        }
    }

    fn handle_launcher_shortcut(&mut self) {
        if host::take_launcher_shortcut() {
            self.toggle_launcher();
        }
    }

    fn tick(&mut self) {
        if self.quit {
            return;
        }
        self.handle_launcher_shortcut();
        self.resolve_native_windows();
        if self.quit {
            return;
        }
        let now = Instant::now();
        if EVENT_DIRTY.swap(false, Ordering::AcqRel)
            || self
                .last_refresh
                .is_none_or(|last| now.duration_since(last) > Duration::from_millis(5000))
        {
            self.refresh();
        }

        self.update_clock();
        self.check_display_change();

        if host::take_cc_deactivation()
            && self.cc_open
            && self
                .cc_opened_at
                .is_none_or(|opened| now.duration_since(opened) > Duration::from_millis(350))
        {
            self.close_control_center();
        }
        if host::take_launcher_deactivation()
            && self.launcher_open
            && self
                .launcher_opened_at
                .is_none_or(|opened| now.duration_since(opened) > Duration::from_millis(350))
        {
            self.close_launcher();
        }

        if self.cc_hide_at.is_some_and(|at| now >= at) {
            self.cc_hide_at = None;
            self.hide_cc_native();
        }
        if self.launcher_hide_at.is_some_and(|at| now >= at) {
            self.launcher_hide_at = None;
            self.hide_launcher_native();
        }
        if self.preview_open {
            if host::preview_hovered() {
                self.preview_hovered = true;
                self.preview_hide_at = None;
            } else if self.preview_hovered && self.hovered_app.is_none() {
                self.preview_hovered = false;
                self.schedule_preview_close(220);
            }
        }
        if self.preview_hide_at.is_some_and(|at| now >= at) {
            self.preview_hide_at = None;
            self.hide_preview_native();
        }
        if let Some(heartbeat) = &self.heartbeat {
            heartbeat.pulse();
        }
        self.check_taskbar_replacement_health();
    }

    fn refresh(&mut self) {
        let tracked = window::enumerate();
        let foreground = window::foreground();
        self.update_icons(&tracked);

        let mut definitions = self
            .pinned
            .iter()
            .map(|app| app.definition.clone())
            .collect::<Vec<_>>();
        let mut seen: HashSet<String> = definitions
            .iter()
            .map(|definition| definition.id.as_str().to_owned())
            .collect();
        for tracked_window in &tracked {
            if seen.insert(tracked_window.app.id.clone()) {
                definitions.push(AppDefinition {
                    id: AppId::new(tracked_window.app.id.clone()),
                    name: tracked_window.app.name.clone(),
                    icon: IconKey::Generic,
                    launch_target: tracked_window.app.launch_target.clone().unwrap_or_default(),
                    pin_order: None,
                });
            }
        }

        let mut dock = DockState::new(definitions);
        for tracked_window in &tracked {
            dock.record_window_discovered(
                &AppId::new(tracked_window.window.app_id.clone()),
                WindowId::new(tracked_window.window.hwnd),
                tracked_window.window.title.clone(),
                tracked_window.window.minimized,
            );
        }

        let active = self.resolve_foreground(foreground, &dock);
        if let Some(window) = active {
            dock.record_activated(window);
        }
        self.dock = dock;
        self.apply();
        if self.preview_open {
            self.refresh_preview_contents();
        }
        self.update_layout();
        self.last_refresh = Some(Instant::now());
        tracing::debug!(
            windows = tracked.len(),
            apps = self.dock.apps().len(),
            active = ?self.dock.active_window(),
            "window model reconciled"
        );
    }

    fn resolve_foreground(
        &mut self,
        foreground: Option<Foreground>,
        dock: &DockState,
    ) -> Option<WindowId> {
        match foreground {
            Some(Foreground {
                hwnd,
                own_process: false,
            }) => {
                self.last_external_foreground = Some(hwnd);
            }
            Some(Foreground {
                own_process: true, ..
            }) => {}
            None => {
                self.last_external_foreground = None;
            }
        }

        let candidate = self.last_external_foreground.map(WindowId::new);
        let exists = candidate.is_some_and(|candidate| {
            dock.apps()
                .iter()
                .any(|app| app.windows.iter().any(|window| window.id == candidate))
        });
        if !exists {
            self.last_external_foreground = None;
            None
        } else {
            candidate
        }
    }

    fn update_icons(&mut self, tracked: &[TrackedWindow]) {
        for app in &self.pinned {
            let key = app.definition.id.as_str().to_owned();
            self.icon_cache
                .entry(key)
                .or_insert_with(|| icons::load_icon(&app.icon_source));
        }
        for app in tracked {
            let key = app.app.id.clone();
            self.icon_cache
                .entry(key)
                .or_insert_with(|| app.app.icon_source.as_deref().and_then(icons::load_icon));
        }
    }

    fn decorated_app_items(&self) -> Vec<AppItem> {
        let mut items = app_items(&self.dock);
        for item in &mut items {
            if let Some(Some(image)) = self.icon_cache.get(item.id.as_str()) {
                item.has_icon = true;
                item.icon = image.clone();
            }
        }
        items
    }

    fn decorated_launcher_items(&self) -> Vec<AppItem> {
        let mut items = launcher_items(&self.dock, &self.launcher_query);
        for item in &mut items {
            if let Some(Some(image)) = self.icon_cache.get(item.id.as_str()) {
                item.has_icon = true;
                item.icon = image.clone();
            }
        }
        items
    }

    fn apply(&mut self) {
        let snapshot = self.platform.snapshot();
        let stage_width = self.work.width as f32 / self.scale;
        let all_apps = self.decorated_app_items();
        let fit = fit_dock_apps(all_apps.len(), stage_width);
        let visible = all_apps
            .into_iter()
            .take(fit.visible_apps)
            .collect::<Vec<_>>();
        self.visible_count = fit.visible_apps;
        self.hidden_count = fit.hidden_apps;
        let launcher_apps = self.decorated_launcher_items();
        let quick_toggles = quick_toggle_items(&snapshot);

        set_taskbar_state(
            &self.taskbar,
            &visible,
            fit.hidden_apps,
            &snapshot,
            TaskbarViewState {
                cc_open: self.cc_open,
                launcher_open: self.launcher_open,
                replacement_active: self.replacement_active,
            },
            &self.config,
        );
        set_control_center_state(
            &self.control_center,
            &quick_toggles,
            &snapshot,
            self.cc_open,
            self.replacement_active,
        );
        set_launcher_state(
            &self.launcher,
            &launcher_apps,
            &self.launcher_query,
            self.launcher_open,
        );

        self.refresh_dock_geometry();
    }

    fn refresh_dock_geometry(&mut self) {
        let stage_width = self.work.width as f32 / self.scale;
        let width = layout::dock_width(stage_width, self.visible_count, self.hidden_count > 0);
        self.dock_geometry = layout::dock_rect(self.work, self.scale, width);
        host::set_taskbar_dock_rect(Some(self.dock_geometry));
    }

    fn effective_work_area(&self) -> PhysicalRect {
        if self.replacement_active {
            host::primary_monitor_area()
        } else {
            host::primary_work_area()
        }
    }

    fn update_layout(&mut self) {
        let work = self.effective_work_area();
        let scale = self
            .taskbar_hwnd
            .map(host::window_scale)
            .unwrap_or_else(host::system_scale)
            .max(0.5);
        self.work = work;
        self.scale = scale;
        let stage_width = work.width as f32 / scale;
        self.taskbar.set_stage_width(stage_width);

        self.refresh_dock_geometry();

        if let Some(hwnd) = self.taskbar_hwnd {
            let window_rect = layout::taskbar_window(work, scale);
            if self.taskbar_bounds != Some(window_rect) {
                host::configure_taskbar(hwnd);
                if let Err(error) = host::position_taskbar(hwnd, window_rect) {
                    tracing::warn!(%error, "taskbar positioning failed");
                }
                self.taskbar_bounds = Some(window_rect);
            }
        }

        if self.cc_visible {
            self.control_center
                .set_panel_max_height((work.height as f32 / scale - 170.0).max(300.0));
            let panel_height = self.control_center.get_panel_height().max(1.0);
            let panel =
                layout::control_center_panel(work, self.dock_geometry, 384.0, panel_height, scale);
            host::set_cc_panel_rect(Some(panel));
            let window_rect = layout::inflate(panel, scale);
            if self.cc_hwnd.is_some() && self.cc_bounds != Some(window_rect) {
                if let Some(hwnd) = self.cc_hwnd
                    && let Err(error) = host::position_panel(hwnd, window_rect, false)
                {
                    tracing::warn!(%error, "Control Center positioning failed");
                } else {
                    self.cc_bounds = Some(window_rect);
                }
            }
        }

        if self.launcher_visible {
            let panel = layout::launcher_panel(work, self.dock_geometry, 360.0, 452.0, scale);
            host::set_launcher_panel_rect(Some(panel));
            let window_rect = layout::inflate(panel, scale);
            if self.launcher_hwnd.is_some() && self.launcher_bounds != Some(window_rect) {
                if let Some(hwnd) = self.launcher_hwnd
                    && let Err(error) = host::position_panel(hwnd, window_rect, false)
                {
                    tracing::warn!(%error, "launcher positioning failed");
                } else {
                    self.launcher_bounds = Some(window_rect);
                }
            }
        }

        if self.preview_visible {
            self.update_preview_layout();
        }
    }

    fn check_display_change(&mut self) {
        let work = self.effective_work_area();
        let scale = self
            .taskbar_hwnd
            .map(host::window_scale)
            .unwrap_or_else(host::system_scale)
            .max(0.5);
        if work != self.work || (scale - self.scale).abs() > 0.01 {
            tracing::info!(
                ?work,
                scale,
                "display work area changed; repositioning shell"
            );
            self.work = work;
            self.scale = scale;
            self.update_layout();
        }
    }

    fn update_clock(&mut self) {
        let clock = format_clock(
            Local::now().naive_local(),
            self.config.dock.use_24_hour_clock,
        );
        let minute = (clock.hour, clock.minute);
        if self.last_clock_minute == Some(minute) {
            return;
        }
        self.last_clock_minute = Some(minute);
        let state = self.taskbar.global::<UiState>();
        state.set_clock_time(clock.time.into());
        state.set_clock_date(clock.date.into());
        state.set_clock_show_date(self.config.dock.show_date);
    }

    fn ensure_control_center_window(&mut self) -> Result<(), String> {
        if self.cc_visible {
            return Ok(());
        }
        self.control_center
            .show()
            .map_err(|error| format!("Control Center window creation failed: {error}"))?;
        self.cc_visible = true;
        self.cc_hwnd = None;
        self.cc_bounds = None;
        self.pending_cc_focus = true;
        self.try_attach_control_center();
        self.schedule_tick(Duration::from_millis(16));
        Ok(())
    }

    fn try_attach_control_center(&mut self) {
        if !self.cc_visible || self.cc_hwnd.is_some() {
            return;
        }
        let Ok(hwnd) = host::hwnd_of(self.control_center.window()) else {
            return;
        };
        host::configure_control_center(hwnd);
        self.cc_hwnd = Some(hwnd);
        self.cc_bounds = None;
        self.update_layout();
        self.focus_control_center_if_ready();
    }

    fn focus_control_center_if_ready(&mut self) {
        if !self.pending_cc_focus {
            return;
        }
        if let Some(hwnd) = self.cc_hwnd {
            self.pending_cc_focus = false;
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
            host::clear_deactivation_flags();
        }
    }

    fn ensure_launcher_window(&mut self) -> Result<(), String> {
        if self.launcher_visible {
            return Ok(());
        }
        self.launcher
            .show()
            .map_err(|error| format!("launcher window creation failed: {error}"))?;
        self.launcher_visible = true;
        self.launcher_hwnd = None;
        self.launcher_bounds = None;
        self.pending_launcher_focus = true;
        self.try_attach_launcher();
        self.schedule_tick(Duration::from_millis(16));
        Ok(())
    }

    fn try_attach_launcher(&mut self) {
        if !self.launcher_visible || self.launcher_hwnd.is_some() {
            return;
        }
        let Ok(hwnd) = host::hwnd_of(self.launcher.window()) else {
            return;
        };
        host::configure_launcher(hwnd);
        self.launcher_hwnd = Some(hwnd);
        self.launcher_bounds = None;
        self.update_layout();
        self.focus_launcher_if_ready();
    }

    fn focus_launcher_if_ready(&mut self) {
        if !self.pending_launcher_focus {
            return;
        }
        if let Some(hwnd) = self.launcher_hwnd {
            self.pending_launcher_focus = false;
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
            host::clear_deactivation_flags();
        }
    }

    fn open_control_center(&mut self) {
        if let Err(error) = self.ensure_control_center_window() {
            tracing::error!(%error, "could not create Control Center");
            return;
        }
        self.hover_generation = self.hover_generation.wrapping_add(1);
        self.hovered_app = None;
        self.close_preview();
        self.close_launcher_immediate();
        self.cc_open = true;
        self.cc_hide_at = None;
        self.cc_opened_at = Some(Instant::now());
        host::clear_deactivation_flags();
        self.apply();
        self.update_layout();
        self.pending_cc_focus = true;
        self.focus_control_center_if_ready();
        // The initial show can emit an inactive notification before the
        // explicit foreground request. Clear it after activation so the first
        // tick does not immediately close a freshly opened panel.
        host::clear_deactivation_flags();
    }

    fn open_launcher(&mut self) {
        if let Err(error) = self.ensure_launcher_window() {
            tracing::error!(%error, "could not create launcher");
            return;
        }
        self.hover_generation = self.hover_generation.wrapping_add(1);
        self.hovered_app = None;
        self.close_preview();
        self.close_control_center_immediate();
        self.launcher_open = true;
        self.launcher_hide_at = None;
        self.launcher_opened_at = Some(Instant::now());
        host::clear_deactivation_flags();
        self.apply();
        self.update_layout();
        self.pending_launcher_focus = true;
        self.focus_launcher_if_ready();
        host::clear_deactivation_flags();
    }

    fn close_control_center(&mut self) {
        if !self.cc_open {
            return;
        }
        self.cc_open = false;
        self.cc_hide_at = Some(Instant::now() + Duration::from_millis(280));
        self.apply();
    }

    fn close_control_center_immediate(&mut self) {
        self.cc_open = false;
        self.cc_hide_at = None;
        self.taskbar.global::<UiState>().set_cc_open(false);
        self.control_center.global::<UiState>().set_cc_open(false);
        self.hide_cc_native();
    }

    fn hide_cc_native(&mut self) {
        let hide_failed = self.cc_visible && self.control_center.hide().is_err();
        if let Some(hwnd) = self.cc_hwnd.take()
            && hide_failed
        {
            host::hide_window(hwnd);
        }
        self.cc_visible = false;
        self.cc_bounds = None;
        self.pending_cc_focus = false;
        host::set_cc_panel_rect(None);
    }

    fn close_launcher(&mut self) {
        if !self.launcher_open {
            return;
        }
        self.launcher_open = false;
        self.launcher_hide_at = Some(Instant::now() + Duration::from_millis(260));
        self.apply();
    }

    fn close_launcher_immediate(&mut self) {
        self.launcher_open = false;
        self.launcher_hide_at = None;
        self.taskbar.global::<UiState>().set_launcher_open(false);
        self.launcher.global::<UiState>().set_launcher_open(false);
        self.hide_launcher_native();
    }

    fn hide_launcher_native(&mut self) {
        let hide_failed = self.launcher_visible && self.launcher.hide().is_err();
        if let Some(hwnd) = self.launcher_hwnd.take()
            && hide_failed
        {
            host::hide_window(hwnd);
        }
        self.launcher_visible = false;
        self.launcher_bounds = None;
        self.pending_launcher_focus = false;
        host::set_launcher_panel_rect(None);
    }

    fn decorated_window_items(&self, app_id: &str) -> Vec<WindowItem> {
        let mut items = window_items(&self.dock, app_id);
        if let Some(Some(icon)) = self.icon_cache.get(app_id) {
            for item in &mut items {
                item.has_icon = true;
                item.icon = icon.clone();
            }
        }
        items
    }

    fn precreate_preview_window(&mut self) {
        if self.preview_visible || self.preview_open {
            return;
        }
        // Create and style the preview HWND once while it is empty. The first
        // hover then shows an already non-activating tool window instead of
        // letting a fresh native window steal focus from the active app.
        if let Err(error) = self.window_preview.show() {
            tracing::warn!(%error, "preview window pre-creation failed");
            return;
        }
        self.preview_visible = true;
        self.preview_hwnd = None;
        self.preview_bounds = None;
        self.schedule_tick(Duration::from_millis(16));
    }

    fn ensure_preview_window(&mut self) {
        if self.preview_visible {
            return;
        }
        if let Err(error) = self.window_preview.show() {
            tracing::warn!(%error, "preview window creation failed");
            self.preview_open = false;
            return;
        }
        self.preview_visible = true;
        self.preview_hwnd = None;
        self.preview_bounds = None;
        self.schedule_tick(Duration::from_millis(16));
    }

    fn try_attach_preview(&mut self) {
        if !self.preview_visible || self.preview_hwnd.is_some() {
            return;
        }
        let Ok(hwnd) = host::hwnd_of(self.window_preview.window()) else {
            return;
        };
        host::configure_preview(hwnd);
        self.preview_hwnd = Some(hwnd);
        self.preview_bounds = None;
        if !self.preview_open {
            // This was the startup pre-creation path. Keep the HWND hidden
            // until a real hover opens the preview.
            self.hide_preview_native();
            return;
        }
        self.update_preview_layout();
    }

    fn update_preview_layout(&mut self) {
        let Some(app_id) = self.preview_app_id.clone() else {
            return;
        };
        let Some(entry) = self.dock.app(&AppId::new(&app_id)) else {
            return;
        };
        let count = entry.windows.len();
        if count == 0 {
            return;
        }
        let panel_width = layout::preview_panel_width(self.work, self.scale, count);
        self.window_preview.set_stage_width(panel_width + 48.0);
        self.window_preview.set_panel_width(panel_width);
        let window_rect = layout::preview_window(
            self.work,
            self.dock_geometry,
            self.scale,
            count,
            self.preview_anchor_slot,
        );
        let panel_rect = layout::deflate(window_rect, self.scale);
        host::set_preview_panel_rect(Some(panel_rect));
        if let Some(hwnd) = self.preview_hwnd
            && self.preview_bounds != Some(window_rect)
            && let Err(error) = host::position_preview(hwnd, window_rect)
        {
            tracing::warn!(%error, "preview positioning failed");
            return;
        }
        self.preview_bounds = Some(window_rect);
        self.sync_preview_thumbnails();
    }

    fn sync_preview_thumbnails(&mut self) {
        let Some(hwnd) = self.preview_hwnd else {
            return;
        };
        let Some(window_rect) = self.preview_bounds else {
            return;
        };
        if !self.preview_open || !self.preview_thumbnails_enabled {
            self.preview_thumbnails.clear();
            return;
        }
        let Some(app_id) = self.preview_app_id.as_deref() else {
            self.preview_thumbnails.clear();
            return;
        };
        let targets: Vec<ThumbnailTarget> = self
            .dock
            .app(&AppId::new(app_id))
            .map(|entry| {
                entry
                    .windows
                    .iter()
                    .map(|window| ThumbnailTarget {
                        hwnd: window.id.get(),
                        minimized: window.minimized,
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.preview_thumbnails
            .sync(hwnd, window_rect, self.scale, &targets);
    }

    fn refresh_preview_contents(&mut self) {
        let Some(app_id) = self.preview_app_id.clone() else {
            self.close_preview();
            return;
        };
        let Some(entry) = self.dock.app(&AppId::new(&app_id)) else {
            self.close_preview();
            return;
        };
        if entry.windows.is_empty() {
            self.close_preview();
            return;
        }
        let app_name = entry.definition.name.clone();
        let windows = entry.windows.clone();
        let slot = self
            .dock
            .apps()
            .iter()
            .position(|entry| entry.definition.id.as_str() == app_id)
            .and_then(|slot| i32::try_from(slot).ok())
            .unwrap_or(-1);
        let items = self.decorated_window_items(&app_id);

        self.preview_anchor_slot = slot;
        self.preview_window_ids.clear();
        for window in &windows {
            self.preview_window_ids
                .insert(window_id_to_i32(window.id), window.id.get());
        }

        let state = self.window_preview.global::<UiState>();
        state.set_preview_live_thumbnails(true);
        state.set_preview_open(true);
        state.set_preview_app_id(app_id.as_str().into());
        state.set_preview_app_name(app_name.into());
        state.set_preview_anchor_slot(slot);
        state.set_preview_windows(ModelRc::new(VecModel::from(items)));
        self.update_preview_layout();
    }

    fn app_hovered(&mut self, id: &str) {
        self.hover_generation = self.hover_generation.wrapping_add(1);
        let generation = self.hover_generation;
        self.hovered_app = Some(id.to_owned());
        // Any new hover cancels a pending preview close.
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);

        // A click just committed an action (often minimize). Do not pop the
        // preview straight back open under the pointer.
        if self
            .hover_suppressed_until
            .is_some_and(|until| Instant::now() < until)
        {
            return;
        }

        if self.preview_open && self.preview_app_id.as_deref() != Some(id) {
            self.close_preview();
        }

        let Some(entry) = self.dock.app(&AppId::new(id)) else {
            return;
        };
        if !entry.is_running() {
            return;
        }
        let app_id = id.to_owned();
        self.schedule(Duration::from_millis(300), move |shell| {
            if shell.hover_generation != generation
                || shell.hovered_app.as_deref() != Some(app_id.as_str())
                || shell.cc_open
                || shell.launcher_open
            {
                return;
            }
            if shell
                .dock
                .app(&AppId::new(&app_id))
                .is_some_and(|entry| entry.is_running())
            {
                shell.open_preview(&app_id);
            }
        });
    }

    fn app_unhovered(&mut self, id: &str) {
        if self.hovered_app.as_deref() != Some(id) {
            return;
        }
        self.hover_generation = self.hover_generation.wrapping_add(1);
        self.hovered_app = None;
        if self.preview_open && !self.preview_hovered {
            self.schedule_preview_close(320);
        }
    }

    fn preview_hovered_changed(&mut self, hovered: bool) {
        if !self.preview_open {
            return;
        }
        if hovered {
            self.preview_hovered = true;
            self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
            self.preview_hide_at = None;
        }
        // A false value is not authoritative: Slint card TouchAreas sit above
        // the root hover area. The 300 ms tick uses native WM_MOUSELEAVE state
        // to decide when the pointer has actually left the combined area.
    }

    fn open_preview(&mut self, app_id: &str) {
        let Some(entry) = self.dock.app(&AppId::new(app_id)) else {
            return;
        };
        if entry.windows.is_empty() {
            return;
        }
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
        self.preview_thumbnails.clear();
        self.preview_app_id = Some(app_id.to_owned());
        self.preview_open = true;
        self.preview_hovered = false;
        self.preview_thumbnails_enabled = false;
        host::reset_preview_hover();
        self.preview_hide_at = None;
        self.ensure_preview_window();
        // The Slint WindowPreview owns its short fade; the native window stays
        // visible until that completes. DWM thumbnails cannot inherit Slint's
        // opacity, so register them only after the surface has faded in.
        self.refresh_preview_contents();
        self.schedule(Duration::from_millis(170), |shell| {
            if shell.preview_open {
                shell.preview_thumbnails_enabled = true;
                shell.sync_preview_thumbnails();
            }
        });
    }

    fn schedule_preview_close(&mut self, delay_ms: u64) {
        if !self.preview_open {
            return;
        }
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
        let generation = self.preview_close_generation;
        self.schedule(Duration::from_millis(delay_ms), move |shell| {
            if shell.preview_close_generation == generation
                && !shell.preview_hovered
                && !host::preview_hovered()
                && shell.hovered_app.is_none()
            {
                shell.close_preview();
            }
        });
    }

    fn close_preview(&mut self) {
        self.preview_close_generation = self.preview_close_generation.wrapping_add(1);
        self.preview_hovered = false;
        self.preview_app_id = None;
        self.preview_thumbnails_enabled = false;
        self.preview_thumbnails.clear();
        host::set_preview_panel_rect(None);
        self.window_preview
            .global::<UiState>()
            .set_preview_open(false);
        if !self.preview_open && !self.preview_visible {
            return;
        }
        self.preview_open = false;
        if self.preview_visible {
            self.preview_hide_at = Some(Instant::now() + Duration::from_millis(230));
        }
    }

    fn hide_preview_native(&mut self) {
        self.preview_thumbnails.clear();
        host::set_preview_panel_rect(None);
        if self.preview_visible
            && self.window_preview.hide().is_err()
            && let Some(hwnd) = self.preview_hwnd
        {
            host::hide_window(hwnd);
        }
        self.preview_visible = false;
        self.preview_hwnd = None;
        self.preview_bounds = None;
        self.preview_window_ids.clear();
        self.preview_open = false;
        self.preview_thumbnails_enabled = false;
        self.preview_hide_at = None;
        self.preview_hovered = false;
        host::reset_preview_hover();
    }

    fn preview_window_activated(&mut self, ui_id: i32) {
        let Some(hwnd) = self.preview_window_ids.get(&ui_id).copied() else {
            return;
        };
        let window = WindowId::new(hwnd);
        self.close_preview();
        self.close_control_center_immediate();
        self.close_launcher_immediate();
        if let Err(error) = self.platform.activate(window) {
            tracing::warn!(%error, "preview window activation failed");
        }
        self.dock.record_activated(window);
        self.apply();
        EVENT_DIRTY.store(true, Ordering::Release);
    }

    fn preview_window_closed(&mut self, ui_id: i32) {
        let Some(hwnd) = self.preview_window_ids.get(&ui_id).copied() else {
            return;
        };
        let window = WindowId::new(hwnd);
        if let Err(error) = self.platform.close(window) {
            tracing::warn!(%error, "preview window close failed");
            EVENT_DIRTY.store(true, Ordering::Release);
            return;
        }
        self.dock.record_closed(window);
        if self
            .preview_app_id
            .as_deref()
            .and_then(|id| self.dock.app(&AppId::new(id)))
            .is_none_or(|entry| entry.windows.is_empty())
        {
            self.close_preview();
        } else {
            self.refresh_preview_contents();
        }
        self.apply();
        EVENT_DIRTY.store(true, Ordering::Release);
    }

    fn toggle_control_center(&mut self) {
        if self.cc_open {
            self.close_control_center();
        } else {
            self.open_control_center();
        }
    }

    fn toggle_launcher(&mut self) {
        if self.launcher_open {
            self.close_launcher();
        } else {
            self.open_launcher();
        }
    }

    fn app_clicked(&mut self, id: &str) {
        self.hover_generation = self.hover_generation.wrapping_add(1);
        self.hovered_app = None;
        self.hover_suppressed_until = Some(Instant::now() + Duration::from_millis(650));
        self.close_preview();
        self.close_control_center_immediate();
        self.close_launcher_immediate();
        let app_id = AppId::new(id);
        let Some(intent) = self.dock.click_intent(&app_id) else {
            return;
        };
        match intent {
            prism_core::AppClickIntent::Launch(app_id) => {
                if let Some(app) = self.dock.app(&app_id) {
                    let definition = app.definition.clone();
                    match self.platform.launch(&definition) {
                        Ok(receipt) => tracing::info!(
                            app = %definition.name,
                            pid = ?receipt.process_id,
                            "application launch requested"
                        ),
                        Err(error) => {
                            tracing::warn!(%error, app = %definition.name, "launch failed")
                        }
                    }
                }
            }
            prism_core::AppClickIntent::Activate(window)
            | prism_core::AppClickIntent::Restore(window) => {
                if let Err(error) = self.platform.activate(window) {
                    tracing::warn!(%error, "window activation failed");
                }
                self.dock.record_activated(window);
            }
            prism_core::AppClickIntent::Minimize(window) => {
                if let Err(error) = self.platform.minimize(window) {
                    tracing::warn!(%error, "window minimize failed");
                }
                self.dock.record_minimized(window);
            }
        }
        self.apply();
        EVENT_DIRTY.store(true, Ordering::Release);
    }

    fn launcher_app_clicked(&mut self, id: &str) {
        self.close_launcher_immediate();
        self.close_control_center_immediate();
        let app_id = AppId::new(id);
        if let Some(window) = self
            .dock
            .app(&app_id)
            .and_then(|app| app.most_recent_window().map(|window| window.id))
        {
            if let Err(error) = self.platform.activate(window) {
                tracing::warn!(%error, "launcher activation failed");
            }
            self.dock.record_activated(window);
        } else if let Some(app) = self.dock.app(&app_id) {
            let definition = app.definition.clone();
            if let Err(error) = self.platform.launch(&definition) {
                tracing::warn!(%error, "launcher launch failed");
            }
        }
        self.apply();
        EVENT_DIRTY.store(true, Ordering::Release);
    }

    fn launcher_search(&mut self, query: &str) {
        self.launcher_query = query.to_owned();
        let apps = self.decorated_launcher_items();
        set_launcher_state(
            &self.launcher,
            &apps,
            &self.launcher_query,
            self.launcher_open,
        );
    }

    fn quick_toggle_clicked(&mut self, id: &str) {
        let uri = match id {
            "wifi" => "ms-settings:network-wifi",
            "bluetooth" => "ms-settings:bluetooth",
            "focus" => "ms-settings:notifications",
            "night-light" => "ms-settings:display",
            _ => return,
        };
        if let Err(error) = self.platform.open_uri(uri) {
            tracing::warn!(%error, %uri, "opening native settings failed");
        }
        self.close_control_center();
    }

    fn open_settings(&mut self, page: &str) {
        let page = match page {
            "sound" => SettingsPage::Sound,
            "network" => SettingsPage::Network,
            "bluetooth" => SettingsPage::Bluetooth,
            "display" => SettingsPage::Display,
            "power" => SettingsPage::Power,
            _ => SettingsPage::Home,
        };
        match self.platform.execute(SystemAction::OpenSettings(page)) {
            Ok(ActionOutcome::Applied) => {}
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "opening Windows Settings failed"),
        }
    }

    fn power_requested(&mut self, action: &str) {
        if action != "lock" {
            tracing::warn!(action, "power action is not exposed in this build");
            return;
        }
        match self
            .platform
            .execute(SystemAction::Power(PowerAction::Lock))
        {
            Ok(_) => {
                self.close_control_center_immediate();
                tracing::info!("workstation lock requested");
            }
            Err(error) => tracing::warn!(%error, "locking the workstation failed"),
        }
    }

    fn quit(&mut self) {
        self.quit = true;
        slint::quit_event_loop().ok();
    }
}

fn discover_pinned_apps() -> Vec<PinnedApp> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_owned());
    let program_files = std::env::var("ProgramFiles").ok();
    let program_files_x86 = std::env::var("ProgramFiles(x86)").ok();

    let mut paths: Vec<String> = vec![
        format!(r"{windir}\explorer.exe"),
        format!(r"{windir}\System32\cmd.exe"),
        format!(r"{windir}\System32\WindowsPowerShell\v1.0\powershell.exe"),
    ];
    for base in [program_files_x86.as_deref(), program_files.as_deref()]
        .into_iter()
        .flatten()
    {
        let candidate = format!(r"{base}\Microsoft\Edge\Application\msedge.exe");
        if std::path::Path::new(&candidate).is_file() {
            paths.push(candidate);
            break;
        }
    }

    let mut apps = Vec::new();
    let mut seen = HashSet::new();
    let mut order = 0u32;
    for path in paths {
        if !std::path::Path::new(&path).is_file() {
            continue;
        }
        let id = normalized_exe_id(&path);
        if !seen.insert(id.clone()) {
            continue;
        }
        let name = friendly_app_name(&path);
        apps.push(PinnedApp {
            definition: AppDefinition {
                id: AppId::new(id),
                name,
                icon: IconKey::Generic,
                launch_target: path.clone(),
                pin_order: Some(order),
            },
            icon_source: path,
        });
        order += 1;
    }
    apps
}

struct TaskbarViewState {
    cc_open: bool,
    launcher_open: bool,
    replacement_active: bool,
}

fn set_taskbar_state(
    window: &ShellTaskbar,
    apps: &[AppItem],
    hidden: usize,
    snapshot: &SystemSnapshot,
    view: TaskbarViewState,
    config: &PrismConfig,
) {
    let state = window.global::<UiState>();
    state.set_development_preview(false);
    state.set_apps(ModelRc::new(VecModel::from(apps.to_vec())));
    state.set_hidden_app_count(i32::try_from(hidden).unwrap_or(i32::MAX));
    state.set_cc_open(view.cc_open);
    state.set_launcher_open(view.launcher_open);
    state.set_taskbar_replacement_active(view.replacement_active);
    state.set_platform_label(
        if view.replacement_active {
            "Windows 11 · taskbar replacement"
        } else {
            "Windows 11 · native taskbar"
        }
        .into(),
    );
    state.set_show_audio_status(false);
    state.set_show_wifi_status(false);
    state.set_preview_open(false);
    state.set_confirm_open(false);
    state.set_tooltip_visible(false);

    let clock = format_clock(Local::now().naive_local(), config.dock.use_24_hour_clock);
    state.set_clock_time(clock.time.into());
    state.set_clock_date(clock.date.into());
    state.set_clock_show_date(config.dock.show_date);

    state.set_battery_present(snapshot.battery.present);
    state.set_battery_percent(i32::from(snapshot.battery.percent));
    state.set_battery_charging(snapshot.battery.charging);
    state.set_battery_detail(
        if snapshot.battery.present {
            if snapshot.battery.charging {
                "Charging"
            } else {
                "On battery"
            }
        } else {
            ""
        }
        .into(),
    );
}

fn set_control_center_state(
    window: &ShellControlCenter,
    quick_toggles: &[prism_ui::ToggleItem],
    snapshot: &SystemSnapshot,
    cc_open: bool,
    taskbar_replacement_active: bool,
) {
    let state = window.global::<UiState>();
    state.set_development_preview(false);
    state.set_platform_label(
        if taskbar_replacement_active {
            "Windows 11 · taskbar replacement"
        } else {
            "Windows 11 · native taskbar"
        }
        .into(),
    );
    state.set_version_label(env!("CARGO_PKG_VERSION").into());
    state.set_footer_label(
        if taskbar_replacement_active {
            "Native taskbar hidden · recovery watchdog armed · use Restore bar or prism-recovery"
        } else {
            "Windows backend · native taskbar + Windows settings"
        }
        .into(),
    );
    state.set_taskbar_replacement_active(taskbar_replacement_active);
    state.set_show_unsafe_power(false);
    state.set_quick_toggles(ModelRc::new(VecModel::from(quick_toggles.to_vec())));
    state.set_cc_open(cc_open);
    state.set_audio_picker_open(false);
    state.set_audio_outputs(ModelRc::new(VecModel::from(
        Vec::<prism_ui::AudioOutputItem>::new(),
    )));
    state.set_audio_device("Not available".into());
    state.set_audio_available(false);
    state.set_audio_state_detail("Audio controls are not implemented in this build.".into());
    state.set_volume(0.0);
    state.set_muted(true);
    state.set_brightness(0.0);
    state.set_brightness_available(false);
    state.set_brightness_note("Brightness control is not implemented in this build.".into());
    state.set_wifi_on(false);
    state.set_wifi_name("Wi-Fi".into());
    state.set_wifi_detail("Windows settings".into());
    state.set_wifi_capability("unsupported".into());
    state.set_bluetooth_on(false);
    state.set_bluetooth_name("Bluetooth".into());
    state.set_bluetooth_detail("Windows settings".into());
    state.set_bluetooth_capability("unsupported".into());
    state.set_wired(false);
    state.set_battery_present(snapshot.battery.present);
    state.set_battery_percent(i32::from(snapshot.battery.percent));
    state.set_battery_charging(snapshot.battery.charging);
    state.set_battery_detail(
        if snapshot.battery.present {
            if snapshot.battery.charging {
                "Charging"
            } else {
                "On battery"
            }
        } else {
            ""
        }
        .into(),
    );
    state.set_media_state("unsupported".into());
    state.set_media_detail("Media controls are not implemented in this build.".into());
    state.set_confirm_open(false);
    state.set_preview_open(false);
}

fn set_launcher_state(window: &ShellLauncher, apps: &[AppItem], query: &str, launcher_open: bool) {
    let state = window.global::<UiState>();
    state.set_development_preview(false);
    state.set_launcher_apps(ModelRc::new(VecModel::from(apps.to_vec())));
    state.set_launcher_query(query.into());
    state.set_launcher_open(launcher_open);
    state.set_launcher_footer_label(
        "Pinned and running applications · Windows Start remains available".into(),
    );
}

fn wire_callbacks(shell: &Rc<RefCell<WindowsShell>>) {
    let taskbar = shell.borrow().taskbar.clone_strong();
    {
        let weak = Rc::downgrade(shell);
        taskbar
            .global::<UiState>()
            .on_app_clicked(move |id: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.app_clicked(id.as_str());
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        taskbar
            .global::<UiState>()
            .on_app_hovered(move |id: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.app_hovered(id.as_str());
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        taskbar
            .global::<UiState>()
            .on_app_unhovered(move |id: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.app_unhovered(id.as_str());
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        taskbar.global::<UiState>().on_toggle_launcher(move || {
            if let Some(shell) = weak.upgrade()
                && let Ok(mut shell) = shell.try_borrow_mut()
            {
                shell.toggle_launcher();
            }
        });
    }
    {
        let weak = Rc::downgrade(shell);
        taskbar
            .global::<UiState>()
            .on_toggle_control_center(move || {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.toggle_control_center();
                }
            });
    }

    let control_center = shell.borrow().control_center.clone_strong();
    {
        let weak = Rc::downgrade(shell);
        control_center
            .global::<UiState>()
            .on_close_control_center(move || {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.close_control_center();
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        control_center.global::<UiState>().on_quick_toggle_clicked(
            move |id: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.quick_toggle_clicked(id.as_str());
                }
            },
        );
    }
    {
        let weak = Rc::downgrade(shell);
        control_center
            .global::<UiState>()
            .on_open_settings(move |page: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.open_settings(page.as_str());
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        control_center.global::<UiState>().on_power_requested(
            move |action: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.power_requested(action.as_str());
                }
            },
        );
    }
    {
        let weak = Rc::downgrade(shell);
        control_center.global::<UiState>().on_quit_shell(move || {
            if let Some(shell) = weak.upgrade()
                && let Ok(mut shell) = shell.try_borrow_mut()
            {
                shell.quit();
            }
        });
    }
    {
        let weak = Rc::downgrade(shell);
        control_center
            .global::<UiState>()
            .on_restore_native_taskbar(move || {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    if shell.replacement_active {
                        shell.restore_taskbar_replacement("Control Center");
                    } else {
                        // Even outside replacement mode this is a safe explicit
                        // recovery action: show the native taskbar immediately.
                        if let Err(error) = taskbar_replacement::restore_all_native_taskbars() {
                            tracing::warn!(%error, "manual taskbar restore failed");
                        }
                    }
                    shell.update_layout();
                    shell.apply();
                }
            });
    }

    let launcher = shell.borrow().launcher.clone_strong();
    {
        let weak = Rc::downgrade(shell);
        launcher
            .global::<UiState>()
            .on_launcher_search(move |query: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.launcher_search(query.as_str());
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        launcher
            .global::<UiState>()
            .on_launcher_app_clicked(move |id: slint::SharedString| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.launcher_app_clicked(id.as_str());
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        launcher.global::<UiState>().on_close_launcher(move || {
            if let Some(shell) = weak.upgrade()
                && let Ok(mut shell) = shell.try_borrow_mut()
            {
                shell.close_launcher();
            }
        });
    }

    let preview = shell.borrow().window_preview.clone_strong();
    {
        let weak = Rc::downgrade(shell);
        preview
            .global::<UiState>()
            .on_window_activated(move |id: i32| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.preview_window_activated(id);
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        preview
            .global::<UiState>()
            .on_window_closed(move |id: i32| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.preview_window_closed(id);
                }
            });
    }
    {
        let weak = Rc::downgrade(shell);
        preview.global::<UiState>().on_close_preview(move || {
            if let Some(shell) = weak.upgrade()
                && let Ok(mut shell) = shell.try_borrow_mut()
            {
                shell.close_preview();
            }
        });
    }
    {
        let weak = Rc::downgrade(shell);
        preview
            .global::<UiState>()
            .on_keep_preview_open(move |hovered: bool| {
                if let Some(shell) = weak.upgrade()
                    && let Ok(mut shell) = shell.try_borrow_mut()
                {
                    shell.preview_hovered_changed(hovered);
                }
            });
    }
}
