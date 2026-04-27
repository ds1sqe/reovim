use super::*;

const SAMPLE_FRAME: &str = "hello world\nsecond line\nthird line\n NORMAL  1:1";

#[test]
fn test_assert_frame_contains_passes() {
    assert_frame_contains(SAMPLE_FRAME, "hello");
    assert_frame_contains(SAMPLE_FRAME, "second");
    assert_frame_contains(SAMPLE_FRAME, "NORMAL");
}

#[test]
#[should_panic(expected = "Frame should contain")]
fn test_assert_frame_contains_fails() {
    assert_frame_contains(SAMPLE_FRAME, "not present");
}

#[test]
fn test_assert_frame_not_contains_passes() {
    assert_frame_not_contains(SAMPLE_FRAME, "INSERT");
    assert_frame_not_contains(SAMPLE_FRAME, "xyz");
}

#[test]
#[should_panic(expected = "Frame should NOT contain")]
fn test_assert_frame_not_contains_fails() {
    assert_frame_not_contains(SAMPLE_FRAME, "hello");
}

#[test]
fn test_assert_frame_line_contains_passes() {
    assert_frame_line_contains(SAMPLE_FRAME, 0, "hello");
    assert_frame_line_contains(SAMPLE_FRAME, 1, "second");
    assert_frame_line_contains(SAMPLE_FRAME, 2, "third");
}

#[test]
#[should_panic(expected = "Line 0 should contain")]
fn test_assert_frame_line_contains_wrong_content() {
    assert_frame_line_contains(SAMPLE_FRAME, 0, "not here");
}

#[test]
fn test_assert_statusline_mode_passes() {
    assert_statusline_mode(SAMPLE_FRAME, "NORMAL");
    assert_statusline_mode(SAMPLE_FRAME, "normal"); // case insensitive
}

#[test]
#[should_panic(expected = "Statusline should show mode")]
fn test_assert_statusline_mode_fails() {
    assert_statusline_mode(SAMPLE_FRAME, "INSERT");
}

#[test]
fn test_frame_contains_non_panicking() {
    assert!(frame_contains(SAMPLE_FRAME, "hello"));
    assert!(!frame_contains(SAMPLE_FRAME, "not present"));
}

#[test]
fn test_frame_line_contains_non_panicking() {
    assert!(frame_line_contains(SAMPLE_FRAME, 0, "hello"));
    assert!(!frame_line_contains(SAMPLE_FRAME, 0, "second"));
    assert!(!frame_line_contains(SAMPLE_FRAME, 100, "anything"));
}

#[test]
fn test_get_statusline() {
    assert_eq!(get_statusline(SAMPLE_FRAME), Some(" NORMAL  1:1"));
    assert_eq!(get_statusline(""), None);
}

#[test]
fn test_get_line() {
    assert_eq!(get_line(SAMPLE_FRAME, 0), Some("hello world"));
    assert_eq!(get_line(SAMPLE_FRAME, 1), Some("second line"));
    assert_eq!(get_line(SAMPLE_FRAME, 100), None);
}
