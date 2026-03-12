use super::*;

#[test]
fn test_headless_output_flush() {
    let mut output = HeadlessOutput;
    let fb = FrameBuffer::new(80, 24);
    assert!(output.flush(&fb).is_ok());
}

#[test]
fn test_headless_output_no_terminal_cursor() {
    let output = HeadlessOutput;
    assert!(!output.uses_terminal_cursor());
}

#[test]
fn test_headless_output_position_cursor() {
    let mut output = HeadlessOutput;
    // No-op - should not panic
    output.position_cursor(10, 20);
}

#[test]
fn test_headless_output_set_cursor_style() {
    let mut output = HeadlessOutput;
    // No-op - should not panic
    output.set_cursor_style(CursorStyleHint::Block);
}

#[test]
fn test_headless_output_set_cursor_visible() {
    let mut output = HeadlessOutput;
    // No-op - should not panic
    output.set_cursor_visible(true);
    output.set_cursor_visible(false);
}
