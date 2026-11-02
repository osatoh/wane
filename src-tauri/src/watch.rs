//! Watches schedule.toml and reloads it on change (Spec §3 Storage, §6).

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::schedule::{self, Schedule, ScheduleError};

/// Last valid schedule plus the error from the latest load, if it failed.
#[derive(Debug, Default)]
pub struct ScheduleState {
    schedule: Option<Schedule>,
    error: Option<String>,
}

impl ScheduleState {
    pub fn schedule(&self) -> Option<&Schedule> {
        self.schedule.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Applies a load result. An error keeps the last valid schedule.
    pub fn apply(&mut self, loaded: Result<Schedule, ScheduleError>) {
        match loaded {
            Ok(s) => {
                self.schedule = Some(s);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}

/// Wait after the first event so a burst from one save (write + rename) reloads once.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// Calls `on_change` once right after registering, then after `path` is created, modified, renamed or removed.
///
/// Watches the parent directory, not the file: editors that save by writing a
/// temp file and renaming it over the original replace the inode, which would
/// silently end a watch on the file itself.
/// The returned watcher must be kept alive for watching to continue.
pub fn watch(
    path: &Path,
    on_change: impl Fn() + Send + 'static,
) -> notify::Result<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(tx)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    watcher.watch(dir, RecursiveMode::NonRecursive)?;

    let target = path.to_path_buf();
    std::thread::spawn(move || {
        // Catch a save that landed between the caller's initial load and registration
        on_change();
        while let Ok(first) = rx.recv() {
            let mut relevant = touches(&first, &target);
            std::thread::sleep(DEBOUNCE);
            while let Ok(ev) = rx.try_recv() {
                relevant |= touches(&ev, &target);
            }
            if relevant {
                on_change();
            }
        }
    });
    Ok(watcher)
}

fn touches(ev: &notify::Result<notify::Event>, target: &Path) -> bool {
    let Ok(ev) = ev else { return false };
    // Events were dropped or coalesced; the file may have changed
    if ev.need_rescan() {
        return true;
    }
    if ev.kind.is_access() {
        return false;
    }
    // FSEvents may report canonical paths (/private/var vs /var), so compare file names
    ev.paths
        .iter()
        .any(|p| p == target || p.file_name() == target.file_name())
}

/// Reloads the file into `state` (used by the watcher callback).
///
/// Does not recreate a missing file: editors may briefly remove it while
/// saving, and writing the default then could clobber the user's save.
pub fn reload(path: &Path, state: &mut ScheduleState) {
    let loaded = std::fs::read_to_string(path)
        .map_err(ScheduleError::from)
        .and_then(|src| schedule::parse(&src));
    state.apply(loaded);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    const VALID_A: &str =
        "[[entry]]\ntime = \"07:00\"\ntitle = \"a\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n";
    const VALID_B: &str =
        "[[entry]]\ntime = \"08:00\"\ntitle = \"a\"\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n";

    fn first_time(state: &ScheduleState) -> String {
        state.schedule().unwrap().entries()[0].time.to_string()
    }

    #[test]
    fn valid_load_replaces_schedule_and_clears_error() {
        let mut st = ScheduleState::default();
        st.apply(schedule::parse(""));
        assert!(st.error().is_some());
        st.apply(schedule::parse(VALID_A));
        assert_eq!(first_time(&st), "07:00");
        assert!(st.error().is_none());
    }

    #[test]
    fn invalid_load_keeps_last_valid_schedule() {
        let mut st = ScheduleState::default();
        st.apply(schedule::parse(VALID_A));
        st.apply(schedule::parse("[[entry]\n"));
        assert_eq!(first_time(&st), "07:00");
        let err = st.error().unwrap();
        assert!(err.starts_with("invalid TOML"), "{err}");
    }

    #[test]
    fn invalid_first_load_has_no_schedule() {
        let mut st = ScheduleState::default();
        st.apply(schedule::parse(""));
        assert!(st.schedule().is_none());
        assert!(st.error().is_some());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wane-watch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn wait_for(state: &Arc<Mutex<ScheduleState>>, pred: impl Fn(&ScheduleState) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if pred(&state.lock().unwrap()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out: {:?}", state.lock().unwrap());
    }

    fn start(dir: &Path) -> (PathBuf, Arc<Mutex<ScheduleState>>, RecommendedWatcher) {
        let path = dir.join("schedule.toml");
        fs::write(&path, VALID_A).unwrap();
        let state = Arc::new(Mutex::new(ScheduleState::default()));
        reload(&path, &mut state.lock().unwrap());
        let (p, s) = (path.clone(), state.clone());
        let w = watch(&path, move || reload(&p, &mut s.lock().unwrap())).unwrap();
        // Let FSEvents settle so the initial write is not reported late
        std::thread::sleep(Duration::from_millis(300));
        (path, state, w)
    }

    #[test]
    fn reloads_on_in_place_write() {
        let dir = temp_dir("inplace");
        let (path, state, _w) = start(&dir);
        fs::write(&path, VALID_B).unwrap();
        wait_for(&state, |s| first_time(s) == "08:00");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reloads_on_rename_over_original() {
        let dir = temp_dir("rename");
        let (path, state, _w) = start(&dir);
        let tmp = dir.join("schedule.toml.tmp");
        fs::write(&tmp, VALID_B).unwrap();
        fs::rename(&tmp, &path).unwrap();
        wait_for(&state, |s| first_time(s) == "08:00");
        // The watch survives the replaced inode
        fs::write(&path, "[[entry]\n").unwrap();
        wait_for(&state, |s| s.error().is_some());
        assert_eq!(first_time(&state.lock().unwrap()), "08:00");
        fs::write(&path, VALID_A).unwrap();
        wait_for(&state, |s| s.error().is_none() && first_time(s) == "07:00");
        fs::remove_dir_all(dir).unwrap();
    }
}
