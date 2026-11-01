//! Window behavior (Spec §7): floating, always on top, all Spaces, persisted frame.

use tauri::{Manager, Runtime, WebviewWindow};
use tauri_plugin_window_state::{StateFlags, WindowExt};

/// Label of the single Wane window (see `tauri.conf.json`).
pub const MAIN: &str = "main";

/// Only the frame is restored across launches; visibility and
/// always-on-top are not part of the remembered state.
pub const PERSISTED_STATE: StateFlags = StateFlags::POSITION.union(StateFlags::SIZE);

/// Plugin that saves the window frame on exit and restores it on launch.
pub fn state_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_window_state::Builder::default()
        .with_state_flags(PERSISTED_STATE)
        .build()
}

/// Collection behavior that keeps the window on every Space and above
/// full-screen apps. Flags that contradict this are removed.
#[cfg(target_os = "macos")]
pub fn floating_collection_behavior(
    current: objc2_app_kit::NSWindowCollectionBehavior,
) -> objc2_app_kit::NSWindowCollectionBehavior {
    use objc2_app_kit::NSWindowCollectionBehavior as B;
    (current - B::MoveToActiveSpace - B::FullScreenPrimary)
        | B::CanJoinAllSpaces
        | B::FullScreenAuxiliary
}

/// Applies native tweaks Tauri's window options cannot express.
pub fn setup<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN) else {
        return Ok(());
    };
    #[cfg(target_os = "macos")]
    {
        let ptr = window.ns_window()?;
        // SAFETY: `setup` runs on the main thread and the pointer is the live
        // NSWindow owned by `window`.
        let ns_window = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
        ns_window
            .setCollectionBehavior(floating_collection_behavior(ns_window.collectionBehavior()));
    }
    // The window starts hidden (`visible: false`) so it does not flash at the
    // default position before the saved frame is applied.
    if let Err(e) = window.restore_state(PERSISTED_STATE) {
        eprintln!("window: could not restore frame: {e}");
    }
    window.show()
}

/// Flips Always on Top and returns the new value.
pub fn toggle_always_on_top<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<bool> {
    let on = !window.is_always_on_top()?;
    window.set_always_on_top(on)?;
    Ok(on)
}

/// Hides the window if it is visible, otherwise shows it. Returns the new visibility.
pub fn toggle_visibility<R: Runtime>(window: &WebviewWindow<R>) -> tauri::Result<bool> {
    if window.is_visible()? {
        window.hide()?;
        Ok(false)
    } else {
        window.show()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_only_position_and_size() {
        assert_eq!(
            PERSISTED_STATE.bits(),
            (StateFlags::POSITION | StateFlags::SIZE).bits()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn joins_all_spaces_and_full_screen_apps_keeping_other_flags() {
        use objc2_app_kit::NSWindowCollectionBehavior as B;
        let result = floating_collection_behavior(B::Managed);
        assert!(result.contains(B::CanJoinAllSpaces));
        assert!(result.contains(B::FullScreenAuxiliary));
        assert!(result.contains(B::Managed));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn drops_flags_that_conflict_with_all_spaces() {
        use objc2_app_kit::NSWindowCollectionBehavior as B;
        let result = floating_collection_behavior(B::MoveToActiveSpace | B::FullScreenPrimary);
        assert!(!result.contains(B::MoveToActiveSpace));
        assert!(!result.contains(B::FullScreenPrimary));
    }
}
