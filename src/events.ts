// Names and payloads of the events exchanged with the Rust backend.

// Listened to by the current display (Spec §5).
export const STATE_EVENT = "wane://state";
export const SCHEDULE_ERROR_EVENT = "wane://schedule-error";
// Emitted once listening, so the backend can send the current state without racing the page load.
export const READY_EVENT = "wane://ready";

// Mirrors the constants and payloads in src-tauri/src/display.rs; keep both sides in sync.

// Payload of STATE_EVENT. Serialized by Rust `display::StatePayload`
// (camelCase keys, entry times as "HH:MM").
export type { State as StatePayload } from "./view";
// Payload of SCHEDULE_ERROR_EVENT: the message, or null to clear it.
export type ScheduleErrorPayload = string | null;

