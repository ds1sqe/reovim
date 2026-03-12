use super::*;

#[test]
fn test_window_error_display() {
    let err = WindowError::CannotCloseLastWindow;
    assert_eq!(err.to_string(), "cannot close last window");

    let window_id = WindowId::new();
    let err = WindowError::NotFound(window_id);
    assert!(err.to_string().contains("window not found"));

    let buffer_id = BufferId::new();
    let err = WindowError::BufferNotFound(buffer_id);
    assert!(err.to_string().contains("buffer not found"));
}

#[test]
fn test_window_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(WindowError::CannotCloseLastWindow);
    assert_eq!(err.to_string(), "cannot close last window");
}

#[test]
fn test_window_error_debug() {
    let err = WindowError::CannotCloseLastWindow;
    let debug = format!("{err:?}");
    assert!(debug.contains("CannotCloseLastWindow"));
}

#[test]
fn test_window_error_clone() {
    let err = WindowError::CannotCloseLastWindow;
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_window_error_eq() {
    assert_eq!(WindowError::CannotCloseLastWindow, WindowError::CannotCloseLastWindow);
}

#[test]
fn test_window_error_not_found_display_format() {
    let id = WindowId::new();
    let err = WindowError::NotFound(id);
    let msg = err.to_string();
    assert!(msg.starts_with("window not found:"));
}

#[test]
fn test_window_error_buffer_not_found_display_format() {
    let id = BufferId::new();
    let err = WindowError::BufferNotFound(id);
    let msg = err.to_string();
    assert!(msg.starts_with("buffer not found:"));
}
