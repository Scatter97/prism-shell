use std::path::{Path, PathBuf};

pub use prism_core::PrismConfig;

#[derive(Clone, Debug)]
pub struct LoadedConfig {
    pub config: PrismConfig,
    pub path: Option<PathBuf>,
    /// Human-readable warning when a file existed but could not be used.
    pub warning: Option<String>,
}

/// Resolve the user configuration path without creating anything.
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PRISM_CONFIG") {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(|base| PathBuf::from(base).join("Prism").join("config.json"))
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(base.join("prism-shell").join("config.json"))
    }
}

/// Load configuration, falling back to safe defaults if the file is missing
/// or corrupt. Never overwrites a user's file.
pub fn load(path_override: Option<&Path>) -> LoadedConfig {
    let path = path_override
        .map(Path::to_path_buf)
        .or_else(default_config_path);

    let Some(path) = path else {
        return LoadedConfig {
            config: PrismConfig::default(),
            path: None,
            warning: Some("No configuration directory could be resolved.".to_owned()),
        };
    };

    if !path.exists() {
        return LoadedConfig {
            config: PrismConfig::default(),
            path: Some(path),
            warning: None,
        };
    }

    match std::fs::read_to_string(&path) {
        Ok(text) => match PrismConfig::from_json(&text) {
            Ok(config) => LoadedConfig {
                config,
                path: Some(path),
                warning: None,
            },
            Err(error) => LoadedConfig {
                config: PrismConfig::default(),
                path: Some(path.clone()),
                warning: Some(format!(
                    "Configuration at {} is invalid ({error}); using safe defaults",
                    path.display()
                )),
            },
        },
        Err(error) => LoadedConfig {
            config: PrismConfig::default(),
            path: Some(path.clone()),
            warning: Some(format!(
                "Could not read configuration at {} ({error}); using safe defaults",
                path.display()
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_path(tag: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "prism-config-{}-{}-{tag}.json",
            std::process::id(),
            id
        ))
    }

    #[test]
    fn missing_file_uses_defaults_without_error() {
        let path = temp_path("missing");
        let loaded = load(Some(&path));
        assert_eq!(loaded.config, PrismConfig::default());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn corrupt_file_falls_back_and_reports_a_warning() {
        let path = temp_path("corrupt");
        std::fs::write(&path, "{ definitely not json").unwrap();
        let loaded = load(Some(&path));
        assert_eq!(loaded.config, PrismConfig::default());
        assert!(
            loaded
                .warning
                .is_some_and(|warning| warning.contains("invalid"))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn valid_file_is_loaded() {
        let path = temp_path("valid");
        let mut config = PrismConfig::default();
        config.dock.use_24_hour_clock = false;
        std::fs::write(&path, config.to_json_pretty().unwrap()).unwrap();
        let loaded = load(Some(&path));
        assert!(!loaded.config.dock.use_24_hour_clock);
        assert!(loaded.warning.is_none());
        let _ = std::fs::remove_file(path);
    }
}
