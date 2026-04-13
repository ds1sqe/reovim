use super::*;

#[test]
fn display_not_available() {
    let err = ClipboardError::NotAvailable("no display".into());
    assert!(err.to_string().contains("not available"));
    assert!(err.to_string().contains("no display"));
}

#[test]
fn display_read_failed() {
    let err = ClipboardError::ReadFailed("timeout".into());
    assert!(err.to_string().contains("read clipboard"));
    assert!(err.to_string().contains("timeout"));
}

#[test]
fn display_write_failed() {
    let err = ClipboardError::WriteFailed("locked".into());
    assert!(err.to_string().contains("write clipboard"));
    assert!(err.to_string().contains("locked"));
}

#[test]
fn display_platform() {
    let err = ClipboardError::Platform("wayland error".into());
    assert!(err.to_string().contains("Platform"));
    assert!(err.to_string().contains("wayland error"));
}

#[test]
fn debug() {
    let err = ClipboardError::NotAvailable("test".into());
    let debug = format!("{err:?}");
    assert!(debug.contains("NotAvailable"));
}

#[test]
fn is_error() {
    let err = ClipboardError::ReadFailed("test".into());
    let _: &dyn std::error::Error = &err;
}
