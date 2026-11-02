// Names and payloads of the events exchanged with the Rust backend.
import type { Entry } from "./view";

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

// Commands of the schedule editor window (Spec §3), served by src-tauri/src/editor.rs.
export const LOAD_SCHEDULE_COMMAND = "load_schedule";
export const SAVE_SCHEDULE_COMMAND = "save_schedule";

// Result of LOAD_SCHEDULE_COMMAND. Serialized by Rust `editor::Snapshot`.
export interface ScheduleSnapshot {
  entries: Entry[];
  // Set when the file is invalid; `entries` are then the last valid schedule.
  fileError: string | null;
  // Opaque fingerprint of the file the entries came from; sent back on save to detect edits on disk.
  revision: string;
}
// Arguments of SAVE_SCHEDULE_COMMAND; it resolves to the new revision.
export interface SaveScheduleArgs {
  rows: Entry[];
  revision: string;
  // Write even if the file changed on disk since `revision`.
  overwrite: boolean;
}
// Rejection of SAVE_SCHEDULE_COMMAND. Serialized by Rust `editor::SaveError`.
export type SaveScheduleError =
  | { kind: "invalid"; message: string }
  | { kind: "conflict" }
  | { kind: "io"; message: string };
