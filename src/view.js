// Pure helpers for the view. State payload (sent by the backend):
//   { current: { title, time }, next: { title, time }, remainingFraction, remainingSecs }
// `time` is the entry's start as "HH:MM". Entry times are unique, so it also identifies
// the task regardless of its title.

// True when the current task changed, which plays the switch transition. Edits to later
// entries do not move the current task, so they update in place without it.
export function isTaskSwitch(prev, state) {
  return prev !== null && prev.current.time !== state.current.time;
}
