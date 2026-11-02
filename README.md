# Wane

Wane is a small, always-on-top macOS app that shows how much time is left until the next item in your daily routine. The remaining time is shown as a shrinking circle instead of numbers. Hover over the window to see the remaining time and the next task's clock time.

Wane is **standalone**. The schedule lives in Wane's own config file. Wane does not read from or write to Apple Reminders, and it needs no Reminders permission.

The intended setup:

- **At the Mac**: follow the routine with Wane.
- **Away from the Mac**: keep using Apple Reminders on iPhone / Apple Watch for notifications.

Optionally, Wane can silence Reminders on iPhone / Watch while you use the Mac, so you do not get the same reminder on both. See [Silence iPhone/Watch while using the Mac](#silence-iphonewatch-while-using-the-mac).

Requires macOS 14 (Sonoma) or later.

## Menu bar item

Wane has no Dock icon. Use its menu bar item:

- **Show / Hide** the window
- **Always on Top** (on by default)
- **Edit Schedule…** opens an editor window to add, edit and delete entries
- **Open Schedule File…** opens `schedule.toml` in your default text editor (the editor window does not keep comments in the file)
- **Silence iPhone/Watch While Using Mac** (off by default)
- **Launch at Login** (on by default)
- **Quit**

## Schedule

The schedule is stored at:

```
~/Library/Application Support/Wane/schedule.toml
```

Edit it from **Edit Schedule…**, or open it in a text editor from **Open Schedule File…**. Wane creates this file with a default schedule on first launch. Wane watches the file and reloads it when you save it, so there is no need to restart. If the file is invalid, Wane keeps using the last valid schedule and shows a short error in the window.

Each `[[entry]]` has a `time` (`HH:MM`, 24-hour, local time) and a `title`. The time is when that activity **starts**:

```toml
[[entry]]
time = "07:00"
title = "Wake up"

[[entry]]
time = "12:00"
title = "Lunch"

[[entry]]
time = "23:00"
title = "Bed"
```

Rules:

- The same schedule repeats every day.
- Entries can be in any order; Wane sorts them by time.
- Titles must not be empty.
- Two entries with the same time are an error.
- At least 2 entries are required.

## Silence iPhone/Watch while using the Mac

While the Mac is in use, Wane turns on a shared Focus that silences Reminders, so your iPhone and Watch stay quiet. When you lock the screen, the Mac sleeps, the screen saver starts, you are idle for 5 minutes, or Wane quits, Wane turns the Focus off again so reminders reach your phone.

Wane switches the Focus by running two Shortcuts that you create. One-time setup:

1. **Turn off Reminders notifications on the Mac.** In System Settings → Notifications → Reminders, turn off "Allow notifications". Wane is the reminder on the Mac.
2. **Create a "Wane" Focus.** In System Settings → Focus, add a custom Focus named "Wane". Under "Allowed Notifications", set it to silence only the Reminders app (allow all other apps and people). Turn on **Share Across Devices** so the Focus also applies to your iPhone and Watch.
3. **Create two shortcuts** in the Shortcuts app, named exactly:
   - `Wane Focus On`: one "Set Focus" action that turns **Wane** **On**.
   - `Wane Focus Off`: one "Set Focus" action that turns **Wane** **Off**.

   You can test them from Terminal with `shortcuts run "Wane Focus On"`.
4. **Turn on the toggle.** In Wane's menu bar item, turn on **Silence iPhone/Watch While Using Mac**.

If a shortcut is missing or fails, Wane shows a short error once and turns the toggle off. Fix the shortcut, then turn the toggle on again.

The idle time defaults to 5 minutes. You can change it with `idle_minutes` in `~/Library/Application Support/Wane/settings.json` (minimum 1). Quit Wane before editing this file and start it again afterwards; unlike the schedule, settings are read only at launch, and Wane rewrites the file when you change a menu toggle.

## Development

Requirements: [Rust](https://www.rust-lang.org/tools/install), Node.js, and the [Tauri prerequisites for macOS](https://v2.tauri.app/start/prerequisites/).

```sh
npm install          # install dependencies
npm run tauri dev    # run the app in development mode
npm test             # run the frontend tests
cd src-tauri && cargo test   # run the Rust tests
```

The requirements spec is [`docs/spec.md`](docs/spec.md).
