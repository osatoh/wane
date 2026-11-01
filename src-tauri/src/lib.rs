#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // tao sets ActivationPolicy::Regular by default, which overrides LSUIElement in Info.plist.
            // Set Accessory at runtime as well so no Dock icon appears (Spec §7).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
