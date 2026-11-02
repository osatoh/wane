// Names and payloads of the events exchanged with the Rust backend.

// Listened to by the current display (Spec §5).
export const STATE_EVENT = "wane://state";
export const SCHEDULE_ERROR_EVENT = "wane://schedule-error";
// Emitted once listening, so the backend can send the current state without racing the page load.
export const READY_EVENT = "wane://ready";

// Payload of STATE_EVENT. No Rust struct serializes this yet: the backend must emit
// `model::State` in this shape (camelCase keys, entry times as "HH:MM").
export type { State as StatePayload } from "./view";
// Payload of SCHEDULE_ERROR_EVENT: the message, or null to clear it.
export type ScheduleErrorPayload = string | null;

// The two backend events below are typed for wiring the backend to the display; the frontend does not listen to them yet.

// Emitted by the backend after the schedule file changes.
// Mirrors `watch::UPDATED_EVENT` / `watch::UpdatedEvent` in src-tauri/src/watch.rs.
export const SCHEDULE_UPDATED_EVENT = "schedule-updated";
export interface ScheduleUpdatedPayload {
  // Short message to show in the window; null when the file is valid (Rust `Option<String>`).
  error: string | null;
}

// Emitted by the backend on each minute and on wake / clock / time zone changes.
// Mirrors `tick::TICK_EVENT` in src-tauri/src/tick.rs; its payload is Rust `()`, which serializes as null.
export const TICK_EVENT = "reevaluate";
export type TickPayload = null;
