//! Portable window eligibility and grouping rules.
//!
//! The Windows tracker fills `WindowFacts` from Win32 and uses these functions
//! to decide what belongs in the dock. Keeping the rules here makes the
//! filtering behavior testable without a Windows session.

use crate::naming::{friendly_app_name, normalized_exe_id, title_or_name};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowFacts {
    pub own_process: bool,
    pub visible: bool,
    pub cloaked: bool,
    pub tool_window: bool,
    pub app_window: bool,
    pub owned: bool,
    pub width: i32,
    pub height: i32,
    pub minimized: bool,
    pub class_name: String,
    pub process_image: Option<String>,
    pub title: String,
    pub hwnd: u64,
}

impl WindowFacts {
    pub fn is_uwp_host(&self) -> bool {
        self.process_image
            .as_deref()
            .is_some_and(|path| path.to_lowercase().ends_with("applicationframehost.exe"))
    }
}

const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "ApplicationManager_DesktopShellWindow",
    "MultitaskingViewFrame",
    "XamlExplorerHostIslandWindow",
];

/// Whether a top-level window is eligible for a taskbar entry.
///
/// This is deliberately conservative: a window we cannot inspect remains out
/// of the dock rather than appearing as a fake application. Known app windows
/// that are minimized are kept.
pub fn is_eligible(facts: &WindowFacts) -> bool {
    if facts.own_process || !facts.visible || facts.cloaked {
        return false;
    }
    if SHELL_CLASSES.contains(&facts.class_name.as_str()) {
        return false;
    }
    if facts.tool_window && !facts.app_window {
        return false;
    }
    if facts.owned && !facts.app_window {
        return false;
    }
    if !facts.minimized && (facts.width <= 0 || facts.height <= 0) {
        return false;
    }
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredApp {
    pub id: String,
    pub name: String,
    pub launch_target: Option<String>,
    pub icon_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredWindow {
    pub hwnd: u64,
    pub app_id: String,
    pub title: String,
    pub minimized: bool,
}

/// Group an eligible window into an application identity.
///
/// Classic Win32 and packaged apps with an explicit AUMID are grouped by
/// normalized executable path. `ApplicationFrameHost.exe` hosts multiple
/// unrelated UWP apps under one process, so each frame is kept separate using
/// its HWND until AUMID extraction is added. That is imperfect but never merges
/// unrelated UWP windows into one fake app.
pub fn group_window(facts: &WindowFacts) -> Option<(DiscoveredApp, DiscoveredWindow)> {
    if !is_eligible(facts) {
        return None;
    }

    let (id, mut name, launch_target, icon_source) = if facts.is_uwp_host() {
        let name = friendly_app_name("applicationframehost.exe");
        (format!("uwp:{}", facts.hwnd), name.clone(), None, None)
    } else if let Some(path) = facts.process_image.as_deref() {
        let name = friendly_app_name(path);
        (
            normalized_exe_id(path),
            name,
            Some(path.to_owned()),
            Some(path.to_owned()),
        )
    } else {
        // Elevated/protected processes may deny image queries. Keep the window
        // visible by HWND rather than inventing a process identity.
        let name = "Application".to_owned();
        (format!("hwnd:{}", facts.hwnd), name, None, None)
    };

    let title = title_or_name(&facts.title, &name);
    if facts.is_uwp_host() {
        name = title.clone();
    }
    Some((
        DiscoveredApp {
            id: id.clone(),
            name,
            launch_target,
            icon_source,
        },
        DiscoveredWindow {
            hwnd: facts.hwnd,
            app_id: id,
            title,
            minimized: facts.minimized,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> WindowFacts {
        WindowFacts {
            own_process: false,
            visible: true,
            cloaked: false,
            tool_window: false,
            app_window: false,
            owned: false,
            width: 1200,
            height: 800,
            minimized: false,
            class_name: "CabinetWClass".into(),
            process_image: Some(r"C:\Windows\explorer.exe".into()),
            title: "Home".into(),
            hwnd: 42,
        }
    }

    #[test]
    fn normal_explorer_window_is_grouped_by_path() {
        let (app, window) = group_window(&facts()).unwrap();
        assert_eq!(app.id, r"exe:c:\windows\explorer.exe");
        assert_eq!(app.name, "File Explorer");
        assert_eq!(window.title, "Home");
        assert!(app.launch_target.is_some());
    }

    #[test]
    fn shell_and_tool_windows_are_filtered() {
        let mut shell = facts();
        shell.class_name = "Shell_TrayWnd".into();
        assert!(!is_eligible(&shell));

        let mut tool = facts();
        tool.tool_window = true;
        assert!(!is_eligible(&tool));
        tool.app_window = true;
        assert!(is_eligible(&tool));
    }

    #[test]
    fn cloaked_invisible_and_own_windows_are_filtered() {
        for mutate in [
            |f: &mut WindowFacts| f.cloaked = true,
            |f: &mut WindowFacts| f.visible = false,
            |f: &mut WindowFacts| f.own_process = true,
            |f: &mut WindowFacts| f.width = 0,
        ] {
            let mut f = facts();
            mutate(&mut f);
            assert!(!is_eligible(&f));
        }
    }

    #[test]
    fn minimized_windows_are_kept() {
        let mut f = facts();
        f.minimized = true;
        f.width = 0;
        f.height = 0;
        assert!(is_eligible(&f));
    }

    #[test]
    fn application_frame_host_windows_are_not_merged_by_pid() {
        let mut first = facts();
        first.process_image = Some(r"C:\Windows\System32\ApplicationFrameHost.exe".into());
        first.hwnd = 100;
        first.title = "Settings".into();
        let mut second = first.clone();
        second.hwnd = 101;
        second.title = "Calculator".into();
        let (a, _) = group_window(&first).unwrap();
        let (b, _) = group_window(&second).unwrap();
        assert_ne!(a.id, b.id);
        assert!(a.id.starts_with("uwp:"));
        assert!(b.id.starts_with("uwp:"));
    }

    #[test]
    fn missing_process_image_keeps_an_honest_hwnd_identity() {
        let mut f = facts();
        f.process_image = None;
        let (app, _) = group_window(&f).unwrap();
        assert_eq!(app.id, "hwnd:42");
        assert_eq!(app.name, "Application");
        assert!(app.launch_target.is_none());
    }

    #[test]
    fn blank_titles_use_the_application_name() {
        let mut f = facts();
        f.title.clear();
        let (_, window) = group_window(&f).unwrap();
        assert_eq!(window.title, "File Explorer");
    }
}
