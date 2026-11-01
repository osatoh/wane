//! Silence iPhone/Watch While Using Mac (Spec §8).
//!
//! A shared Focus is switched by running the user's `Wane Focus On` /
//! `Wane Focus Off` shortcuts. The Mac counts as "in use" while the toggle is
//! on and none of the blocking conditions (locked, asleep, screen saver, idle)
//! holds. Turning off runs at once; turning on waits for the state to settle,
//! because a stuck Focus would silence the phone while the user is away.

use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const SHORTCUT_ON: &str = "Wane Focus On";
pub const SHORTCUT_OFF: &str = "Wane Focus Off";

/// How long the Mac must stay in use before the Focus is turned on.
pub const DEBOUNCE: Duration = Duration::from_secs(10);

/// Shortcut to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    On,
    Off,
}

impl Action {
    pub fn shortcut(self) -> &'static str {
        match self {
            Action::On => SHORTCUT_ON,
            Action::Off => SHORTCUT_OFF,
        }
    }
}

/// Conditions under which the Mac is not in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    Locked,
    Asleep,
    ScreenSaver,
    Idle,
    /// Another user's session is in front (fast user switching).
    SwitchedOut,
}

impl Blocker {
    fn bit(self) -> u8 {
        match self {
            Blocker::Locked => 1,
            Blocker::Asleep => 2,
            Blocker::ScreenSaver => 4,
            Blocker::Idle => 8,
            Blocker::SwitchedOut => 16,
        }
    }
}

/// Decides which shortcut to run, if any. Pure: time is passed in.
#[derive(Debug, Default)]
pub struct Controller {
    enabled: bool,
    blockers: u8,
    /// Last state Wane set; `None` while unknown.
    applied: Option<bool>,
    pending_on: Option<Instant>,
    /// A shortcut failed and the Focus may be left on: only quitting tries
    /// "off" again, so a broken shortcut never runs in a loop.
    failed: bool,
}

impl Controller {
    pub fn new(enabled: bool, now: Instant) -> Self {
        let mut c = Self::default();
        c.set_enabled(enabled, now);
        c
    }

    pub fn set_enabled(&mut self, enabled: bool, now: Instant) -> Option<Action> {
        if self.failed {
            if !enabled {
                return None;
            }
            // Turned back on after a failure: start over
            self.failed = false;
        }
        self.enabled = enabled;
        self.update(now)
    }

    pub fn set_blocker(&mut self, blocker: Blocker, active: bool, now: Instant) -> Option<Action> {
        if active {
            self.blockers |= blocker.bit();
        } else {
            self.blockers &= !blocker.bit();
        }
        self.update(now)
    }

    /// Fires a pending "on" once its debounce has elapsed.
    pub fn poll(&mut self, now: Instant) -> Option<Action> {
        match self.pending_on {
            Some(at) if now >= at && self.wants_on() => {
                self.pending_on = None;
                self.applied = Some(true);
                Some(Action::On)
            }
            _ => None,
        }
    }

    /// When [`poll`](Self::poll) should be called next.
    pub fn deadline(&self) -> Option<Instant> {
        self.pending_on
    }

    /// A shortcut failed: stop switching without retrying. The Focus may
    /// still be on, so quitting tries "off" once more.
    pub fn fail(&mut self) {
        self.enabled = false;
        self.applied = None;
        self.pending_on = None;
        self.failed = true;
    }

    /// Wane is quitting: turn off unless known to be off already.
    pub fn shutdown(&mut self) -> Option<Action> {
        self.pending_on = None;
        if std::mem::take(&mut self.failed) {
            self.applied = Some(false);
            return Some(Action::Off);
        }
        let action = self.turn_off();
        self.enabled = false;
        action
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn wants_on(&self) -> bool {
        self.enabled && self.blockers == 0
    }

    fn update(&mut self, now: Instant) -> Option<Action> {
        if self.wants_on() {
            if self.applied != Some(true) && self.pending_on.is_none() {
                self.pending_on = Some(now + DEBOUNCE);
            }
            return None;
        }
        self.pending_on = None;
        self.turn_off()
    }

    /// Turns off unless known to be off. Unknown counts as possibly on while
    /// the toggle is on: prefer turning off.
    fn turn_off(&mut self) -> Option<Action> {
        let maybe_on = self.applied == Some(true) || (self.enabled && self.applied.is_none());
        if maybe_on {
            self.applied = Some(false);
            Some(Action::Off)
        } else {
            None
        }
    }
}

/// How often idle time is sampled.
const IDLE_POLL: Duration = Duration::from_secs(15);

enum Msg {
    Enabled(bool),
    Blocker(Blocker, bool, Option<mpsc::Sender<()>>),
    Shutdown(mpsc::Sender<()>),
}

/// Handle to the worker thread that owns the [`Controller`].
#[derive(Clone)]
pub struct Focus(mpsc::Sender<Msg>);

impl Focus {
    pub fn set_enabled(&self, on: bool) {
        let _ = self.0.send(Msg::Enabled(on));
    }

    pub fn set_blocker(&self, blocker: Blocker, active: bool) {
        let _ = self.0.send(Msg::Blocker(blocker, active, None));
    }

    /// Like [`set_blocker`](Self::set_blocker), but waits up to `timeout` for
    /// any resulting shortcut to finish (e.g. "off" before the Mac sleeps).
    pub fn set_blocker_and_wait(&self, blocker: Blocker, active: bool, timeout: Duration) {
        let (tx, rx) = mpsc::channel();
        if self.0.send(Msg::Blocker(blocker, active, Some(tx))).is_ok() {
            let _ = rx.recv_timeout(timeout);
        }
    }

    /// Turns the Focus off before quitting, waiting up to `timeout`.
    /// Use [`OFF_TIMEOUT`].
    pub fn shutdown(&self, timeout: Duration) {
        let (tx, rx) = mpsc::channel();
        if self.0.send(Msg::Shutdown(tx)).is_ok() {
            let _ = rx.recv_timeout(timeout);
        }
    }
}

/// Starts the worker.
///
/// - `run` runs a shortcut.
/// - `idle_secs` returns seconds since the last user input.
/// - `on_failure` is called once when a shortcut fails; the worker then stays
///   inactive until enabled again.
pub fn spawn(
    enabled: bool,
    idle_after: Duration,
    run: impl Fn(Action) -> Result<(), String> + Send + 'static,
    idle_secs: impl Fn() -> f64 + Send + 'static,
    on_failure: impl Fn(String) + Send + 'static,
) -> Focus {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut c = Controller::new(enabled, Instant::now());
        let exec = |c: &mut Controller, action: Option<Action>| {
            if let Some(action) = action
                && let Err(e) = run(action)
            {
                c.fail();
                on_failure(format!("\"{}\": {e}", action.shortcut()));
            }
        };
        loop {
            let now = Instant::now();
            // Disabled: nothing to sample, so sleep until the next message
            let received = if c.is_enabled() {
                let wait = c.deadline().map_or(IDLE_POLL, |at| {
                    at.saturating_duration_since(now).min(IDLE_POLL)
                });
                rx.recv_timeout(wait)
            } else {
                rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            };
            let mut ack = None;
            let action = match received {
                Ok(Msg::Enabled(on)) => c.set_enabled(on, Instant::now()),
                Ok(Msg::Blocker(b, active, done)) => {
                    ack = done;
                    c.set_blocker(b, active, Instant::now())
                }
                Ok(Msg::Shutdown(ack)) => {
                    if let Some(action) = c.shutdown()
                        && let Err(e) = run(action)
                    {
                        eprintln!("focus: {e}");
                    }
                    let _ = ack.send(());
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            };
            exec(&mut c, action);
            if let Some(ack) = ack {
                let _ = ack.send(());
            }
            if !c.is_enabled() {
                continue;
            }
            let idle = Duration::from_secs_f64(idle_secs().max(0.0)) >= idle_after;
            let action = c.set_blocker(Blocker::Idle, idle, Instant::now());
            exec(&mut c, action);
            let action = c.poll(Instant::now());
            exec(&mut c, action);
        }
    });
    Focus(tx)
}

/// Longest a shortcut may run. A hung shortcut must not block later "off"
/// actions, so it is killed and treated as a failure.
const SHORTCUT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long quitting or sleeping waits for "off": room for a shortcut
/// already running plus the "off" itself.
pub const OFF_TIMEOUT: Duration = Duration::from_secs(2 * SHORTCUT_TIMEOUT.as_secs() + 1);

/// Runs `/usr/bin/shortcuts run <name>`.
pub fn run_shortcut(action: Action) -> Result<(), String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new("/usr/bin/shortcuts")
        .args(["run", action.shortcut()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if started.elapsed() >= SHORTCUT_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timed out after {}s", SHORTCUT_TIMEOUT.as_secs()));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    let stderr = stderr.trim();
    Err(if stderr.is_empty() {
        format!("shortcuts exited with {status}")
    } else {
        stderr.to_string()
    })
}

/// Seconds since the last keyboard / mouse / trackpad input.
#[cfg(target_os = "macos")]
pub fn idle_seconds() -> f64 {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    const COMBINED_SESSION_STATE: i32 = 0;
    const ANY_INPUT_EVENT: u32 = u32::MAX;
    // SAFETY: plain C function without pointers; safe to call from any thread.
    unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT) }
}

/// Feeds lock / sleep / screen saver notifications to the worker.
///
/// Must be called on the main thread; the observers live for the app's lifetime.
#[cfg(target_os = "macos")]
pub fn observe_system_changes(focus: &Focus) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::{
        NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceSessionDidBecomeActiveNotification,
        NSWorkspaceSessionDidResignActiveNotification, NSWorkspaceWillSleepNotification,
    };
    use objc2_foundation::{
        NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSString,
    };

    let add = |center: &NSNotificationCenter, name: &NSString, blocker: Blocker, active: bool| {
        let focus = focus.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            if blocker == Blocker::Asleep && active {
                // Returning lets the Mac sleep, so hold it until "off" has run
                focus.set_blocker_and_wait(blocker, active, OFF_TIMEOUT);
            } else {
                focus.set_blocker(blocker, active);
            }
        });
        // SAFETY: the block only talks to the worker and is valid on any thread.
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
        };
        // Observers stay registered until the process exits
        std::mem::forget(observer);
    };

    let distributed = NSDistributedNotificationCenter::defaultCenter();
    for (name, blocker, active) in [
        ("com.apple.screenIsLocked", Blocker::Locked, true),
        ("com.apple.screenIsUnlocked", Blocker::Locked, false),
        ("com.apple.screensaver.didstart", Blocker::ScreenSaver, true),
        ("com.apple.screensaver.didstop", Blocker::ScreenSaver, false),
    ] {
        add(&distributed, &NSString::from_str(name), blocker, active);
    }
    // Sleep / wake and user switching are posted only on NSWorkspace's own center
    let workspace = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: the statics are valid NSString constants provided by AppKit.
    let workspace_names = unsafe {
        [
            (NSWorkspaceWillSleepNotification, Blocker::Asleep, true),
            (NSWorkspaceDidWakeNotification, Blocker::Asleep, false),
            (
                NSWorkspaceSessionDidResignActiveNotification,
                Blocker::SwitchedOut,
                true,
            ),
            (
                NSWorkspaceSessionDidBecomeActiveNotification,
                Blocker::SwitchedOut,
                false,
            ),
        ]
    };
    for (name, blocker, active) in workspace_names {
        add(&workspace, name, blocker, active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_does_nothing() {
        let now = Instant::now();
        let mut c = Controller::new(false, now);
        assert_eq!(c.set_blocker(Blocker::Locked, true, now), None);
        assert_eq!(c.set_blocker(Blocker::Locked, false, now), None);
        assert_eq!(c.poll(now + DEBOUNCE * 2), None);
        assert_eq!(c.shutdown(), None);
    }

    #[test]
    fn turns_on_after_the_debounce() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        assert_eq!(c.poll(now + DEBOUNCE - Duration::from_millis(1)), None);
        assert_eq!(c.deadline(), Some(now + DEBOUNCE));
        assert_eq!(c.poll(now + DEBOUNCE), Some(Action::On));
        assert_eq!(c.poll(now + DEBOUNCE * 2), None);
    }

    #[test]
    fn turns_off_at_once_when_blocked() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        c.poll(now + DEBOUNCE);
        assert_eq!(c.set_blocker(Blocker::Locked, true, now), Some(Action::Off));
        // No change, no second run
        assert_eq!(c.set_blocker(Blocker::Idle, true, now), None);
    }

    #[test]
    fn blocked_before_first_on_still_turns_off() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        assert_eq!(c.set_blocker(Blocker::Asleep, true, now), Some(Action::Off));
        assert_eq!(c.deadline(), None);
    }

    #[test]
    fn stays_off_until_every_blocker_clears() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        c.poll(now + DEBOUNCE);
        c.set_blocker(Blocker::Locked, true, now);
        c.set_blocker(Blocker::ScreenSaver, true, now);
        assert_eq!(c.set_blocker(Blocker::Locked, false, now), None);
        assert_eq!(c.deadline(), None);
        assert_eq!(c.set_blocker(Blocker::ScreenSaver, false, now), None);
        assert_eq!(c.poll(now + DEBOUNCE), Some(Action::On));
    }

    #[test]
    fn rapid_flapping_does_not_turn_on() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        c.poll(now + DEBOUNCE);
        let later = now + DEBOUNCE;
        assert_eq!(
            c.set_blocker(Blocker::Locked, true, later),
            Some(Action::Off)
        );
        c.set_blocker(Blocker::Locked, false, later);
        assert_eq!(c.set_blocker(Blocker::Locked, true, later), None);
        assert_eq!(c.poll(later + DEBOUNCE), None);
    }

    #[test]
    fn toggling_off_turns_off() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        c.poll(now + DEBOUNCE);
        assert_eq!(c.set_enabled(false, now), Some(Action::Off));
        assert_eq!(c.poll(now + DEBOUNCE * 2), None);
    }

    #[test]
    fn shutdown_turns_off_when_on_or_unknown() {
        let now = Instant::now();
        let mut on = Controller::new(true, now);
        on.poll(now + DEBOUNCE);
        assert_eq!(on.shutdown(), Some(Action::Off));

        let mut pending = Controller::new(true, now);
        assert_eq!(pending.shutdown(), Some(Action::Off));

        let mut off = Controller::new(true, now);
        off.set_blocker(Blocker::Locked, true, now);
        assert_eq!(off.shutdown(), None);
    }

    #[test]
    fn failure_stops_switching_but_still_turns_off_on_quit() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        c.fail();
        assert_eq!(c.poll(now + DEBOUNCE), None);
        assert_eq!(c.set_blocker(Blocker::Locked, false, now), None);
        // The failure handler turns the toggle off: must not run "off" again
        assert_eq!(c.set_enabled(false, now), None);
        assert_eq!(c.shutdown(), Some(Action::Off));
        assert_eq!(c.shutdown(), None);
    }

    #[test]
    fn re_enabling_after_failure_turns_on_again() {
        let now = Instant::now();
        let mut c = Controller::new(true, now);
        c.fail();
        assert_eq!(c.set_enabled(true, now), None);
        assert_eq!(c.poll(now + DEBOUNCE), Some(Action::On));
    }

    #[test]
    fn worker_runs_off_and_reports_failure_once() {
        let (ran_tx, ran_rx) = mpsc::channel();
        let (err_tx, err_rx) = mpsc::channel();
        let focus = spawn(
            true,
            Duration::from_secs(300),
            move |a| {
                ran_tx.send(a).unwrap();
                Err("not found".into())
            },
            || 0.0,
            move |e| err_tx.send(e).unwrap(),
        );
        focus.set_blocker(Blocker::Locked, true);
        let wait = Duration::from_secs(1);
        assert_eq!(ran_rx.recv_timeout(wait), Ok(Action::Off));
        assert!(err_rx.recv_timeout(wait).unwrap().contains(SHORTCUT_OFF));
        // Failed: no more switching, and no second error report
        focus.set_blocker(Blocker::Locked, false);
        assert!(ran_rx.recv_timeout(Duration::from_millis(200)).is_err());
        // Quitting still tries "off" once
        focus.shutdown(wait);
        assert_eq!(ran_rx.try_recv(), Ok(Action::Off));
        assert!(err_rx.try_recv().is_err());
    }

    #[test]
    fn set_blocker_and_wait_returns_after_the_shortcut() {
        let (ran_tx, ran_rx) = mpsc::channel();
        let focus = spawn(
            true,
            Duration::from_secs(300),
            move |a| {
                std::thread::sleep(Duration::from_millis(100));
                ran_tx.send(a).unwrap();
                Ok(())
            },
            || 0.0,
            |_| {},
        );
        focus.set_blocker_and_wait(Blocker::Asleep, true, Duration::from_secs(1));
        assert_eq!(ran_rx.try_recv(), Ok(Action::Off));
    }

    #[test]
    fn worker_treats_long_input_gap_as_idle() {
        let (ran_tx, ran_rx) = mpsc::channel();
        let focus = spawn(
            true,
            Duration::from_secs(300),
            move |a| {
                ran_tx.send(a).unwrap();
                Ok(())
            },
            || 301.0,
            |_| {},
        );
        // Any message makes the worker sample idle time
        focus.set_blocker(Blocker::ScreenSaver, false);
        assert_eq!(ran_rx.recv_timeout(Duration::from_secs(1)), Ok(Action::Off));
        focus.shutdown(Duration::from_secs(1));
        assert!(ran_rx.try_recv().is_err());
    }
}
