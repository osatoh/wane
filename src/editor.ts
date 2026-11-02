// Schedule editor window (Spec §3). Loads schedule.toml through the backend, edits the
// rows in memory, and writes them back on Save. The file watcher then updates the gauge.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { validateRows, type Row } from "./editorModel";
import {
  LOAD_SCHEDULE_COMMAND,
  SAVE_SCHEDULE_COMMAND,
  type SaveScheduleArgs,
  type SaveScheduleError,
  type ScheduleSnapshot,
} from "./events";

// Elements are static in editor.html, so a missing one is a bug.
function $<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing #${id}`);
  return el as T;
}
const form = $<HTMLFormElement>("editor");
const tbody = $("rows");
const formError = $("form-error");
const status = $("status");
const save = $<HTMLButtonElement>("save");
const conflict = $("conflict");
const overwrite = $<HTMLButtonElement>("overwrite");
const add = $<HTMLButtonElement>("add");

let rows: Row[] = [];
let revision = "";
// False until a snapshot has loaded; there is nothing safe to save before that.
let loaded = false;

function showError(el: HTMLElement, message: string | null): void {
  el.textContent = message ?? "";
  el.hidden = message === null;
}

// Rebuilds the table. Called on load and on add/delete; typing only revalidates,
// so the focused input is never replaced.
function renderRows(): void {
  tbody.replaceChildren(
    ...rows.map((row, i) => {
      const tr = document.createElement("tr");
      const cell = (field: keyof Row, label: string, placeholder: string) => {
        const td = document.createElement("td");
        td.className = field;
        const input = document.createElement("input");
        input.value = row[field];
        input.placeholder = placeholder;
        input.setAttribute("aria-label", `${label} ${i + 1}`);
        // Screen readers announce the inline error with its input
        const errId = `${field}-error-${i}`;
        input.setAttribute("aria-describedby", errId);
        input.addEventListener("input", () => {
          row[field] = input.value;
          validate();
        });
        const err = document.createElement("p");
        err.id = errId;
        err.className = "field-error";
        err.hidden = true;
        td.append(input, err);
        return td;
      };
      const del = document.createElement("button");
      del.type = "button";
      del.textContent = "Delete";
      del.setAttribute("aria-label", `Delete entry ${i + 1}`);
      del.addEventListener("click", () => {
        rows.splice(i, 1);
        renderRows();
      });
      const delCell = document.createElement("td");
      delCell.append(del);
      tr.append(cell("time", "Time", "HH:MM"), cell("title", "Title", "Title"), delCell);
      return tr;
    }),
  );
  validate();
}

// Shows inline errors and enables Save only for a valid schedule. Runs after every
// edit, so it also clears a "Saved" status that no longer describes the rows.
function validate(): void {
  status.textContent = "";
  const v = validateRows(rows);
  [...tbody.children].forEach((tr, i) => {
    for (const field of ["time", "title"] as const) {
      const td = tr.querySelector<HTMLElement>(`td.${field}`);
      const input = td?.querySelector("input");
      const err = td?.querySelector<HTMLElement>(".field-error");
      if (!input || !err) continue;
      const message = v.rows[i][field];
      input.setAttribute("aria-invalid", String(message !== null));
      showError(err, message);
    }
  });
  showError(formError, v.form);
  save.disabled = overwrite.disabled = !(loaded && v.ok);
}

async function load(): Promise<void> {
  let snap: ScheduleSnapshot;
  // Freeze the form like submit() does, so edits or a save cannot race with the
  // pending snapshot and end up shown as saved for rows that were never written
  form.inert = true;
  try {
    snap = await invoke<ScheduleSnapshot>(LOAD_SCHEDULE_COMMAND);
  } catch (e) {
    // Keep Cancel usable, but nothing can be edited or saved without a snapshot
    status.textContent = `Cannot load the schedule: ${String(e)}`;
    loaded = false;
    add.disabled = true;
    save.disabled = overwrite.disabled = true;
    return;
  } finally {
    form.inert = false;
  }
  loaded = true;
  add.disabled = false;
  rows = snap.entries;
  revision = snap.revision;
  conflict.hidden = true;
  renderRows();
  if (snap.fileError !== null) {
    status.textContent = `schedule.toml is invalid (${snap.fileError}); showing the last valid schedule. Save replaces the file.`;
  }
}

function isSaveError(e: unknown): e is SaveScheduleError {
  return typeof e === "object" && e !== null && "kind" in e;
}

async function submit(overwrite: boolean): Promise<void> {
  // Freeze the form so the rows shown as saved are exactly the rows sent
  form.inert = true;
  try {
    revision = await invoke<string>(SAVE_SCHEDULE_COMMAND, {
      rows,
      revision,
      overwrite,
    } satisfies SaveScheduleArgs);
  } catch (e) {
    if (isSaveError(e) && e.kind === "conflict") {
      conflict.hidden = false;
      status.textContent = "";
    } else {
      // IPC failures reject with a plain string rather than a SaveScheduleError
      const message = isSaveError(e) && e.kind !== "conflict" ? e.message : String(e);
      status.textContent = `Not saved: ${message}`;
    }
    return;
  } finally {
    form.inert = false;
  }
  // Show the entries sorted, as the file now has them. Valid "HH:MM" sorts as text.
  rows.sort((a, b) => a.time.localeCompare(b.time));
  conflict.hidden = true;
  renderRows();
  status.textContent = "Saved";
}

form.addEventListener("submit", (e) => {
  e.preventDefault();
  void submit(false);
});
overwrite.addEventListener("click", () => void submit(true));
$("reload").addEventListener("click", () => void load());
add.addEventListener("click", () => {
  rows.push({ time: "", title: "" });
  renderRows();
  tbody.querySelector<HTMLInputElement>("tr:last-child input")?.focus();
});
// Closing discards unsaved edits
$("cancel").addEventListener("click", () => void getCurrentWindow().close());

await load();
