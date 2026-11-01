use tauri::Manager;

pub mod login_item;
mod menu;
pub mod model;
pub mod schedule;
pub mod settings;
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
            menu::setup(app, app.path().data_dir()?)?;

            // Display and file watching come in later issues; log the result for now.
            let loaded = app
                .path()
                .data_dir()
                .map_err(|e| e.to_string())
                .map(|dir| schedule::path_in(&dir))
                .and_then(|path| {
                    schedule::load_or_create(&path)
                        .map(|s| {
                            format!(
                                "loaded {} entries from {}",
                                s.entries().len(),
                                path.display()
                            )
                        })
                        .map_err(|e| e.to_string())
                });
            match loaded {
                Ok(msg) | Err(msg) => eprintln!("schedule: {msg}"),
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
