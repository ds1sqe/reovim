//! Frame capture assertion helpers for TUI E2E tests.
//!
//! Provides utility functions for asserting on captured TUI frames.
//! These helpers make it easy to verify frame content, mode indicators,
//! and text positioning.
//!
//! # Example
//!
//! ```ignore
//! use reovim_testing::frame::*;
//!
//! let frame = tui.capture(ScreenFormat::PlainText).await?;
//! assert_frame_contains(&frame, "hello");
//! assert_statusline_mode(&frame, "INSERT");
//! ```

/// Assert frame contains text anywhere.
///
/// # Panics
///
/// Panics if `expected` text is not found in the frame.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn assert_frame_contains(frame: &str, expected: &str) {
    assert!(frame.contains(expected), "Frame should contain '{expected}'\n\nFrame:\n{frame}");
}

/// Assert frame does NOT contain text.
///
/// # Panics
///
/// Panics if `unexpected` text IS found in the frame.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn assert_frame_not_contains(frame: &str, unexpected: &str) {
    assert!(
        !frame.contains(unexpected),
        "Frame should NOT contain '{unexpected}'\n\nFrame:\n{frame}"
    );
}

/// Assert frame contains text at specific line (0-indexed).
///
/// # Panics
///
/// Panics if the line doesn't exist or doesn't contain the expected text.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn assert_frame_line_contains(frame: &str, line: usize, expected: &str) {
    let lines: Vec<&str> = frame.lines().collect();
    let len = lines.len();
    assert!(lines.get(line).is_some(), "Frame has {len} lines, but line {line} requested");
    let got = lines[line];
    assert!(
        got.contains(expected),
        "Line {line} should contain '{expected}'\nGot: '{got}'\n\nFull frame:\n{frame}"
    );
}

/// Assert statusline (last line) shows expected mode.
///
/// Performs case-insensitive comparison.
///
/// # Panics
///
/// Panics if the last line doesn't contain the mode string.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn assert_statusline_mode(frame: &str, mode: &str) {
    let last = frame.lines().last().unwrap_or("");
    assert!(
        last.to_lowercase().contains(&mode.to_lowercase()),
        "Statusline should show mode '{mode}'\nGot: '{last}'\n\nFull frame:\n{frame}"
    );
}

/// Assert frame has the expected number of lines.
///
/// # Panics
///
/// Panics if line count doesn't match.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn assert_frame_line_count(frame: &str, expected: usize) {
    let actual = frame.lines().count();
    assert_eq!(
        actual, expected,
        "Frame should have {expected} lines, has {actual}\n\nFrame:\n{frame}"
    );
}

/// Assert frame width is at least `min_width` characters.
///
/// Checks the longest line in the frame.
///
/// # Panics
///
/// Panics if no line meets the minimum width.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn assert_frame_min_width(frame: &str, min_width: usize) {
    let max_width = frame.lines().map(str::len).max().unwrap_or(0);
    assert!(
        max_width >= min_width,
        "Frame should have at least {min_width} width, max line has {max_width}\n\nFrame:\n{frame}"
    );
}

/// Check if frame contains text (non-panicking version).
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn frame_contains(frame: &str, expected: &str) -> bool {
    frame.contains(expected)
}

/// Check if frame line contains text (non-panicking version).
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn frame_line_contains(frame: &str, line: usize, expected: &str) -> bool {
    frame
        .lines()
        .nth(line)
        .is_some_and(|l| l.contains(expected))
}

/// Extract the statusline (last line) from a frame.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn get_statusline(frame: &str) -> Option<&str> {
    frame.lines().last()
}

/// Extract a specific line from a frame.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn get_line(frame: &str, line: usize) -> Option<&str> {
    frame.lines().nth(line)
}

#[cfg(test)]
mod tests {
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
}
