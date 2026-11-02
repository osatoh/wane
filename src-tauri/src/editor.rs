//! In-app schedule editor window (Spec §3 "Editor", §7).
//!
//! Commands are mirrored by `src/events.ts`; keep both sides in sync.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State, WebviewUrl, WebviewWindowBuilder};

use crate::display::EntryPayload;
use crate::schedule::{self, Schedule};
use crate::settings;
use crate::watch::ScheduleState;

/// Label of the editor window. Only one exists at a time.
pub const EDITOR: &str = "editor";

/// Payload of the `load_schedule` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub entries: Vec<EntryPayload>,
    /// Why the file could not be used, when `entries` come from the last
    /// valid schedule instead of the file.
    pub file_error: Option<String>,
    /// Fingerprint of the file the entries were read from; passed back on save
    /// to detect edits made on disk while the editor was open.
    pub revision: String,
}

/// Error of the `save_schedule` command, serialized as `{ kind, message }`.
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum SaveError {
    /// The entries break a schedule rule; nothing was written.
    Invalid(String),
    /// The file changed on disk since it was loaded; nothing was written.
    Conflict,
    Io(String),
}

/// One row as the editor sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct RowInput {
    pub time: String,
    pub title: String,
}

/// Path of `schedule.toml`, managed so the commands can reach it.
pub struct SchedulePath(pub PathBuf);

/// Opens the editor window, or focuses it if it is already open.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(EDITOR) {
        w.show()?;
        return w.set_focus();
    }
    let w = WebviewWindowBuilder::new(app, EDITOR, WebviewUrl::App("editor.html".into()))
        .title("Edit Schedule")
        .inner_size(420.0, 520.0)
        .min_inner_size(360.0, 300.0)
        .build()?;
    #[cfg(debug_assertions)]
    eprintln!("editor: window opened");
    w.set_focus()
}

/// Command: the schedule to edit. Async so the file I/O and the state lock
/// stay off the main thread.
#[tauri::command]
pub async fn load_schedule(
    path: State<'_, SchedulePath>,
    state: State<'_, Mutex<ScheduleState>>,
) -> Result<Snapshot, String> {
    // Clone the fallback so the lock is not held during file I/O
    let fallback = state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .schedule()
        .cloned();
    load(&path.0, fallback.as_ref())
}

/// Command: writes the rows. The file watcher then reloads the gauge (Spec §6).
#[tauri::command]
pub async fn save_schedule(
    path: State<'_, SchedulePath>,
    rows: Vec<RowInput>,
    revision: String,
    overwrite: bool,
) -> Result<String, SaveError> {
    let result = save(&path.0, rows, &revision, overwrite);
    #[cfg(debug_assertions)]
    eprintln!("editor: save {:?}", result.as_ref().map(|_| "ok"));
    result
}

/// Reads the file for the editor. If it is currently invalid, the editor
/// starts from `fallback` (the last valid schedule) so the user can fix it.
pub fn load(path: &Path, fallback: Option<&Schedule>) -> Result<Snapshot, String> {
    let bytes = read_bytes(path).map_err(|e| e.to_string())?;
    // Same outcome as the watcher: invalid UTF-8 or TOML means "use the last valid schedule"
    let parsed = std::str::from_utf8(&bytes)
        .map_err(|e| format!("not valid UTF-8: {e}"))
        .and_then(|src| schedule::parse(src).map_err(|e| e.to_string()));
    let (entries, file_error) = match parsed {
        Ok(s) => (payload(&s), None),
        Err(e) => (fallback.map(payload).unwrap_or_default(), Some(e)),
    };
    Ok(Snapshot {
        entries,
        file_error,
        revision: revision_of(&bytes),
    })
}

/// Validates and writes the rows. Unless `overwrite` is set, refuses when the
/// file no longer matches `revision`. Returns the new revision.
pub fn save(
    path: &Path,
    rows: Vec<RowInput>,
    revision: &str,
    overwrite: bool,
) -> Result<String, SaveError> {
    let schedule = schedule::from_entries(rows.into_iter().map(|r| (r.time, r.title)).collect())
        .map_err(|e| SaveError::Invalid(e.to_string()))?;
    let io = |e: io::Error| SaveError::Io(e.to_string());
    if !overwrite && revision_of(&read_bytes(path).map_err(io)?) != revision {
        return Err(SaveError::Conflict);
    }
    let src = schedule::to_toml(&schedule);
    settings::write_atomic(path, src.as_bytes()).map_err(io)?;
    Ok(revision_of(src.as_bytes()))
}

/// File contents, or nothing when the file does not exist.
fn read_bytes(path: &Path) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        r => r,
    }
}

/// Revision for the contents: the bytes as hex. Lossless, so any change on
/// disk is detected, including invalid UTF-8 that a text form would hide.
fn revision_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn payload(s: &Schedule) -> Vec<EntryPayload> {
    s.entries().iter().map(EntryPayload::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const VALID: &str = "[[entry]]\ntime = \"07:00\"\ntitle = \"a\"\n\n[[entry]]\ntime = \"10:00\"\ntitle = \"b\"\n";

    /// Revision of a file holding `VALID`.
    fn vrev() -> String {
        revision_of(VALID.as_bytes())
    }

    fn temp_file(name: &str, src: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("wane-editor-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("schedule.toml");
        fs::write(&path, src).unwrap();
        (dir, path)
    }

    fn row(time: &str, title: &str) -> RowInput {
        RowInput {
            time: time.into(),
            title: title.into(),
        }
    }

    fn entry(time: &str, title: &str) -> EntryPayload {
        EntryPayload {
            time: time.into(),
            title: title.into(),
        }
    }

    #[test]
    fn load_returns_entries_and_file_contents() {
        let (dir, path) = temp_file("load", VALID);
        let snap = load(&path, None).unwrap();
        assert_eq!(snap.entries, vec![entry("07:00", "a"), entry("10:00", "b")]);
        assert_eq!(snap.revision, vrev());
        assert_eq!(snap.file_error, None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_of_invalid_file_starts_from_last_valid_schedule() {
        let (dir, path) = temp_file("load-invalid", "[[entry]\n");
        let last = schedule::parse(VALID).unwrap();
        let snap = load(&path, Some(&last)).unwrap();
        assert_eq!(snap.entries.len(), 2);
        assert_eq!(snap.revision, revision_of(b"[[entry]\n"));
        assert!(snap.file_error.unwrap().starts_with("invalid TOML"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_of_non_utf8_file_falls_back_and_can_be_saved_over() {
        let (dir, path) = temp_file("utf8", "");
        // Otherwise valid TOML with a bad byte inside a title
        let mut bytes = VALID.as_bytes().to_vec();
        let at = VALID.find("\"a\"").unwrap() + 1;
        bytes[at] = 0xff;
        fs::write(&path, &bytes).unwrap();
        let last = schedule::parse(&VALID.replace("07:00", "06:00")).unwrap();
        let snap = load(&path, Some(&last)).unwrap();
        assert_eq!(snap.entries[0], entry("06:00", "a"));
        let rows = vec![row("08:00", "x"), row("09:00", "y")];
        save(&path, rows, &snap.revision, false).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_detects_a_change_between_two_invalid_utf8_files() {
        let (dir, path) = temp_file("utf8-conflict", "");
        fs::write(&path, b"\xff").unwrap();
        let snap = load(&path, None).unwrap();
        fs::write(&path, b"\xfe").unwrap();
        let rows = vec![row("08:00", "x"), row("09:00", "y")];
        assert_eq!(
            save(&path, rows, &snap.revision, false),
            Err(SaveError::Conflict)
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_writes_sorted_entries() {
        let (dir, path) = temp_file("save", VALID);
        let rows = vec![row("12:00", "Lunch"), row("08:00", "Wake up")];
        let rev = save(&path, rows, &vrev(), false).unwrap();
        let written = fs::read_to_string(&path).unwrap();
        assert_eq!(rev, revision_of(written.as_bytes()));
        let s = schedule::parse(&written).unwrap();
        assert_eq!(s.entries()[0].title, "Wake up");
        // No temp file is left next to the schedule
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_rejects_invalid_rows_without_writing() {
        let (dir, path) = temp_file("invalid", VALID);
        let err = save(
            &path,
            vec![row("09:00", "a"), row("09:00", "b")],
            &vrev(),
            false,
        );
        assert_eq!(err, Err(SaveError::Invalid("duplicate time 09:00".into())));
        assert_eq!(fs::read_to_string(&path).unwrap(), VALID);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_refuses_when_file_changed_on_disk_unless_overwriting() {
        let (dir, path) = temp_file("conflict", VALID);
        let external = VALID.replace("07:00", "06:00");
        fs::write(&path, &external).unwrap();
        let rows = || vec![row("08:00", "x"), row("09:00", "y")];
        assert_eq!(
            save(&path, rows(), &vrev(), false),
            Err(SaveError::Conflict)
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), external);
        save(&path, rows(), &vrev(), true).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("08:00"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_error_serializes_with_kind_and_message() {
        let json = serde_json::to_value(SaveError::Invalid("x".into())).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "invalid", "message": "x" })
        );
        let json = serde_json::to_value(SaveError::Conflict).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "conflict" }));
    }
}
