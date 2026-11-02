use std::sync::Mutex;

use tauri::{Listener, Manager};

use watch::ScheduleState;

pub mod display;
pub mod editor;
pub mod focus;
pub mod login_item;
mod menu;
pub mod model;
pub mod schedule;
pub mod settings;
pub mod tick;
pub mod watch;
pub mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(window::state_plugin())
        .setup(|app| {
            // tao sets ActivationPolicy::Regular by default, which overrides LSUIElement in Info.plist.
            // Set Accessory at runtime as well so no Dock icon appears (Spec §7).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            window::setup(app)?;
            let settings = menu::setup(app, app.path().data_dir()?)?;

            // Silence iPhone/Watch While Using Mac (Spec §8). Launch counts as "in use".
            let handle = app.handle().clone();
            let focus = focus::spawn(
                settings.silence_devices,
                settings.idle_after(),
                focus::run_shortcut,
                focus::idle_seconds,
                move |e| {
                    eprintln!("focus: {e}");
                    // Menu updates wait for the main thread; doing them from this
                    // worker while holding the settings lock could deadlock with
                    // a menu click, so run the whole handler there.
                    let h = handle.clone();
                    if let Err(err) =
                        handle.run_on_main_thread(move || menu::silence_failed(&h, &e))
                    {
                        eprintln!("focus: cannot report failure: {err}");
                    }
                },
            );
            #[cfg(target_os = "macos")]
            focus::observe_system_changes(&focus);
            app.manage(focus);

            let path = schedule::path_in(&app.path().data_dir()?);
            let mut state = ScheduleState::default();
            state.apply(schedule::load_or_create(&path));
            if let Some(err) = state.error() {
                eprintln!("schedule: {err}");
            }
            app.manage(Mutex::new(state));
            app.manage(editor::SchedulePath(path.clone()));

            // The frontend asks for its first draw once it is listening (Spec §5)
            let handle = app.handle().clone();
            app.listen(display::READY_EVENT, move |_| {
                let state = handle.state::<Mutex<ScheduleState>>();
                let state = state.lock().unwrap_or_else(|e| e.into_inner());
                display::emit_all(&handle, &state);
            });

            let handle = app.handle().clone();
            let watch_path = path.clone();
            let watcher = watch::watch(&path, move || {
                let state = handle.state::<Mutex<ScheduleState>>();
                let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                watch::reload(&watch_path, &mut state);
                if let Some(err) = state.error() {
                    eprintln!("schedule: {err}");
                }
                // Redraw with the reloaded schedule (Spec §6)
                display::emit_all(&handle, &state);
            });
            // Minute timer plus wake / clock / time zone changes (Spec §6).
            // App launch is covered by the frontend's ready event.
            let handle = app.handle().clone();
            let poke = tick::spawn(move || {
                let state = handle.state::<Mutex<ScheduleState>>();
                let state = state.lock().unwrap_or_else(|e| e.into_inner());
                display::emit_state(&handle, &state);
            });
            #[cfg(target_os = "macos")]
            tick::observe_system_changes(poke);
            #[cfg(not(target_os = "macos"))]
            drop(poke);

            match watcher {
                // Keep the watcher alive for the app's lifetime
                Ok(w) => {
                    app.manage(w);
                }
                Err(e) => eprintln!("schedule: cannot watch {}: {e}", path.display()),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            editor::load_schedule,
            editor::save_schedule
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Never leave the Focus on after quitting (Spec §8)
            if let tauri::RunEvent::Exit = event
                && let Some(focus) = app.try_state::<focus::Focus>()
            {
                focus.shutdown(focus::OFF_TIMEOUT);
            }
        });
}
