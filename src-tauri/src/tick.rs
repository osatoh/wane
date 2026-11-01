//! Update triggers (Spec §6): a timer aligned to the minute boundary, plus
//! immediate re-evaluation on wake from sleep and clock / time zone changes.
//!
//! Task start times have minute precision, so the timer that fires on every
//! minute boundary also fires at the exact start of the next task. There is no
//! separate one-shot timer.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Event the frontend listens to; it re-evaluates the state and redraws on each one.
pub const TICK_EVENT: &str = "reevaluate";

/// Fire slightly after the boundary so the frontend never reads 59.999 s.
const MARGIN: Duration = Duration::from_millis(5);

/// Time from `now` until just after the next minute boundary.
///
/// Uses Unix time: every time zone offset in use is a whole number of
/// minutes, so local and UTC minute boundaries coincide.
pub fn until_next_minute(now: SystemTime) -> Duration {
    let since = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let into_minute = Duration::new(since.as_secs() % 60, since.subsec_nanos());
    Duration::from_secs(60) - into_minute + MARGIN
}

/// Handle that makes the ticker fire now and recompute its timer.
#[derive(Clone, Default)]
pub struct Poke(Arc<(Mutex<bool>, Condvar)>);

impl Poke {
    pub fn poke(&self) {
        let (flag, cvar) = &*self.0;
        *flag.lock().unwrap_or_else(|e| e.into_inner()) = true;
        cvar.notify_one();
    }

    /// Waits up to `timeout`; returns early if poked.
    fn wait(&self, timeout: Duration) {
        let (flag, cvar) = &*self.0;
        let guard = flag.lock().unwrap_or_else(|e| e.into_inner());
        let (mut guard, _) = cvar
            .wait_timeout_while(guard, timeout, |poked| !*poked)
            .unwrap_or_else(|e| e.into_inner());
        *guard = false;
    }
}

/// Calls `on_tick` on every minute boundary and whenever the returned handle is poked.
///
/// The timer is recomputed from the wall clock after every tick. The wait runs
/// on a monotonic clock that stops while the Mac sleeps, so wake and clock
/// changes must poke the ticker (see [`observe_system_changes`]).
pub fn spawn(on_tick: impl Fn() + Send + 'static) -> Poke {
    let poke = Poke::default();
    let p = poke.clone();
    std::thread::spawn(move || {
        loop {
            p.wait(until_next_minute(SystemTime::now()));
            on_tick();
        }
    });
    poke
}

/// Pokes on wake from sleep and on system clock / time zone changes.
///
/// Must be called on the main thread; the observers live for the app's lifetime.
#[cfg(target_os = "macos")]
pub fn observe_system_changes(poke: Poke) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2_app_kit::{NSWorkspace, NSWorkspaceDidWakeNotification};
    use objc2_foundation::{
        NSNotification, NSNotificationCenter, NSNotificationName,
        NSSystemClockDidChangeNotification, NSSystemTimeZoneDidChangeNotification,
    };

    let block = RcBlock::new(move |_: NonNull<NSNotification>| poke.poke());
    let add = |center: &NSNotificationCenter, name: &NSNotificationName| {
        // SAFETY: the block only signals a condvar and is valid on any thread.
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
        };
        // Observers stay registered until the process exits
        std::mem::forget(observer);
    };
    // Wake is posted only on NSWorkspace's own center
    // SAFETY: the statics are valid NSString constants provided by the frameworks.
    unsafe {
        add(
            &NSWorkspace::sharedWorkspace().notificationCenter(),
            NSWorkspaceDidWakeNotification,
        );
        let center = NSNotificationCenter::defaultCenter();
        add(&center, NSSystemClockDidChangeNotification);
        add(&center, NSSystemTimeZoneDidChangeNotification);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    fn at(secs: u64, millis: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs) + Duration::from_millis(millis)
    }

    #[test]
    fn fires_just_after_the_next_boundary() {
        // 13:29:30 UTC on some day
        let d = until_next_minute(at(1_700_000_000 - 1_700_000_000 % 60 + 30, 0));
        assert_eq!(d, Duration::from_secs(30) + MARGIN);
    }

    #[test]
    fn mid_second_counts_subseconds() {
        let base = 1_700_000_000 - 1_700_000_000 % 60;
        assert_eq!(
            until_next_minute(at(base + 59, 999)),
            Duration::from_millis(1) + MARGIN
        );
    }

    #[test]
    fn exactly_on_boundary_waits_a_full_minute() {
        let base = 1_700_000_000 - 1_700_000_000 % 60;
        assert_eq!(
            until_next_minute(at(base, 0)),
            Duration::from_secs(60) + MARGIN
        );
    }

    #[test]
    fn poke_fires_immediately_and_repeatedly() {
        let (tx, rx) = mpsc::channel();
        let poke = spawn(move || tx.send(()).unwrap());
        for _ in 0..2 {
            let start = Instant::now();
            poke.poke();
            rx.recv_timeout(Duration::from_secs(1))
                .expect("tick after poke");
            assert!(start.elapsed() < Duration::from_secs(1));
        }
    }
}
