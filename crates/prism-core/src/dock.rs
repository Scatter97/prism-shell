use crate::ids::{AppId, IconKey, WindowId};

/// Static description of an application that can appear in the dock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDefinition {
    pub id: AppId,
    pub name: String,
    pub icon: IconKey,
    /// Human-readable launch target. Windows will use an executable path,
    /// AppUserModelID, or shortcut path here.
    pub launch_target: String,
    /// `Some` for pinned entries; the number is the user-visible pin order.
    pub pin_order: Option<u32>,
}

impl AppDefinition {
    pub fn pinned(id: &str, name: &str, icon: IconKey, order: u32) -> Self {
        Self {
            id: AppId::new(id),
            name: name.to_owned(),
            icon,
            launch_target: format!("preview://{id}"),
            pin_order: Some(order),
        }
    }

    pub fn running(id: &str, name: &str, icon: IconKey) -> Self {
        Self {
            id: AppId::new(id),
            name: name.to_owned(),
            icon,
            launch_target: format!("preview://{id}"),
            pin_order: None,
        }
    }

    pub fn is_pinned(&self) -> bool {
        self.pin_order.is_some()
    }
}

/// A currently open top-level window belonging to an application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppWindow {
    pub id: WindowId,
    pub title: String,
    pub minimized: bool,
    /// Monotonic activation stamp used to activate the most recently used
    /// window when an application has several windows.
    pub last_active_seq: u64,
    /// Creation order. Used for stable preview ordering.
    pub open_seq: u64,
}

impl AppWindow {
    pub fn new(id: WindowId, title: impl Into<String>, open_seq: u64) -> Self {
        Self {
            id,
            title: title.into(),
            minimized: false,
            last_active_seq: 0,
            open_seq,
        }
    }
}

/// One dock entry. Pinned applications exist without windows; unpinned
/// entries only exist while the application is running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppEntry {
    pub definition: AppDefinition,
    pub windows: Vec<AppWindow>,
}

impl AppEntry {
    pub fn is_running(&self) -> bool {
        !self.windows.is_empty()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn most_recent_window(&self) -> Option<&AppWindow> {
        self.windows
            .iter()
            .max_by_key(|window| (window.last_active_seq, window.open_seq))
    }

    pub fn active_window(&self, active_window: Option<WindowId>) -> Option<&AppWindow> {
        let active_window = active_window?;
        self.windows
            .iter()
            .find(|window| window.id == active_window)
    }
}

/// A user click on a dock entry, resolved into an explicit intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppClickIntent {
    Launch(AppId),
    Activate(WindowId),
    Minimize(WindowId),
    /// The application is running but every window is minimized.
    Restore(WindowId),
}

/// Portable state for the dock's application model.
///
/// This type deliberately does not know how to launch or activate anything.
/// The application controller resolves intents against the platform adapter,
/// then feeds the result back with `record_*` methods.
#[derive(Clone, Debug)]
pub struct DockState {
    apps: Vec<AppEntry>,
    active_window: Option<WindowId>,
    activation_seq: u64,
    open_seq: u64,
    next_window_id: u64,
}

impl DockState {
    pub fn new(definitions: impl IntoIterator<Item = AppDefinition>) -> Self {
        let mut apps: Vec<AppEntry> = definitions
            .into_iter()
            .map(|definition| AppEntry {
                definition,
                windows: Vec::new(),
            })
            .collect();
        Self::sort_entries(&mut apps);
        Self {
            apps,
            active_window: None,
            activation_seq: 0,
            open_seq: 0,
            next_window_id: 1,
        }
    }

    pub fn apps(&self) -> &[AppEntry] {
        &self.apps
    }

    pub fn app(&self, id: &AppId) -> Option<&AppEntry> {
        self.apps.iter().find(|entry| &entry.definition.id == id)
    }

    pub fn app_mut(&mut self, id: &AppId) -> Option<&mut AppEntry> {
        self.apps
            .iter_mut()
            .find(|entry| &entry.definition.id == id)
    }

    pub fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    pub fn active_app(&self) -> Option<&AppId> {
        let active = self.active_window?;
        self.apps
            .iter()
            .find(|entry| entry.windows.iter().any(|window| window.id == active))
            .map(|entry| &entry.definition.id)
    }

    pub fn is_app_active(&self, id: &AppId) -> bool {
        self.active_app() == Some(id)
    }

    /// Resolve the documented single-click behavior for one dock entry.
    pub fn click_intent(&self, id: &AppId) -> Option<AppClickIntent> {
        let entry = self.app(id)?;
        if entry.windows.is_empty() {
            return Some(AppClickIntent::Launch(entry.definition.id.clone()));
        }
        if self.is_app_active(id)
            && let Some(window) = entry.active_window(self.active_window)
        {
            return Some(AppClickIntent::Minimize(window.id));
        }
        if let Some(window) = entry
            .windows
            .iter()
            .filter(|window| !window.minimized)
            .max_by_key(|window| (window.last_active_seq, window.open_seq))
        {
            return Some(AppClickIntent::Activate(window.id));
        }
        if let Some(window) = entry.most_recent_window() {
            return Some(AppClickIntent::Restore(window.id));
        }
        Some(AppClickIntent::Launch(entry.definition.id.clone()))
    }

    /// Registers a newly launched window and makes it the active window.
    pub fn record_window_opened(
        &mut self,
        app_id: &AppId,
        title: impl Into<String>,
    ) -> Option<WindowId> {
        let window_id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        self.open_seq += 1;
        self.activation_seq += 1;

        let open_seq = self.open_seq;
        let activation_seq = self.activation_seq;
        let entry = self.app_mut(app_id)?;
        entry.windows.push(AppWindow {
            id: window_id,
            title: title.into(),
            minimized: false,
            last_active_seq: activation_seq,
            open_seq,
        });
        self.active_window = Some(window_id);
        Some(window_id)
    }

    pub fn record_activated(&mut self, window_id: WindowId) {
        self.activation_seq += 1;
        let activation_seq = self.activation_seq;
        for entry in &mut self.apps {
            for window in &mut entry.windows {
                if window.id == window_id {
                    window.minimized = false;
                    window.last_active_seq = activation_seq;
                }
            }
        }
        self.active_window = Some(window_id);
    }

    pub fn record_minimized(&mut self, window_id: WindowId) {
        for entry in &mut self.apps {
            for window in &mut entry.windows {
                if window.id == window_id {
                    window.minimized = true;
                }
            }
        }
        if self.active_window == Some(window_id) {
            self.active_window = None;
        }
    }

    pub fn record_closed(&mut self, window_id: WindowId) {
        let mut removed_app = None;
        for entry in &mut self.apps {
            let before = entry.windows.len();
            entry.windows.retain(|window| window.id != window_id);
            if entry.windows.len() != before {
                removed_app = Some(entry.definition.id.clone());
                break;
            }
        }
        if self.active_window == Some(window_id) {
            self.active_window = None;
        }
        if let Some(app_id) = removed_app {
            let remove_entry = self
                .app(&app_id)
                .is_some_and(|entry| !entry.definition.is_pinned() && entry.windows.is_empty());
            if remove_entry {
                self.apps.retain(|entry| entry.definition.id != app_id);
            }
        }
    }

    /// Register a window discovered by the platform, using the platform's real
    /// stable window id (for Windows this is the HWND).
    ///
    /// Returns `false` when the application entry is not present; the caller
    /// must ensure running applications exist in the dock first.
    pub fn record_window_discovered(
        &mut self,
        app_id: &AppId,
        window_id: WindowId,
        title: impl Into<String>,
        minimized: bool,
    ) -> bool {
        let Some(index) = self
            .apps
            .iter()
            .position(|entry| &entry.definition.id == app_id)
        else {
            return false;
        };
        let title = title.into();
        let entry = &mut self.apps[index];
        if let Some(window) = entry
            .windows
            .iter_mut()
            .find(|window| window.id == window_id)
        {
            window.title = title;
            window.minimized = minimized;
            return true;
        }
        self.open_seq += 1;
        let open_seq = self.open_seq;
        entry.windows.push(AppWindow {
            id: window_id,
            title,
            minimized,
            last_active_seq: 0,
            open_seq,
        });
        true
    }

    /// Applies a best-effort reconciliation when the platform reports a window
    /// that is already gone. Safe to call repeatedly.
    pub fn record_window_missing(&mut self, window_id: WindowId) {
        self.record_closed(window_id);
    }

    /// Ensure a running application discovered by the platform is present,
    /// even when it was not pinned.
    pub fn ensure_running_app(&mut self, definition: AppDefinition) -> &mut AppEntry {
        if let Some(index) = self
            .apps
            .iter()
            .position(|entry| entry.definition.id == definition.id)
        {
            return &mut self.apps[index];
        }
        self.apps.push(AppEntry {
            definition,
            windows: Vec::new(),
        });
        Self::sort_entries(&mut self.apps);
        self.apps.last_mut().expect("dock always has the new entry")
    }

    pub fn windows_for(&self, app_id: &AppId) -> &[AppWindow] {
        self.app(app_id)
            .map(|entry| entry.windows.as_slice())
            .unwrap_or_default()
    }

    fn sort_entries(apps: &mut [AppEntry]) {
        apps.sort_by_key(|entry| match entry.definition.pin_order {
            Some(order) => (0_u8, order, entry.definition.name.to_lowercase()),
            None => (1_u8, u32::MAX, entry.definition.name.to_lowercase()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definitions() -> Vec<AppDefinition> {
        vec![
            AppDefinition::pinned("files", "Files", IconKey::Files, 0),
            AppDefinition::pinned("browser", "Browser", IconKey::Browser, 1),
            AppDefinition::pinned("terminal", "Terminal", IconKey::Terminal, 2),
            AppDefinition::running("music", "Music", IconKey::Music),
        ]
    }

    #[test]
    fn pinned_apps_are_ordered_before_running_apps() {
        let dock = DockState::new(definitions());
        let ids: Vec<_> = dock
            .apps()
            .iter()
            .map(|app| app.definition.id.as_str())
            .collect();
        assert_eq!(ids, ["files", "browser", "terminal", "music"]);
    }

    #[test]
    fn click_on_stopped_pinned_app_requests_launch() {
        let dock = DockState::new(definitions());
        assert_eq!(
            dock.click_intent(&"files".into()),
            Some(AppClickIntent::Launch("files".into()))
        );
    }

    #[test]
    fn windows_group_under_one_application() {
        let mut dock = DockState::new(definitions());
        let first = dock
            .record_window_opened(&"browser".into(), "Inbox")
            .unwrap();
        let second = dock
            .record_window_opened(&"browser".into(), "Documentation")
            .unwrap();
        assert_eq!(dock.windows_for(&"browser".into()).len(), 2);
        assert_eq!(dock.active_window(), Some(second));
        assert_ne!(first, second);
    }

    #[test]
    fn active_app_click_minimizes_and_clears_active_state() {
        let mut dock = DockState::new(definitions());
        let window = dock
            .record_window_opened(&"browser".into(), "Inbox")
            .unwrap();
        assert!(dock.is_app_active(&"browser".into()));
        assert_eq!(
            dock.click_intent(&"browser".into()),
            Some(AppClickIntent::Minimize(window))
        );
        dock.record_minimized(window);
        assert!(!dock.is_app_active(&"browser".into()));
        assert_eq!(
            dock.click_intent(&"browser".into()),
            Some(AppClickIntent::Restore(window))
        );
    }

    #[test]
    fn minimizing_one_of_two_windows_activates_the_visible_window() {
        let mut dock = DockState::new(definitions());
        let first = dock
            .record_window_opened(&"browser".into(), "First")
            .unwrap();
        let second = dock
            .record_window_opened(&"browser".into(), "Second")
            .unwrap();
        dock.record_activated(first);
        assert_eq!(dock.active_window(), Some(first));
        dock.record_minimized(first);
        assert_eq!(
            dock.click_intent(&"browser".into()),
            Some(AppClickIntent::Activate(second))
        );
    }

    #[test]
    fn all_minimized_windows_restore_most_recently_active() {
        let mut dock = DockState::new(definitions());
        let first = dock
            .record_window_opened(&"browser".into(), "First")
            .unwrap();
        let second = dock
            .record_window_opened(&"browser".into(), "Second")
            .unwrap();
        dock.record_activated(first);
        dock.record_minimized(first);
        dock.record_minimized(second);
        assert_eq!(
            dock.click_intent(&"browser".into()),
            Some(AppClickIntent::Restore(first))
        );
    }

    #[test]
    fn closing_last_unpinned_window_removes_entry() {
        let mut dock = DockState::new(definitions());
        let window = dock
            .record_window_opened(&"music".into(), "Now Playing")
            .unwrap();
        assert!(dock.app(&"music".into()).is_some());
        dock.record_closed(window);
        assert!(dock.app(&"music".into()).is_none());
    }

    #[test]
    fn closing_last_pinned_window_keeps_entry() {
        let mut dock = DockState::new(definitions());
        let window = dock.record_window_opened(&"files".into(), "Home").unwrap();
        dock.record_closed(window);
        let entry = dock.app(&"files".into()).expect("pinned entry stays");
        assert!(entry.windows.is_empty());
        assert!(entry.definition.is_pinned());
    }

    #[test]
    fn discovered_windows_keep_platform_ids_and_update_titles() {
        let mut dock = DockState::new(definitions());
        let id = WindowId::new(4242);
        assert!(dock.record_window_discovered(&"browser".into(), id, "First", false));
        assert!(dock.record_window_discovered(&"browser".into(), id, "Renamed", true));
        let windows = dock.windows_for(&"browser".into());
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, id);
        assert_eq!(windows[0].title, "Renamed");
        assert!(windows[0].minimized);
        assert!(!dock.record_window_discovered(&"missing".into(), id, "x", false));
    }

    #[test]
    fn missing_window_reconciliation_is_idempotent() {
        let mut dock = DockState::new(definitions());
        let window = dock.record_window_opened(&"music".into(), "Track").unwrap();
        dock.record_window_missing(window);
        dock.record_window_missing(window);
        assert!(dock.app(&"music".into()).is_none());
        assert_eq!(dock.active_window(), None);
    }

    #[test]
    fn running_app_is_added_without_pin() {
        let mut dock = DockState::new(definitions());
        dock.ensure_running_app(AppDefinition::running("notes", "Notes", IconKey::Notes));
        assert!(dock.app(&"notes".into()).is_some());
        assert!(!dock.app(&"notes".into()).unwrap().definition.is_pinned());
    }
}
