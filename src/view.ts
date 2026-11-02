// Pure helpers for the view. State payload (sent by the backend):
//   { current: { title, time }, next: { title, time }, remainingFraction, remainingSecs }
// `time` is the entry's start as "HH:MM". Entry times are unique, so it also identifies
// the task regardless of its title.

export interface Entry {
  title: string;
  // Start time as "HH:MM".
  time: string;
}

export interface State {
  current: Entry;
  next: Entry;
  // Share of the current task still left, in [0, 1].
  remainingFraction: number;
  remainingSecs: number;
}

// True when the current task changed, which plays the switch transition. Edits to later
// entries do not move the current task, so they update in place without it.
export function isTaskSwitch(
  prev: Pick<State, "current"> | null,
  state: Pick<State, "current">,
): boolean {
  return prev !== null && prev.current.time !== state.current.time;
}
