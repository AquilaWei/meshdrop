//! Keep already-granted permission checks separate from native permission prompts.
use tauri_plugin_notification::PermissionState;

/// Returns an existing grant without prompting; native check/request errors propagate.
/// Some Android plugin versions leave a request unresolved when permission is granted.
pub(crate) fn notification_permission(
    check: impl FnOnce() -> Result<PermissionState, String>,
    request: impl FnOnce() -> Result<PermissionState, String>,
) -> Result<PermissionState, String> {
    match check()? {
        PermissionState::Granted => Ok(PermissionState::Granted),
        _ => request(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granted_notifications_do_not_request_permission_again() {
        assert_eq!(
            notification_permission(
                || Ok(PermissionState::Granted),
                || Err("unexpected prompt".into())
            ),
            Ok(PermissionState::Granted)
        );
    }

    #[test]
    fn undecided_notifications_request_permission() {
        assert_eq!(
            notification_permission(
                || Ok(PermissionState::Prompt),
                || Ok(PermissionState::Granted)
            ),
            Ok(PermissionState::Granted)
        );
    }

    #[test]
    fn notification_check_errors_reach_the_caller() {
        assert_eq!(
            notification_permission(
                || Err("check failed".into()),
                || Ok(PermissionState::Granted)
            ),
            Err("check failed".into())
        );
    }

    #[test]
    fn notification_request_errors_reach_the_caller() {
        assert_eq!(
            notification_permission(
                || Ok(PermissionState::Prompt),
                || Err("request failed".into())
            ),
            Err("request failed".into())
        );
    }
}
