use super::*;

#[test]
fn test_mode_error_display() {
    let err = ModeError::CannotPopHomeMode;
    assert_eq!(err.to_string(), "cannot pop home mode");
}

#[test]
fn test_mode_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(ModeError::CannotPopHomeMode);
    assert_eq!(err.to_string(), "cannot pop home mode");
}

#[test]
fn test_mode_error_debug() {
    let err = ModeError::CannotPopHomeMode;
    let debug = format!("{err:?}");
    assert!(debug.contains("CannotPopHomeMode"));
}

#[test]
fn test_mode_error_clone() {
    let err = ModeError::CannotPopHomeMode;
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_mode_error_eq() {
    assert_eq!(ModeError::CannotPopHomeMode, ModeError::CannotPopHomeMode);
}
