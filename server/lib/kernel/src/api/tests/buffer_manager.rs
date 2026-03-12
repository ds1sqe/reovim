use super::*;

// ========== BufferError tests ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_buffer_error_not_found_display() {
    let id = BufferId::from_raw(42);
    let err = BufferError::NotFound(id);
    let display = format!("{err}");
    assert!(display.contains("buffer not found"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_buffer_error_already_exists_display() {
    let id = BufferId::from_raw(7);
    let err = BufferError::AlreadyExists(id);
    let display = format!("{err}");
    assert!(display.contains("buffer already exists"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_buffer_error_invalid_operation_display() {
    let err = BufferError::InvalidOperation("cannot delete last buffer");
    let display = format!("{err}");
    assert_eq!(display, "invalid operation: cannot delete last buffer");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_buffer_error_debug() {
    let id = BufferId::from_raw(1);
    let err = BufferError::NotFound(id);
    let debug = format!("{err:?}");
    assert!(debug.contains("NotFound"));
}

#[test]
fn test_buffer_error_clone() {
    let id = BufferId::from_raw(5);
    let err = BufferError::NotFound(id);
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_buffer_error_eq() {
    let id = BufferId::from_raw(1);
    assert_eq!(BufferError::NotFound(id), BufferError::NotFound(id));
    assert_ne!(BufferError::NotFound(id), BufferError::InvalidOperation("test"));
}

#[test]
fn test_buffer_error_is_std_error() {
    let id = BufferId::from_raw(1);
    let err: Box<dyn std::error::Error> = Box::new(BufferError::NotFound(id));
    // Verify it implements std::error::Error
    let _ = format!("{err}");
}
