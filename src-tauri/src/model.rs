//! Display state as a pure function of the clock and the schedule (Spec §4).

use crate::schedule::{Entry, Schedule, TimeOfDay};

const SECS_PER_DAY: u32 = 24 * 60 * 60;

/// What to show at a given moment. Nothing here is stored between calls.
#[derive(Debug, Clone, PartialEq)]
pub struct State<'a> {
    pub current: &'a Entry,
    pub next: &'a Entry,
    /// `(next.start - now) / (next.start - current.start)`, clamped to [0, 1].
    pub remaining_fraction: f64,
    /// Seconds until the next task starts.
    pub remaining_secs: u32,
}

impl State<'_> {
    /// Clock time the next task starts (shown on hover).
    pub fn next_time(&self) -> TimeOfDay {
        self.next.time
    }
}

/// Computes the state for `now`, given as seconds since local midnight.
pub fn state(now_secs: u32, schedule: &Schedule) -> State<'_> {
    let now = now_secs.min(SECS_PER_DAY - 1);
    let entries = schedule.entries();
    let start = |e: &Entry| u32::from(e.time.minutes()) * 60;
    // Entries are sorted with unique times; a valid schedule has at least 2
    let idx = entries.partition_point(|e| start(e) <= now);
    // idx == 0: before the first entry, so the current task started the previous day
    let current = &entries[idx.checked_sub(1).unwrap_or(entries.len() - 1)];
    // idx == len: after the last entry, so the next task starts the next day
    let next = &entries[idx % entries.len()];

    // Measure everything from the current task's start, wrapping past midnight
    let since = |t: u32| (t + SECS_PER_DAY - start(current)) % SECS_PER_DAY;
    let window = since(start(next));
    let elapsed = since(now);
    let remaining_secs = window - elapsed;
    State {
        current,
        next,
        remaining_fraction: (f64::from(remaining_secs) / f64::from(window)).clamp(0.0, 1.0),
        remaining_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::{DEFAULT_SCHEDULE, parse};

    fn at(h: u32, m: u32, s: u32) -> u32 {
        (h * 60 + m) * 60 + s
    }

    fn default() -> Schedule {
        parse(DEFAULT_SCHEDULE).unwrap()
    }

    #[test]
    fn ac1_midway_through_gym() {
        let s = default();
        let st = state(at(12, 20, 0), &s);
        assert!(st.current.title.starts_with("Gym"));
        assert_eq!(st.next.title, "Shower → Meal 3");
        assert!((st.remaining_fraction - 70.0 / 90.0).abs() < 1e-9);
        assert_eq!(st.remaining_secs, 70 * 60);
        assert_eq!(st.next_time().to_string(), "13:30");
    }

    #[test]
    fn ac2_exactly_at_entry_start_is_full() {
        let s = default();
        let st = state(at(13, 30, 0), &s);
        assert_eq!(st.current.title, "Shower → Meal 3");
        assert_eq!(st.next.title, "Back to work");
        assert_eq!(st.remaining_fraction, 1.0);
        assert_eq!(st.remaining_secs, 30 * 60);
    }

    #[test]
    fn ac3_after_last_entry_wraps_to_next_day() {
        let s = default();
        let st = state(at(23, 45, 0), &s);
        assert_eq!(st.current.title, "Dayvigo → Bed");
        assert!(st.next.title.starts_with("Wake up"));
        assert_eq!(st.next_time().to_string(), "08:00");
        // 23:30 → 08:00 is 510 min; 495 min remain
        assert_eq!(st.remaining_secs, 495 * 60);
        assert!((st.remaining_fraction - 495.0 / 510.0).abs() < 1e-9);
    }

    #[test]
    fn ac4_before_first_entry_uses_previous_day() {
        let s = default();
        let st = state(at(7, 0, 0), &s);
        assert_eq!(st.current.title, "Dayvigo → Bed");
        assert!(st.next.title.starts_with("Wake up"));
        assert_eq!(st.remaining_secs, 60 * 60);
        assert!((st.remaining_fraction - 60.0 / 510.0).abs() < 1e-9);
    }

    #[test]
    fn last_second_before_next_entry_is_nearly_empty() {
        let s = default();
        let st = state(at(13, 29, 59), &s);
        assert!(st.current.title.starts_with("Gym"));
        assert_eq!(st.remaining_secs, 1);
        assert!(st.remaining_fraction > 0.0 && st.remaining_fraction < 0.001);
    }

    #[test]
    fn midnight_and_end_of_day() {
        let s = default();
        for now in [0, SECS_PER_DAY - 1] {
            let st = state(now, &s);
            assert_eq!(st.current.title, "Dayvigo → Bed");
            assert!(st.next.title.starts_with("Wake up"));
            assert!((0.0..=1.0).contains(&st.remaining_fraction));
        }
    }

    #[test]
    fn out_of_range_now_is_clamped() {
        // A leap second or bad clock value must not panic or leave [0, 1]
        let s = default();
        let st = state(SECS_PER_DAY + 10, &s);
        assert_eq!(st.current.title, "Dayvigo → Bed");
        assert!((0.0..=1.0).contains(&st.remaining_fraction));
    }
}
