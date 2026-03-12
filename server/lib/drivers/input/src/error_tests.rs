use crate::{ClipboardError, InputError};

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_error_display() {
    assert_eq!(format!("{}", InputError::NotInitialized), "input driver not initialized");
    assert_eq!(format!("{}", InputError::AlreadyRunning), "input driver already running");
    assert_eq!(format!("{}", InputError::NotRunning), "input driver not running");
    assert!(format!("{}", InputError::StartFailed("test".into())).contains("test"));
    assert!(format!("{}", InputError::StopFailed("test".into())).contains("test"));
    assert!(format!("{}", InputError::InjectionFailed("test".into())).contains("test"));
    assert!(format!("{}", InputError::InvalidKeySequence("test".into())).contains("test"));
    assert!(format!("{}", InputError::KeymapError("test".into())).contains("test"));
    assert!(format!("{}", InputError::HandlerRegistrationFailed("test".into())).contains("test"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_clipboard_error_display() {
    assert_eq!(format!("{}", ClipboardError::NotAvailable), "clipboard not available");
    assert_eq!(format!("{}", ClipboardError::NotText), "clipboard content is not text");
    assert_eq!(format!("{}", ClipboardError::NoProvider), "no clipboard provider configured");
    assert!(format!("{}", ClipboardError::ReadFailed("test".into())).contains("test"));
    assert!(format!("{}", ClipboardError::WriteFailed("test".into())).contains("test"));
}

#[test]
fn test_input_error_is_std_error() {
    // Verify InputError implements std::error::Error
    fn assert_error<T: std::error::Error>() {}
    assert_error::<InputError>();
}

#[test]
fn test_clipboard_error_is_std_error() {
    // Verify ClipboardError implements std::error::Error
    fn assert_error<T: std::error::Error>() {}
    assert_error::<ClipboardError>();
}

#[test]
fn test_input_error_source_is_none() {
    // Default Error::source() returns None
    let err = InputError::NotInitialized;
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn test_clipboard_error_source_is_none() {
    let err = ClipboardError::NotAvailable;
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_error_debug() {
    let err = InputError::NotInitialized;
    let debug = format!("{err:?}");
    assert!(debug.contains("NotInitialized"));

    let err = InputError::StartFailed("oops".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("StartFailed"));
    assert!(debug.contains("oops"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_clipboard_error_debug() {
    let err = ClipboardError::NotAvailable;
    let debug = format!("{err:?}");
    assert!(debug.contains("NotAvailable"));

    let err = ClipboardError::ReadFailed("permission denied".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("ReadFailed"));
}
