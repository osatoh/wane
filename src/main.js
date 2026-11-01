// Wires backend events to the display (Spec §5).
//
// Events from the backend (emitted by the update triggers, #4 / #5):
//   "wane://state"          payload: state object (see view.js)
//   "wane://schedule-error" payload: string message, or null to clear it
// The frontend emits "wane://ready" once listening, so the backend can send the current state
// without racing the page load.
import { createCircleRenderer } from "./renderers/circle.js";
import { formatRemaining } from "./format.js";
import { isTaskSwitch } from "./view.js";

const $ = (id) => document.getElementById(id);
const gauge = createCircleRenderer($("gauge"));
const titles = $("titles");
const currentTitle = $("current-title");
const nextTitle = $("next-title");
const remaining = $("remaining");
const nextTime = $("next-time");
const error = $("error");
let prev = null;

function render(state) {
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

function showError(message) {
  error.textContent = message ?? "";
  error.hidden = !message;
}

const { listen, emit } = window.__TAURI__.event;
await Promise.all([
  listen("wane://state", (e) => render(e.payload)),
  listen("wane://schedule-error", (e) => showError(e.payload)),
]);
await emit("wane://ready");
