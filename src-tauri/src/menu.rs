//! Menu bar item (Spec §7 "Menu bar item").

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime};

use crate::settings::{self, Settings};
use crate::{focus, login_item, window};

const SHOW_HIDE: &str = "show_hide";
const ALWAYS_ON_TOP: &str = "always_on_top";
const EDIT_SCHEDULE: &str = "edit_schedule";
const SILENCE_DEVICES: &str = "silence_devices";
const LAUNCH_AT_LOGIN: &str = "launch_at_login";
const QUIT: &str = "quit";

/// Managed state shared by the menu event handler.
struct MenuState<R: Runtime> {
    settings: Mutex<Settings>,
    always_on_top: CheckMenuItem<R>,
    silence_devices: CheckMenuItem<R>,
    launch_at_login: CheckMenuItem<R>,
    settings_path: PathBuf,
    schedule_path: PathBuf,
}

/// Loads the persisted toggles, applies them, and installs the menu bar item.
/// Returns the loaded settings for features started after the menu.
pub fn setup<R: Runtime>(app: &tauri::App<R>, data_dir: PathBuf) -> tauri::Result<Settings> {
    let settings_path = settings::path_in(&data_dir);
    let (mut settings, err) = settings::load(&settings_path);
    if let Some(w) = app.get_webview_window(window::MAIN)
        && let Err(e) = w.set_always_on_top(settings.always_on_top)
    {
        eprintln!("always on top: {e}");
    }
    match err {
        // Do not touch the login item based on fallback defaults: that could
        // re-enable it after the user turned it off.
        // Take the current system state instead, so saving an unrelated toggle
        // later does not persist a Launch at Login value the user never chose.
        Some(e) => {
            eprintln!("settings: using defaults: {e}");
            settings.launch_at_login = login_item::is_registered();
        }
        // Keep the login item in sync so the default (on) takes effect on first launch.
        None => {
            if let Err(e) = login_item::apply(settings.launch_at_login) {
                eprintln!("launch at login: {e}");
            }
        }
    }

    let check = |id: &str, text: &str, on: bool| {
        CheckMenuItem::with_id(app, id, text, true, on, None::<&str>)
    };
    let always_on_top = check(ALWAYS_ON_TOP, "Always on Top", settings.always_on_top)?;
    let silence_devices = check(
        SILENCE_DEVICES,
        "Silence iPhone/Watch While Using Mac",
        settings.silence_devices,
    )?;
    let launch_at_login = check(LAUNCH_AT_LOGIN, "Launch at Login", settings.launch_at_login)?;

    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, SHOW_HIDE, "Show / Hide Window", true, None::<&str>)?,
            &always_on_top,
            &MenuItem::with_id(app, EDIT_SCHEDULE, "Edit Schedule…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &silence_devices,
            &launch_at_login,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit Wane", true, None::<&str>)?,
        ],
    )?;

    app.manage(MenuState {
        settings: Mutex::new(settings),
        always_on_top,
        silence_devices,
        launch_at_login,
        settings_path,
        schedule_path: crate::schedule::path_in(&data_dir),
    });

    let mut tray = TrayIconBuilder::with_id("wane")
        .tooltip("Wane")
        .menu(&menu)
        .on_menu_event(|app, event| handle(app, &event));
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(settings)
}

fn handle<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) {
    let id = event.id().as_ref();
    let result = match id {
        SHOW_HIDE => with_window(app, |w| window::toggle_visibility(w).map(|_| ())),
        EDIT_SCHEDULE => open_schedule(app),
        QUIT => {
            app.exit(0);
            Ok(())
        }
        ALWAYS_ON_TOP | SILENCE_DEVICES | LAUNCH_AT_LOGIN => toggle(app, id),
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("menu {id}: {e}");
    }
}

fn with_window<R: Runtime>(
    app: &AppHandle<R>,
    f: impl FnOnce(&tauri::WebviewWindow<R>) -> tauri::Result<()>,
) -> Result<(), String> {
    match app.get_webview_window(window::MAIN) {
        Some(w) => f(&w).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Opens `schedule.toml` in the default text editor.
fn open_schedule<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let state = app.state::<MenuState<R>>();
    std::process::Command::new("/usr/bin/open")
        .arg("-t")
        .arg(&state.schedule_path)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("open exited with {status}"))
            }
        })
}

/// Applies the check item's new state, then persists it. If applying fails,
/// the check mark is reverted and nothing is saved.
fn toggle<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), String> {
    let state = app.state::<MenuState<R>>();
    let mut settings = state.settings.lock().map_err(|e| e.to_string())?;
    let (current, item) = match id {
        ALWAYS_ON_TOP => (settings.always_on_top, &state.always_on_top),
        SILENCE_DEVICES => (settings.silence_devices, &state.silence_devices),
        _ => (settings.launch_at_login, &state.launch_at_login),
    };
    let on = !current;

    let applied = match id {
        ALWAYS_ON_TOP => with_window(app, |w| w.set_always_on_top(on)),
        SILENCE_DEVICES => {
            if let Some(focus) = app.try_state::<focus::Focus>() {
                focus.set_enabled(on);
            }
            Ok(())
        }
        _ => login_item::apply(on),
    };
    // macOS flips the check mark on click regardless of the outcome, so set it explicitly.
    if let Err(e) = applied {
        let _ = item.set_checked(current);
        return Err(e);
    }
    store(&state, &mut settings, id, on).map_err(|e| e.to_string())
}

/// Sets the check mark and the setting, then persists it.
fn store<R: Runtime>(
    state: &MenuState<R>,
    settings: &mut Settings,
    id: &str,
    on: bool,
) -> std::io::Result<()> {
    let item = match id {
        ALWAYS_ON_TOP => {
            settings.always_on_top = on;
            &state.always_on_top
        }
        SILENCE_DEVICES => {
            settings.silence_devices = on;
            &state.silence_devices
        }
        _ => {
            settings.launch_at_login = on;
            &state.launch_at_login
        }
    };
    let _ = item.set_checked(on);
    settings::save(&state.settings_path, settings)
}

/// A Focus shortcut failed (Spec §8): turn the toggle off, persist it, and
/// tell the user once. The worker has already stopped, so nothing retries.
pub fn silence_failed<R: Runtime>(app: &AppHandle<R>, error: &str) {
    // The user may have turned the toggle back on before this ran; keep the
    // worker in line with the menu, which is about to say "off".
    if let Some(focus) = app.try_state::<focus::Focus>() {
        focus.set_enabled(false);
    }
    let state = app.state::<MenuState<R>>();
    let mut settings = state.settings.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = store(&state, &mut settings, SILENCE_DEVICES, false) {
        eprintln!("settings: {e}");
    }
    drop(settings);
    show_alert(&format!(
        "Could not run the Focus shortcut, so \"Silence iPhone/Watch While Using Mac\" was turned off.\n\n{error}"
    ));
}

/// Shows a short alert without blocking the caller. The text goes in as an
/// argument so it is never parsed as AppleScript.
fn show_alert(message: &str) {
    let spawned = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display alert \"Wane\" message (item 1 of argv)",
            "-e",
            "end run",
            message,
        ])
        .spawn();
    if let Err(e) = spawned {
        eprintln!("alert: {e}: {message}");
    }
}
