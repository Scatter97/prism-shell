use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CONFIG_VERSION: u32 = 1;
pub const MAX_PINNED_APPS: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PrismConfig {
    pub version: u32,
    /// Stable application identifiers. The Windows backend resolves these to
    /// launch targets; the preview maps them to its designed fixture catalog.
    pub pinned_apps: Vec<String>,
    pub dock: DockConfig,
    pub accessibility: AccessibilityConfig,
    /// Shell-integration experiments. Every field defaults to `false`; current
    /// Windows builds must remain safe coexist-with-Explorer overlays unless the
    /// owner explicitly opts in after real Windows recovery validation.
    pub experimental: ExperimentalConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DockConfig {
    pub show_date: bool,
    pub use_24_hour_clock: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AccessibilityConfig {
    pub reduced_motion: bool,
}

/// Opt-in shell integrations. These are disabled by default and are not part
/// of the validated V1 behavior until tested on a real Windows 11 desktop.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExperimentalConfig {
    /// Hide the native Windows taskbar and use Prism as the visible taskbar.
    /// Requires the independent `prism-recovery.exe` watchdog next to the shell.
    pub taskbar_replacement: bool,
    /// Intercept a standalone left/right Windows key release and toggle the
    /// Prism launcher. Combinations (Win+E, Win+L, ...) are always passed
    /// through unchanged.
    pub windows_key_launcher: bool,
    /// Also register the fallback launcher shortcut. Registered automatically
    /// when the low-level Windows-key hook cannot be installed.
    pub windows_key_fallback_hotkey: bool,
    /// Fallback launcher hotkey. Supported modifiers are Ctrl, Alt, Shift, and
    /// Win; supported keys include A-Z, 0-9, Space, Tab, F1-F24, and arrows.
    pub windows_key_fallback_shortcut: String,
}

impl Default for ExperimentalConfig {
    fn default() -> Self {
        Self {
            taskbar_replacement: false,
            windows_key_launcher: false,
            windows_key_fallback_hotkey: false,
            windows_key_fallback_shortcut: "Ctrl+Alt+Space".to_owned(),
        }
    }
}

impl Default for PrismConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            pinned_apps: vec![
                "files".to_owned(),
                "browser".to_owned(),
                "terminal".to_owned(),
                "mail".to_owned(),
            ],
            dock: DockConfig::default(),
            accessibility: AccessibilityConfig::default(),
            experimental: ExperimentalConfig::default(),
        }
    }
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            show_date: true,
            use_24_hour_clock: true,
        }
    }
}

impl PrismConfig {
    /// Parse and validate user configuration.
    ///
    /// A structurally valid file with an unknown version is rejected rather
    /// than partially applied, preventing surprising destructive upgrades.
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    pub fn to_json_pretty(&self) -> Result<String, ConfigError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.pinned_apps.len() > MAX_PINNED_APPS {
            return Err(ConfigError::TooManyPinnedApps {
                count: self.pinned_apps.len(),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for id in &self.pinned_apps {
            let id = id.trim();
            if id.is_empty() {
                return Err(ConfigError::EmptyPinnedApp);
            }
            if !seen.insert(id) {
                return Err(ConfigError::DuplicatePinnedApp(id.to_owned()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("configuration version {found} is not supported (expected {CONFIG_VERSION})")]
    UnsupportedVersion { found: u32 },
    #[error("too many pinned applications ({count}); maximum is {MAX_PINNED_APPS}")]
    TooManyPinnedApps { count: usize },
    #[error("pinned application id cannot be empty")]
    EmptyPinnedApp,
    #[error("pinned application id '{0}' is duplicated")]
    DuplicatePinnedApp(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid_and_round_trips() {
        let config = PrismConfig::default();
        config.validate().unwrap();
        let json = config.to_json_pretty().unwrap();
        let restored = PrismConfig::from_json(&json).unwrap();
        assert_eq!(config, restored);
    }

    #[test]
    fn malformed_json_is_rejected() {
        let error = PrismConfig::from_json("{not-json}").unwrap_err();
        assert!(matches!(error, ConfigError::Json(_)));
    }

    #[test]
    fn unknown_version_is_rejected_instead_of_partially_applied() {
        let config = PrismConfig {
            version: 99,
            ..PrismConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(matches!(
            PrismConfig::from_json(&json).unwrap_err(),
            ConfigError::UnsupportedVersion { found: 99 }
        ));
    }

    #[test]
    fn duplicate_pins_are_rejected() {
        let config = PrismConfig {
            pinned_apps: vec!["files".into(), "files".into()],
            ..PrismConfig::default()
        };
        assert!(matches!(
            config.validate().unwrap_err(),
            ConfigError::DuplicatePinnedApp(id) if id == "files"
        ));
    }

    #[test]
    fn missing_optional_fields_fall_back_to_defaults() {
        let json = r#"{"version":1,"pinned_apps":["files"]}"#;
        let config = PrismConfig::from_json(json).unwrap();
        assert!(config.dock.show_date);
        assert!(!config.accessibility.reduced_motion);
        assert!(!config.experimental.taskbar_replacement);
        assert!(!config.experimental.windows_key_launcher);
        assert!(!config.experimental.windows_key_fallback_hotkey);
        assert_eq!(
            config.experimental.windows_key_fallback_shortcut,
            "Ctrl+Alt+Space"
        );
    }

    #[test]
    fn experimental_integrations_are_opt_in_and_round_trip() {
        let mut config = PrismConfig::default();
        assert_eq!(config.experimental, ExperimentalConfig::default());
        config.experimental.taskbar_replacement = true;
        config.experimental.windows_key_launcher = true;
        let restored = PrismConfig::from_json(&config.to_json_pretty().unwrap()).unwrap();
        assert!(restored.experimental.taskbar_replacement);
        assert!(restored.experimental.windows_key_launcher);
        assert!(!restored.experimental.windows_key_fallback_hotkey);
    }
}
