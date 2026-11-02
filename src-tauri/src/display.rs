//! Contract with the frontend display (Spec §5): event names and payloads.
//!
//! Mirrored by `src/events.ts`; keep both sides in sync.

use serde::Serialize;
use tauri::Emitter;

use crate::model;
use crate::schedule::Entry;
use crate::watch::ScheduleState;

/// Backend → frontend: the current display state, payload [`StatePayload`].
pub const STATE_EVENT: &str = "wane://state";
/// Backend → frontend: the schedule error message, or `null` once the file is valid again.
pub const SCHEDULE_ERROR_EVENT: &str = "wane://schedule-error";
/// Frontend → backend: the page is listening and needs the current state.
pub const READY_EVENT: &str = "wane://ready";

/// One schedule entry as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EntryPayload {
    pub title: String,
    /// Start time as "HH:MM".
    pub time: String,
}

/// Payload of [`STATE_EVENT`]; matches `State` in `src/view.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatePayload {
    pub current: EntryPayload,
    pub next: EntryPayload,
    pub remaining_fraction: f64,
    pub remaining_secs: u32,
}

impl From<&Entry> for EntryPayload {
    fn from(e: &Entry) -> Self {
        Self {
            title: e.title.clone(),
            time: e.time.to_string(),
        }
    }
}

impl From<model::State<'_>> for StatePayload {
    fn from(s: model::State<'_>) -> Self {
        Self {
            current: s.current.into(),
            next: s.next.into(),
            remaining_fraction: s.remaining_fraction,
            remaining_secs: s.remaining_secs,
        }
    }
}

/// Seconds since local midnight, as [`model::state`] expects.
pub fn local_now_secs() -> u32 {
    use chrono::Timelike;
    chrono::Local::now().num_seconds_from_midnight()
}

/// Sends the state for the current time (if a valid schedule was ever loaded)
/// and the schedule error, so the frontend can draw from scratch.
pub fn emit_all(app: &tauri::AppHandle, state: &ScheduleState) {
    emit_state(app, state);
    if let Err(e) = app.emit(SCHEDULE_ERROR_EVENT, state.error()) {
        eprintln!("display: cannot emit {SCHEDULE_ERROR_EVENT}: {e}");
    }
}

/// Sends the state for the current time; nothing while no valid schedule exists.
pub fn emit_state(app: &tauri::AppHandle, state: &ScheduleState) {
    let Some(schedule) = state.schedule() else {
        return;
    };
    let payload = StatePayload::from(model::state(local_now_secs(), schedule));
    // Dev builds log each emit so the wiring can be checked from the terminal
    #[cfg(debug_assertions)]
    eprintln!("display: emit {STATE_EVENT} {payload:?}");
    if let Err(e) = app.emit(STATE_EVENT, payload) {
        eprintln!("display: cannot emit {STATE_EVENT}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::{DEFAULT_SCHEDULE, parse};

    #[test]
    fn state_payload_uses_the_frontend_field_names() {
        let schedule = parse(DEFAULT_SCHEDULE).unwrap();
        // 12:20, midway through lunch
        let payload = StatePayload::from(model::state((12 * 60 + 20) * 60, &schedule));
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "current": { "title": "Lunch", "time": "12:00" },
                "next": { "title": "Back to work", "time": "13:00" },
                "remainingFraction": json["remainingFraction"],
                "remainingSecs": 40 * 60,
            })
        );
        let fraction = json["remainingFraction"].as_f64().unwrap();
        assert!((fraction - 40.0 / 60.0).abs() < 1e-9);
    }
}
