//! Menu bar toggle states persisted across launches (Spec §7 "Menu bar item").

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Toggle states. Fields missing from the file fall back to their defaults,
/// so adding a toggle later does not reset the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub always_on_top: bool,
    /// Silence iPhone/Watch While Using Mac (Spec §8). Behavior comes in #9.
    pub silence_devices: bool,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: true,
            silence_devices: false,
            launch_at_login: true,
        }
    }
}

/// Settings file under the OS data dir, next to `schedule.toml`.
pub fn path_in(data_dir: &Path) -> PathBuf {
    data_dir.join("Wane/settings.json")
}

/// Reads settings. A missing file yields the defaults; an unreadable or
/// malformed file yields the defaults and an error to report.
pub fn load(path: &Path) -> (Settings, Option<String>) {
    match std::fs::read_to_string(path) {
        Ok(src) => match serde_json::from_str(&src) {
            Ok(s) => (s, None),
            Err(e) => (
                Settings::default(),
                Some(format!("{}: {e}", path.display())),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), None),
        Err(e) => (
            Settings::default(),
            Some(format!("{}: {e}", path.display())),
        ),
    }
}

/// Writes settings, creating the parent directory if needed.
pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wane-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn lives_in_the_wane_directory() {
        assert_eq!(
            path_in(Path::new("/data")),
            Path::new("/data/Wane/settings.json")
        );
    }

    #[test]
    fn defaults_match_spec() {
        let s = Settings::default();
        assert!(s.always_on_top);
        assert!(!s.silence_devices);
        assert!(s.launch_at_login);
    }

    #[test]
    fn missing_file_yields_defaults_without_error() {
        let dir = temp_dir("missing");
        assert_eq!(load(&path_in(&dir)), (Settings::default(), None));
    }

    #[test]
    fn round_trips_through_save_and_load() {
        let dir = temp_dir("roundtrip");
        let path = path_in(&dir);
        let s = Settings {
            always_on_top: false,
            silence_devices: true,
            launch_at_login: false,
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), (s, None));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let dir = temp_dir("partial");
        let path = path_in(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"silence_devices": true}"#).unwrap();
        let (s, err) = load(&path);
        assert_eq!(err, None);
        assert_eq!(
            s,
            Settings {
                silence_devices: true,
                ..Settings::default()
            }
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_file_yields_defaults_with_error() {
        let dir = temp_dir("malformed");
        let path = path_in(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        let (s, err) = load(&path);
        assert_eq!(s, Settings::default());
        assert!(err.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
