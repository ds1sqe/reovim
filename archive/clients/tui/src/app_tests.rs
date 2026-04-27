use super::*;

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_tui_app_error_display() {
    let err = TuiAppError::Disconnected;
    assert_eq!(err.to_string(), "Server disconnected");

    let err = TuiAppError::StreamEnded;
    assert_eq!(err.to_string(), "Notification stream ended");
}
