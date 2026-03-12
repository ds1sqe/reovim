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
#[path = "frame_tests.rs"]
mod tests;
