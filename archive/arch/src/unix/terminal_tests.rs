use super::*;

#[test]
fn test_terminal_creation() {
    let _term = UnixTerminal::new();
    // Just verify it can be created
}

#[test]
fn test_terminal_size() {
    let term = UnixTerminal::new();
    // In a CI environment without a TTY, this may fail
    // but it should at least not panic
    let _ = term.size();
}

#[test]
fn test_supports_keyboard_enhancement() {
    let term = UnixTerminal::new();
    // Just verify the method can be called
    let _ = term.supports_keyboard_enhancement();
}

#[test]
fn test_terminal_default() {
    let term = UnixTerminal::default();
    assert!(!term.keyboard_enhancement_enabled);
}

#[test]
fn test_keyboard_enhancement_initial_state() {
    let term = UnixTerminal::new();
    assert!(!term.keyboard_enhancement_enabled);
}

#[test]
fn test_enable_keyboard_enhancement_returns_ok() {
    // Even without a TTY, enable_keyboard_enhancement should return Ok
    // since it checks supports_keyboard_enhancement first
    let mut term = UnixTerminal::new();
    let result = term.enable_keyboard_enhancement();
    assert!(result.is_ok());
}

#[test]
fn test_disable_keyboard_enhancement_noop_when_not_enabled() {
    let mut term = UnixTerminal::new();
    // Should be a no-op when not enabled
    let result = term.disable_keyboard_enhancement();
    assert!(result.is_ok());
    assert!(!term.keyboard_enhancement_enabled);
}

#[test]
fn test_terminal_write_flush() {
    // Writing to stdout should work even without a TTY
    let mut term = UnixTerminal::new();
    let result = term.write(b"");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
    let flush_result = term.flush();
    assert!(flush_result.is_ok());
}

#[test]
fn test_terminal_write_str_via_trait() {
    use crate::traits::Terminal;
    let mut term = UnixTerminal::new();
    // write_str is the default trait method that calls write()
    let result = term.write_str("");
    assert!(result.is_ok());
}

#[test]
fn test_terminal_write_nonzero_bytes() {
    let mut term = UnixTerminal::new();
    // Writing some actual bytes to stdout (they'll go to the test runner)
    let result = term.write(b"test output");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 11);
}

#[test]
fn test_terminal_clear_all_variants() {
    // These may fail without a TTY, but should not panic
    let mut term = UnixTerminal::new();
    let _ = term.clear(ClearType::All);
    let _ = term.clear(ClearType::FromCursorDown);
    let _ = term.clear(ClearType::FromCursorUp);
    let _ = term.clear(ClearType::CurrentLine);
    let _ = term.clear(ClearType::UntilNewLine);
    let _ = term.clear(ClearType::Purge);
}

#[test]
fn test_terminal_cursor_operations() {
    // These may fail without a TTY, but should not panic
    let mut term = UnixTerminal::new();
    let _ = term.hide_cursor();
    let _ = term.show_cursor();
    let _ = term.move_cursor(0, 0);
}

#[test]
fn test_terminal_alternate_screen() {
    // These may fail without a TTY, but should not panic
    let mut term = UnixTerminal::new();
    let _ = term.enter_alternate_screen();
    let _ = term.leave_alternate_screen();
}

#[test]
fn test_terminal_mouse_capture() {
    // These may fail without a TTY, but should not panic
    let mut term = UnixTerminal::new();
    let _ = term.enable_mouse_capture();
    let _ = term.disable_mouse_capture();
}

#[test]
fn test_terminal_flush() {
    let mut term = UnixTerminal::new();
    let result = term.flush();
    assert!(result.is_ok());
}

#[test]
fn test_terminal_disable_keyboard_enhancement_after_enable() {
    let mut term = UnixTerminal::new();
    // Enable first (may be no-op without TTY support)
    let _ = term.enable_keyboard_enhancement();
    // Disable should work regardless
    let result = term.disable_keyboard_enhancement();
    assert!(result.is_ok());
    assert!(!term.keyboard_enhancement_enabled);
}
