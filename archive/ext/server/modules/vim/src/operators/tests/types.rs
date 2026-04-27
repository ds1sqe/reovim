use {
    super::super::*,
    reovim_domain_text::{HistoryRing, Position, RegisterBank},
    reovim_kernel::api::v1::BufferId,
};

#[test]
fn test_range_new() {
    let range = Range::new(Position::new(0, 0), Position::new(1, 5));
    assert_eq!(range.start, Position::new(0, 0));
    assert_eq!(range.end, Position::new(1, 5));
}

#[test]
fn test_range_is_empty() {
    let empty = Range::from_position(Position::new(5, 10));
    assert!(empty.is_empty());

    let not_empty = Range::new(Position::new(0, 0), Position::new(0, 1));
    assert!(!not_empty.is_empty());
}

#[test]
fn test_range_line_count() {
    let single = Range::new(Position::new(5, 0), Position::new(5, 10));
    assert!(single.is_single_line());
    assert_eq!(single.line_count(), 1);

    let multi = Range::new(Position::new(0, 0), Position::new(3, 0));
    assert!(!multi.is_single_line());
    assert_eq!(multi.line_count(), 4);
}

#[test]
fn test_range_normalized() {
    // Already normalized
    let range = Range::new(Position::new(0, 0), Position::new(1, 5));
    let norm = range.normalized();
    assert_eq!(norm.start, Position::new(0, 0));
    assert_eq!(norm.end, Position::new(1, 5));

    // Needs normalization
    let reversed = Range::new(Position::new(1, 5), Position::new(0, 0));
    let norm = reversed.normalized();
    assert_eq!(norm.start, Position::new(0, 0));
    assert_eq!(norm.end, Position::new(1, 5));
}

#[test]
fn test_operator_error_display() {
    let err = OperatorError::OperationFailed("test".into());
    assert!(err.to_string().contains("operation failed"));
}

// ========================================================================
// Additional Range tests for coverage
// ========================================================================

#[test]
fn test_range_new_is_characterwise() {
    let range = Range::new(Position::new(0, 0), Position::new(1, 5));
    assert!(!range.is_linewise);
}

#[test]
fn test_range_linewise() {
    let range = Range::linewise(Position::new(0, 0), Position::new(3, 0));
    assert!(range.is_linewise);
    assert_eq!(range.start, Position::new(0, 0));
    assert_eq!(range.end, Position::new(3, 0));
}

#[test]
fn test_range_from_position() {
    let pos = Position::new(5, 10);
    let range = Range::from_position(pos);
    assert_eq!(range.start, pos);
    assert_eq!(range.end, pos);
    assert!(!range.is_linewise);
    assert!(range.is_empty());
}

#[test]
fn test_range_to_linewise() {
    let range = Range::new(Position::new(1, 5), Position::new(3, 10));
    assert!(!range.is_linewise);

    let linewise = range.to_linewise();
    assert!(linewise.is_linewise);
    assert_eq!(linewise.start, Position::new(1, 5));
    assert_eq!(linewise.end, Position::new(3, 10));
}

#[test]
fn test_range_is_single_line_same_line() {
    let range = Range::new(Position::new(5, 0), Position::new(5, 10));
    assert!(range.is_single_line());
}

#[test]
fn test_range_is_single_line_different_lines() {
    let range = Range::new(Position::new(5, 0), Position::new(6, 0));
    assert!(!range.is_single_line());
}

#[test]
fn test_range_line_count_zero() {
    // When end < start (inverted range before normalization)
    let range = Range::new(Position::new(5, 0), Position::new(3, 0));
    assert_eq!(range.line_count(), 0);
}

#[test]
fn test_range_line_count_one() {
    let range = Range::new(Position::new(5, 0), Position::new(5, 10));
    assert_eq!(range.line_count(), 1);
}

#[test]
fn test_range_line_count_many() {
    let range = Range::new(Position::new(0, 0), Position::new(9, 0));
    assert_eq!(range.line_count(), 10);
}

#[test]
fn test_range_normalized_same_line_reversed_columns() {
    let range = Range::new(Position::new(5, 10), Position::new(5, 2));
    let norm = range.normalized();
    assert_eq!(norm.start, Position::new(5, 2));
    assert_eq!(norm.end, Position::new(5, 10));
}

#[test]
fn test_range_normalized_preserves_linewise() {
    let range = Range::linewise(Position::new(5, 0), Position::new(2, 0));
    let norm = range.normalized();
    assert!(norm.is_linewise);
    assert_eq!(norm.start, Position::new(2, 0));
    assert_eq!(norm.end, Position::new(5, 0));
}

#[test]
fn test_range_normalized_already_normalized() {
    let range = Range::new(Position::new(0, 0), Position::new(5, 10));
    let norm = range.normalized();
    assert_eq!(norm.start, range.start);
    assert_eq!(norm.end, range.end);
}

#[test]
fn test_range_empty_different_lines() {
    // Range spanning different lines is never empty
    let range = Range::new(Position::new(0, 0), Position::new(1, 0));
    assert!(!range.is_empty());
}

#[test]
fn test_range_copy_clone() {
    let range = Range::new(Position::new(1, 2), Position::new(3, 4));
    let copied = range;
    assert_eq!(copied, range);
}

#[test]
fn test_range_debug() {
    let range = Range::new(Position::new(0, 0), Position::new(1, 0));
    let debug_str = format!("{range:?}");
    assert!(debug_str.contains("Range"));
}

// ========================================================================
// OperatorError additional tests
// ========================================================================

#[test]
fn test_operator_error_buffer_not_found_display() {
    let err = OperatorError::BufferNotFound(BufferId::new());
    let display = err.to_string();
    assert!(display.contains("buffer not found"));
}

#[test]
fn test_operator_error_invalid_range_display() {
    let range = Range::new(Position::new(0, 0), Position::new(1, 0));
    let err = OperatorError::InvalidRange(range);
    let display = err.to_string();
    assert!(display.contains("invalid range"));
}

#[test]
fn test_operator_error_is_error_trait() {
    let err = OperatorError::OperationFailed("test".into());
    // Verify it implements std::error::Error
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_operator_error_clone() {
    let err = OperatorError::OperationFailed("test".into());
    let cloned = err.clone();
    assert_eq!(err.to_string(), cloned.to_string());
}

#[test]
fn test_operator_error_debug() {
    let err = OperatorError::OperationFailed("test".into());
    let debug_str = format!("{err:?}");
    assert!(debug_str.contains("OperationFailed"));
}

// ========================================================================
// OperatorContext field tests
// ========================================================================

#[test]
fn test_operator_context_fields() {
    use reovim_domain_text::Register;
    let kernel = reovim_kernel::api::v1::KernelContext::default();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let op_ctx = OperatorContext {
        kernel: &kernel,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id: BufferId::from_raw(42),
        register: Register::Slot('a'),
        count: 3,
        cursor_position: Position::new(5, 10),
        cursor_after: None,
    };
    assert_eq!(op_ctx.buffer_id, BufferId::from_raw(42));
    assert_eq!(op_ctx.register, Register::Slot('a'));
    assert_eq!(op_ctx.count, 3);
    assert_eq!(op_ctx.cursor_position, Position::new(5, 10));
}

#[test]
fn test_operator_context_default_register() {
    use reovim_domain_text::Register;
    let kernel = reovim_kernel::api::v1::KernelContext::default();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let op_ctx = OperatorContext {
        kernel: &kernel,
        registers: &mut registers,
        clipboard_history: &mut clipboard_history,
        buffer_id: BufferId::from_raw(1),
        register: Register::Default,
        count: 1,
        cursor_position: Position::origin(),
        cursor_after: None,
    };
    assert_eq!(op_ctx.register, Register::Default);
    assert_eq!(op_ctx.count, 1);
}

// ========================================================================
// Range edge cases
// ========================================================================

#[test]
fn test_range_eq() {
    let a = Range::new(Position::new(0, 0), Position::new(1, 5));
    let b = Range::new(Position::new(0, 0), Position::new(1, 5));
    assert_eq!(a, b);
}

#[test]
fn test_range_ne_start() {
    let a = Range::new(Position::new(0, 0), Position::new(1, 5));
    let b = Range::new(Position::new(0, 1), Position::new(1, 5));
    assert_ne!(a, b);
}

#[test]
fn test_range_ne_end() {
    let a = Range::new(Position::new(0, 0), Position::new(1, 5));
    let b = Range::new(Position::new(0, 0), Position::new(1, 6));
    assert_ne!(a, b);
}

#[test]
fn test_range_ne_linewise() {
    let a = Range::new(Position::new(0, 0), Position::new(1, 5));
    let b = Range::linewise(Position::new(0, 0), Position::new(1, 5));
    assert_ne!(a, b);
}

#[test]
fn test_range_from_position_is_zero_width() {
    let range = Range::from_position(Position::new(0, 0));
    assert!(range.is_empty());
    assert!(range.is_single_line());
    assert_eq!(range.line_count(), 1);
}

#[test]
fn test_range_to_linewise_from_already_linewise() {
    let range = Range::linewise(Position::new(0, 0), Position::new(2, 0));
    let again = range.to_linewise();
    assert!(again.is_linewise);
    assert_eq!(again.start, range.start);
    assert_eq!(again.end, range.end);
}

#[test]
fn test_range_normalized_same_start_end() {
    let range = Range::from_position(Position::new(3, 7));
    let norm = range.normalized();
    assert_eq!(norm.start, Position::new(3, 7));
    assert_eq!(norm.end, Position::new(3, 7));
}

#[test]
fn test_range_line_count_two_lines() {
    let range = Range::new(Position::new(5, 0), Position::new(6, 0));
    assert_eq!(range.line_count(), 2);
}

#[test]
fn test_range_is_empty_with_different_columns() {
    let range = Range::new(Position::new(3, 5), Position::new(3, 10));
    assert!(!range.is_empty());
}

// ========================================================================
// OperatorError edge cases
// ========================================================================

#[test]
fn test_operator_error_operation_failed_empty_message() {
    let err = OperatorError::OperationFailed(String::new());
    let display = err.to_string();
    assert!(display.contains("operation failed"));
}

#[test]
fn test_operator_error_clone_buffer_not_found() {
    let err = OperatorError::BufferNotFound(BufferId::from_raw(42));
    let cloned = err.clone();
    assert_eq!(err.to_string(), cloned.to_string());
}

#[test]
fn test_operator_error_clone_invalid_range() {
    let range = Range::new(Position::new(0, 0), Position::new(1, 0));
    let err = OperatorError::InvalidRange(range);
    let cloned = err.clone();
    assert_eq!(err.to_string(), cloned.to_string());
}

#[test]
fn test_operator_error_debug_buffer_not_found() {
    let err = OperatorError::BufferNotFound(BufferId::from_raw(1));
    let debug = format!("{err:?}");
    assert!(debug.contains("BufferNotFound"));
}

#[test]
fn test_operator_error_debug_invalid_range() {
    let range = Range::new(Position::new(0, 0), Position::new(1, 0));
    let err = OperatorError::InvalidRange(range);
    let debug = format!("{err:?}");
    assert!(debug.contains("InvalidRange"));
}

#[test]
fn test_operator_error_source_is_none() {
    let err = OperatorError::OperationFailed("test".into());
    assert!(std::error::Error::source(&err).is_none());
}
