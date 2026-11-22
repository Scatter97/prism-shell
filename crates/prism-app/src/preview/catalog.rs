//! Deterministic application fixtures for the Linux development preview.
//!
//! These are the only application identities the preview can show. The names,
//! window titles, and capabilities are fictional and clearly marked as mock
//! data by the UI. None of this is used by the Windows backend.

use prism_core::{AppDefinition, DockState, IconKey};
use prism_platform::PreviewScenario;

struct FixtureApp {
    definition: AppDefinition,
    windows: &'static [&'static str],
    active: bool,
}

impl FixtureApp {
    fn pinned(
        id: &str,
        name: &str,
        icon: IconKey,
        order: u32,
        windows: &'static [&'static str],
        active: bool,
    ) -> Self {
        Self {
            definition: AppDefinition::pinned(id, name, icon, order),
            windows,
            active,
        }
    }

    fn running(
        id: &str,
        name: &str,
        icon: IconKey,
        windows: &'static [&'static str],
        active: bool,
    ) -> Self {
        Self {
            definition: AppDefinition::running(id, name, icon),
            windows,
            active,
        }
    }
}

/// Build the dock model for one fixture. Window ids and activation order are
/// deterministic, so screenshots and interaction tests are repeatable.
pub fn build_dock(scenario: PreviewScenario) -> DockState {
    let fixtures = fixtures_for(scenario);
    let mut dock = DockState::new(fixtures.iter().map(|app| app.definition.clone()));

    let mut active_window = None;

    for fixture in &fixtures {
        let Some(first_title) = fixture.windows.first() else {
            continue;
        };
        let first = dock
            .record_window_opened(&fixture.definition.id, *first_title)
            .expect("fixture app exists in dock");
        for title in &fixture.windows[1..] {
            dock.record_window_opened(&fixture.definition.id, *title);
        }
        if fixture.active {
            active_window = Some(first);
        }
    }

    if let Some(window) = active_window {
        dock.record_activated(window);
    }

    dock
}

fn fixtures_for(scenario: PreviewScenario) -> Vec<FixtureApp> {
    match scenario {
        PreviewScenario::Laptop | PreviewScenario::Faults => vec![
            FixtureApp::pinned("files", "Files", IconKey::Files, 0, &[], false),
            FixtureApp::pinned(
                "browser",
                "Browser",
                IconKey::Browser,
                1,
                &["Prism — Overview", "Design notes"],
                true,
            ),
            FixtureApp::pinned(
                "terminal",
                "Terminal",
                IconKey::Terminal,
                2,
                &["prism-shell — zsh"],
                false,
            ),
            FixtureApp::pinned("mail", "Mail", IconKey::Mail, 3, &[], false),
            FixtureApp::running(
                "music",
                "Music",
                IconKey::Music,
                &["Night Signals — Lumen Fields"],
                false,
            ),
        ],
        PreviewScenario::Desktop => vec![
            FixtureApp::pinned(
                "files",
                "Files",
                IconKey::Files,
                0,
                &["Home", "Projects"],
                true,
            ),
            FixtureApp::pinned(
                "browser",
                "Browser",
                IconKey::Browser,
                1,
                &["Prism — Overview"],
                false,
            ),
            FixtureApp::pinned("terminal", "Terminal", IconKey::Terminal, 2, &[], false),
            FixtureApp::pinned("mail", "Mail", IconKey::Mail, 3, &[], false),
            FixtureApp::running("notes", "Notes", IconKey::Notes, &["Launch notes"], false),
        ],
        PreviewScenario::Minimal => vec![
            FixtureApp::pinned("files", "Files", IconKey::Files, 0, &[], false),
            FixtureApp::pinned(
                "browser",
                "Browser",
                IconKey::Browser,
                1,
                &["Prism — Overview"],
                false,
            ),
            FixtureApp::pinned(
                "terminal",
                "Terminal",
                IconKey::Terminal,
                2,
                &["prism-shell — zsh"],
                true,
            ),
        ],
        PreviewScenario::ManyApps => vec![
            FixtureApp::pinned(
                "files",
                "Files",
                IconKey::Files,
                0,
                &["Home", "Projects"],
                false,
            ),
            FixtureApp::pinned(
                "browser",
                "Browser",
                IconKey::Browser,
                1,
                &["Prism — Overview", "Design notes", "Journal"],
                true,
            ),
            FixtureApp::pinned(
                "terminal",
                "Terminal",
                IconKey::Terminal,
                2,
                &["prism-shell — zsh"],
                false,
            ),
            FixtureApp::pinned("mail", "Mail", IconKey::Mail, 3, &["Inbox"], false),
            FixtureApp::pinned(
                "music",
                "Music",
                IconKey::Music,
                4,
                &["Night Signals"],
                false,
            ),
            FixtureApp::pinned(
                "notes",
                "Notes",
                IconKey::Notes,
                5,
                &["Launch notes"],
                false,
            ),
            FixtureApp::pinned(
                "calendar",
                "Calendar",
                IconKey::Generic,
                6,
                &["November 2026"],
                false,
            ),
            FixtureApp::pinned(
                "photos",
                "Photos",
                IconKey::Generic,
                7,
                &["Aurora set"],
                false,
            ),
            FixtureApp::pinned(
                "reader",
                "Reader",
                IconKey::Generic,
                8,
                &["Design systems"],
                false,
            ),
            FixtureApp::pinned(
                "maps",
                "Maps",
                IconKey::Generic,
                9,
                &["Office route"],
                false,
            ),
            FixtureApp::running(
                "studio",
                "Studio",
                IconKey::Generic,
                &["Untitled session"],
                false,
            ),
            FixtureApp::running("chat", "Chat", IconKey::Generic, &["Design team"], false),
            FixtureApp::running("tasks", "Tasks", IconKey::Generic, &["This week"], false),
            FixtureApp::running("weather", "Weather", IconKey::Generic, &["Forecast"], false),
            FixtureApp::running("archive", "Archive", IconKey::Generic, &["2025"], false),
            FixtureApp::running("clock", "Clock", IconKey::Generic, &["World clock"], false),
            FixtureApp::running("camera", "Camera", IconKey::Generic, &["Camera"], false),
            FixtureApp::running("store", "Store", IconKey::Generic, &["Featured"], false),
            FixtureApp::running("wallet", "Wallet", IconKey::Generic, &["Activity"], false),
            FixtureApp::running(
                "podcasts",
                "Podcasts",
                IconKey::Generic,
                &["Subscriptions"],
                false,
            ),
            FixtureApp::running("books", "Books", IconKey::Generic, &["Library"], false),
            FixtureApp::running(
                "translate",
                "Translate",
                IconKey::Generic,
                &["Translate"],
                false,
            ),
            FixtureApp::running("health", "Health", IconKey::Generic, &["Today"], false),
            FixtureApp::running("news", "News", IconKey::Generic, &["Briefing"], false),
            FixtureApp::running("journal", "Journal", IconKey::Generic, &["Entries"], false),
            FixtureApp::running("recipes", "Recipes", IconKey::Generic, &["Dinner"], false),
            FixtureApp::running("garden", "Garden", IconKey::Generic, &["Plan"], false),
            FixtureApp::running("radio", "Radio", IconKey::Generic, &["Stations"], false),
            FixtureApp::running(
                "theater",
                "Theater",
                IconKey::Generic,
                &["Watchlist"],
                false,
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn laptop_fixture_has_grouped_windows_and_one_active_app() {
        let dock = build_dock(PreviewScenario::Laptop);
        let browser = dock.app(&"browser".into()).expect("browser fixture exists");
        assert_eq!(browser.windows.len(), 2);
        assert_eq!(dock.active_app().map(|id| id.as_str()), Some("browser"));
    }

    #[test]
    fn every_scenario_has_running_apps_and_no_orphan_active_window() {
        for scenario in PreviewScenario::ALL {
            let dock = build_dock(scenario);
            assert!(
                dock.apps().iter().any(|app| app.is_running()),
                "{scenario:?}"
            );
            if let Some(active) = dock.active_window() {
                let found = dock
                    .apps()
                    .iter()
                    .any(|app| app.windows.iter().any(|window| window.id == active));
                assert!(found, "{scenario:?}");
            }
        }
    }

    #[test]
    fn many_apps_fixture_exceeds_a_1280_dock() {
        let dock = build_dock(PreviewScenario::ManyApps);
        let fit = prism_core::fit_dock_apps(dock.apps().len(), 1280.0);
        assert!(fit.hidden_apps > 0);
    }
}
