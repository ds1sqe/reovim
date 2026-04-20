use super::*;

#[test]
fn test_display_not_initialized() {
    let err = DisplayError::NotInitialized;
    assert_eq!(format!("{err}"), "display driver not initialized");
}

#[test]
fn test_display_invalid_size() {
    let err = DisplayError::InvalidSize {
        width: 0,
        height: 0,
    };
    assert_eq!(format!("{err}"), "invalid terminal size: 0x0");
}

#[test]
fn test_display_render_failed() {
    let err = DisplayError::RenderFailed("test error".to_string());
    assert_eq!(format!("{err}"), "render failed: test error");
}

#[test]
fn test_display_window_not_found() {
    let err = DisplayError::WindowNotFound(42);
    assert_eq!(format!("{err}"), "window not found: 42");
}

#[test]
fn test_display_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
    let err = DisplayError::Io(io_err);
    let msg = format!("{err}");
    assert!(msg.starts_with("I/O error:"));
    assert!(msg.contains("pipe broke"));
}

#[test]
fn test_display_compositor_error() {
    let err = DisplayError::CompositorError("layer conflict".to_string());
    assert_eq!(format!("{err}"), "compositor error: layer conflict");
}

#[test]
fn test_display_cursor_out_of_bounds() {
    let err = DisplayError::CursorOutOfBounds { x: 100, y: 50 };
    assert_eq!(format!("{err}"), "cursor position out of bounds: (100, 50)");
}

#[test]
fn test_error_source_io() {
    use std::error::Error;
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err = DisplayError::Io(io_err);
    assert!(err.source().is_some());
}

#[test]
fn test_error_source_non_io() {
    use std::error::Error;
    let err = DisplayError::NotInitialized;
    assert!(err.source().is_none());

    let err = DisplayError::RenderFailed("msg".to_string());
    assert!(err.source().is_none());

    let err = DisplayError::WindowNotFound(1);
    assert!(err.source().is_none());

    let err = DisplayError::CompositorError("msg".to_string());
    assert!(err.source().is_none());

    let err = DisplayError::CursorOutOfBounds { x: 0, y: 0 };
    assert!(err.source().is_none());

    let err = DisplayError::InvalidSize {
        width: 0,
        height: 0,
    };
    assert!(err.source().is_none());
}

#[test]
fn test_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: DisplayError = io_err.into();
    assert!(matches!(err, DisplayError::Io(_)));
}

#[test]
fn test_debug_impl() {
    let err = DisplayError::NotInitialized;
    let debug = format!("{err:?}");
    assert!(debug.contains("NotInitialized"));
}
