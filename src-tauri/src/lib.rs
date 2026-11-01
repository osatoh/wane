use std::sync::Mutex;

use tauri::{Emitter, Manager};

use watch::{ScheduleState, UpdatedEvent};

pub mod model;
pub mod schedule;
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

            let path = schedule::path_in(&app.path().data_dir()?);
            let mut state = ScheduleState::default();
            state.apply(schedule::load_or_create(&path));
            if let Some(err) = state.error() {
                eprintln!("schedule: {err}");
            }
            app.manage(Mutex::new(state));

            let handle = app.handle().clone();
            let watch_path = path.clone();
            let watcher = watch::watch(&path, move || {
                let state = handle.state::<Mutex<ScheduleState>>();
                let event = {
                    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                    watch::reload(&watch_path, &mut state);
                    state.event()
                };
                if let Some(err) = &event.error {
                    eprintln!("schedule: {err}");
                }
                // The frontend re-evaluates the state and redraws on this event (Spec §6)
                if let Err(e) = handle.emit(watch::UPDATED_EVENT, event) {
                    eprintln!("schedule: cannot emit update: {e}");
                }
            });
            // Minute timer plus wake / clock / time zone changes (Spec §6).
            // App launch is covered by the frontend's first draw.
            let handle = app.handle().clone();
            let poke = tick::spawn(move || {
                if let Err(e) = handle.emit(tick::TICK_EVENT, ()) {
                    eprintln!("tick: cannot emit: {e}");
                }
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
        .invoke_handler(tauri::generate_handler![schedule_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Current schedule error for the frontend's first draw; later changes arrive as events.
#[tauri::command]
fn schedule_status(state: tauri::State<'_, Mutex<ScheduleState>>) -> UpdatedEvent {
    state.lock().unwrap_or_else(|e| e.into_inner()).event()
}
