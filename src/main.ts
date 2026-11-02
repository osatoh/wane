// Wires backend events to the display (Spec §5).
//
// Events from the backend (emitted by the update triggers, #4 / #5):
//   "wane://state"          payload: state object (see view.js)
//   "wane://schedule-error" payload: string message, or null to clear it
// The frontend emits "wane://ready" once listening, so the backend can send the current state
// without racing the page load.
import { emit, listen } from "@tauri-apps/api/event";
import { createCircleRenderer } from "./renderers/circle";
import { formatRemaining } from "./format";
import { isTaskSwitch, type State } from "./view";
import {
  READY_EVENT,
  SCHEDULE_ERROR_EVENT,
  STATE_EVENT,
  type ScheduleErrorPayload,
  type StatePayload,
} from "./events";

// Elements are static in index.html, so a missing one is a bug.
function $(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing #${id}`);
  return el;
}
const gauge = createCircleRenderer($("gauge"));
const titles = $("titles");
const currentTitle = $("current-title");
const nextTitle = $("next-title");
const remaining = $("remaining");
const nextTime = $("next-time");
const error = $("error");
let prev: State | null = null;

function render(state: State): void {
  const refill = isTaskSwitch(prev, state);
  prev = state;
  gauge.update(state.remainingFraction, { refill });
  currentTitle.textContent = state.current.title;
  nextTitle.textContent = state.next.title;
  remaining.textContent = formatRemaining(state.remainingSecs);
  nextTime.textContent = state.next.time;
  if (refill) {
    // Restart the "titles move forward" animation.
    titles.classList.remove("advancing");
    void titles.offsetWidth;
    titles.classList.add("advancing");
  }
}

function showError(message: ScheduleErrorPayload): void {
  error.textContent = message ?? "";
  error.hidden = !message;
}

await Promise.all([
  listen<StatePayload>(STATE_EVENT, (e) => render(e.payload)),
  listen<ScheduleErrorPayload>(SCHEDULE_ERROR_EVENT, (e) => showError(e.payload)),
]);
await emit(READY_EVENT);
