//! Menu bar toggle states persisted across launches (Spec §7 "Menu bar item").

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Toggle states. Fields missing from the file fall back to their defaults,
/// so adding a toggle later does not reset the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub always_on_top: bool,
    /// Silence iPhone/Watch While Using Mac (Spec §8).
    pub silence_devices: bool,
    pub launch_at_login: bool,
    /// Minutes without input after which the Mac counts as not in use (Spec §8).
    pub idle_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: true,
            silence_devices: false,
            launch_at_login: true,
            idle_minutes: 5,
        }
    }
}

impl Settings {
    /// Idle threshold; at least one minute so a zero never blocks the Focus for good.
    pub fn idle_after(&self) -> std::time::Duration {
        std::time::Duration::from_secs(u64::from(self.idle_minutes.max(1)) * 60)
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
    write_atomic(path, json.as_bytes())
}

/// Writes a sibling file and renames it over `path`, so a failed write never
/// leaves a truncated file behind and file watchers never see a partial one.
/// The name is unique per process and call, and create_new makes sure no
/// other writer shares it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = unique_tmp_path(path);
    let result = write_new(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn unique_tmp_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
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
        assert_eq!(s.idle_minutes, 5);
    }

    #[test]
    fn idle_threshold_is_at_least_a_minute() {
        let s = |idle_minutes| Settings {
            idle_minutes,
            ..Settings::default()
        };
        assert_eq!(s(5).idle_after().as_secs(), 300);
        assert_eq!(s(0).idle_after().as_secs(), 60);
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
            idle_minutes: 7,
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), (s, None));
        assert_no_tmp_files(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn assert_no_tmp_files(dir: &Path) {
        let settings_dir = path_in(dir);
        let leftovers: Vec<_> = std::fs::read_dir(settings_dir.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    }

    #[test]
    fn concurrent_saves_never_corrupt_the_file() {
        let dir = temp_dir("concurrent");
        let path = path_in(&dir);
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let s = Settings {
                        always_on_top: i % 2 == 0,
                        silence_devices: i % 3 == 0,
                        launch_at_login: i % 5 == 0,
                        idle_minutes: i,
                    };
                    for _ in 0..20 {
                        save(&path, &s).unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // Whatever write won, the file must parse cleanly
        assert_eq!(load(&path).1, None);
        assert_no_tmp_files(&dir);
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
