# Wane — Requirements

## 1. Overview

**Wane** is a small, always-on-top macOS app that shows how much time is left until the next item in a fixed daily routine. The remaining time is shown as a shrinking shape instead of numbers.

Wane is **standalone**. The schedule is stored in the app's own config file. It does not read from or write to Apple Reminders.

Intended setup:
- **At the Mac**: Wane is the way the user follows the routine.
- **Away from the Mac**: the user keeps using Apple Reminders on iPhone / Apple Watch for notifications, as today.

Wane can optionally silence Reminders notifications on iPhone / Watch while the Mac is in use (section 8).

## 2. Platform & Stack

- macOS 14 (Sonoma) or later
- Tauri v2 (Rust backend, HTML/CSS/JS frontend)
- Native window tweaks via `objc2-app-kit` where Tauri's options are not enough
- No EventKit, no Reminders permission

## 3. Schedule

### Storage

- Stored as TOML at `~/Library/Application Support/Wane/schedule.toml`.
- On first launch, create the file with the default schedule below.
- Watch the file and **reload automatically** when it changes. No restart is needed.
- If the file is invalid, keep using the last valid schedule and show a short error message in the window.
- v1 has no in-app editor. The menu bar item opens the file in the default text editor.

### Rules

- The schedule repeats every day. All days use the same schedule.
- Each entry has a time (`HH:MM`, 24-hour, local time) and a title.
- Entries are sorted by time on load. Duplicate times are an error.
- At least 2 entries are required.

### Default schedule

```toml
[[entry]]
time = "07:00"
title = "Wake up"

[[entry]]
time = "07:30"
title = "Breakfast"

[[entry]]
time = "09:00"
title = "Start work"

[[entry]]
time = "12:00"
title = "Lunch"

[[entry]]
time = "13:00"
title = "Back to work"

[[entry]]
time = "18:00"
title = "Finish work"

[[entry]]
time = "19:00"
title = "Dinner"

[[entry]]
time = "21:00"
title = "Wind down"

[[entry]]
time = "23:00"
title = "Bed"
```

## 4. Core Model

Each entry's time is the **start time** of that activity.

- **Current task**: the entry with the latest time that is ≤ now. If now is earlier than the first entry, it is the last entry of the previous day.
- **Next task**: the entry with the earliest time that is > now. If now is later than the last entry, it is the first entry of the next day.
- **Progress window**: from the current task's start (100%) to the next task's start (0%).
- **Remaining fraction**: `(next.start - now) / (next.start - current.start)`, linear, clamped to [0, 1].
- The state is a pure function of the clock and the schedule. There is no stored progress and no completion state. After sleep or restart, the correct state is shown immediately.

## 5. Display

- **Primary visual**: a shape whose filled area shrinks as time passes. v1 uses a circle. Keep the renderer swappable so an hourglass can be added later.
- **Always visible**: the current task's title and the next task's title.
- **Never shown by default**: numbers.
- **On hover**: remaining time (e.g. `42 min`, `1 h 05 min`) and the next task's clock time (e.g. `12:00`).
- When the next task starts, play a short transition: the gauge refills and the titles move forward.

## 6. Updates

Re-evaluate the state and redraw on:

- A timer every **60 seconds**, aligned to the minute boundary
- The exact start time of the next task, so the switch happens on time and not up to a minute late
- Wake from sleep (`NSWorkspaceDidWakeNotification`)
- System clock or time zone change (`NSSystemClockDidChangeNotification`, `NSSystemTimeZoneDidChangeNotification`)
- Schedule file change
- App launch

Animations are short transitions between the minute steps. There is no continuous 60 fps redraw.

## 7. Window Behavior

- Small, frameless window with a native blur backdrop (macOS HUD material, rounded corners) that can be dragged to move. The backdrop keeps the content readable over any background.
- Always on top by default. This can be toggled on/off.
- Show on all Spaces and above full-screen apps (`canJoinAllSpaces`, `fullScreenAuxiliary`).
- Can be hidden and shown again.
- Remember the window position and size across launches.
- No Dock icon (`LSUIElement = true`).

### Menu bar item

- Show / Hide window
- Always on Top (toggle)
- Edit Schedule… (opens `schedule.toml` in the default editor)
- Silence iPhone/Watch While Using Mac (toggle; default off; see section 8)
- Launch at Login (toggle; default on; uses `SMAppService`)
- Quit

## 8. Optional: Silence iPhone/Watch While Using Mac

The goal is to avoid getting the same reminder on the Mac screen and on the Watch at the same time.

### How it works

- The user creates a Focus mode (e.g. "Wane") that silences only the Reminders app and has "Share Across Devices" turned on.
- The user creates two Shortcuts: `Wane Focus On` and `Wane Focus Off`. Each uses the "Set Focus" action.
- Wane runs them with `/usr/bin/shortcuts run "<name>"` when the Mac becomes active or inactive.
- The shared Focus then applies to iPhone and Watch.

### Triggers

**Turn on** when:
- The screen is unlocked (`com.apple.screenIsUnlocked`)
- The Mac wakes from sleep
- Wane launches
- The user becomes active again after being idle

**Turn off** when:
- The screen is locked (`com.apple.screenIsLocked`)
- The Mac is going to sleep (`NSWorkspaceWillSleepNotification`)
- The screen saver starts
- The user has been idle for **N minutes** (default 5, configurable; idle time from `CGEventSourceSecondsSinceLastEventType`)
- Wane quits

### Rules

- Run a shortcut only when the state actually changes, and debounce rapid changes (e.g. 10 seconds).
- If a shortcut is missing or fails, show a short error once and turn the toggle off. Do not retry in a loop.
- Prefer turning off too often over leaving it on, because a stuck Focus would silence the phone while the user is away.

### User setup (document in README)

1. In System Settings → Notifications → Reminders, turn off notifications on the Mac.
2. Create the "Wane" Focus mode as described above.
3. Create the two shortcuts.
4. Turn on the toggle in Wane's menu bar item.

## 9. Out of Scope (v1)

- Apple Reminders sync
- In-app schedule editor
- Different schedules for different weekdays
- Wane's own notifications or sounds

## 10. Acceptance Criteria

Use the default schedule.

1. At 12:20: current task is *Lunch*, next task is *Back to work*. The gauge is about 67% full (40 of 60 minutes remaining).
2. At 13:00:00: the current task switches to *Back to work* within a second, and the gauge is full again with *Finish work* as the next task.
3. At 23:30: current task is *Bed*, next task is *Wake up* at 07:00 the next day.
4. At 06:00: current task is *Bed* (from the previous day), next task is *Wake up*.
5. Close the Mac lid at 10:00 and open it at 15:00. The correct state (*Back to work* → *Finish work*) appears without a stale frame.
6. Edit an entry's time in `schedule.toml` and save. The window updates without a restart. Saving an invalid file shows an error and keeps the old schedule.
7. Hovering shows the remaining time. With no hover, no numbers are visible.
8. Turning Always on Top off lets other windows cover Wane. Turning it on puts Wane back on top.
9. With the Focus option on: unlocking the Mac turns the Wane Focus on, and it shows on iPhone. Locking the Mac, sleeping, or being idle for N minutes turns it off.
