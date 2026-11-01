#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // tao は既定で ActivationPolicy::Regular を設定し Info.plist の LSUIElement を上書きするため、
            // 実行時にも Accessory を明示して Dock アイコンを出さない (Spec §7)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
