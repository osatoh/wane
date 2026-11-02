// Pure validation for the schedule editor. Mirrors the file rules in
// src-tauri/src/schedule.rs (Spec §3); the backend validates again on save.

import type { Entry } from "./view";

// One editable row; same shape as a schedule entry.
export type Row = Entry;

export interface RowErrors {
  time: string | null;
  title: string | null;
}

export interface Validation {
  ok: boolean;
  rows: RowErrors[];
  // Error that belongs to no single row.
  form: string | null;
}

const TIME = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;

export function validateRows(rows: Row[]): Validation {
  const counts = new Map<string, number>();
  for (const r of rows) counts.set(r.time, (counts.get(r.time) ?? 0) + 1);

  const errors = rows.map((r): RowErrors => {
    let time: string | null = null;
    if (!TIME.test(r.time)) time = "Use HH:MM (24-hour)";
    else if ((counts.get(r.time) ?? 0) > 1) time = "Duplicate time";
    // \p{White_Space} matches Rust's char::is_whitespace, which the backend uses
    const title = /^\p{White_Space}*$/u.test(r.title) ? "Title is required" : null;
    return { time, title };
  });
  const form = rows.length < 2 ? "At least 2 entries are required" : null;
  const ok = form === null && errors.every((e) => e.time === null && e.title === null);
  return { ok, rows: errors, form };
}
