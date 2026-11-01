//! Launch at Login via `SMAppService` (Spec §7 "Menu bar item").

/// Registration change needed to match the desired setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Register,
    Unregister,
    None,
}

/// Decides what to do given the desired setting and whether the app is
/// currently registered (enabled or awaiting the user's approval).
pub fn action(desired: bool, registered: bool) -> Action {
    match (desired, registered) {
        (true, false) => Action::Register,
        (false, true) => Action::Unregister,
        _ => Action::None,
    }
}

/// Registers or unregisters the app as a login item to match `enabled`.
#[cfg(target_os = "macos")]
pub fn apply(enabled: bool) -> Result<(), String> {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    // SAFETY: plain ServiceManagement calls with no pointer arguments.
    unsafe {
        let service = SMAppService::mainAppService();
        let status = service.status();
        let registered =
            status == SMAppServiceStatus::Enabled || status == SMAppServiceStatus::RequiresApproval;
        match action(enabled, registered) {
            Action::Register => service.registerAndReturnError(),
            Action::Unregister => service.unregisterAndReturnError(),
            Action::None => Ok(()),
        }
        .map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply(_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_when_wanted_and_not_registered() {
        assert_eq!(action(true, false), Action::Register);
    }

    #[test]
    fn unregisters_when_unwanted_and_registered() {
        assert_eq!(action(false, true), Action::Unregister);
    }

    #[test]
    fn does_nothing_when_already_in_sync() {
        assert_eq!(action(true, true), Action::None);
        assert_eq!(action(false, false), Action::None);
    }
}
