//! Assertion macros for cleaner test syntax.
//!
//! These macros provide a more readable way to write assertions
//! in integration tests.

/// Assert buffer content equals expected.
///
/// # Example
///
/// ```ignore
/// assert_buffer_eq!(result, "expected content");
/// ```
#[macro_export]
macro_rules! assert_buffer_eq {
    ($result:expr, $expected:expr) => {
        $result.assert_buffer_eq($expected)
    };
}

/// Assert cursor position (line, col) - matches vim convention.
///
/// # Example
///
/// ```ignore
/// assert_cursor!(result, 0, 5);  // Line 0, column 5
/// ```
#[macro_export]
macro_rules! assert_cursor {
    ($result:expr, $line:expr, $col:expr) => {
        $result.assert_cursor($line, $col)
    };
}

/// Assert register content and type.
///
/// # Example
///
/// ```ignore
/// assert_register!(result, "\"", "yanked text", "linewise");
/// ```
#[macro_export]
macro_rules! assert_register {
    ($result:expr, $reg:expr, $content:expr, $yank_type:expr) => {
        $result.assert_register($reg, $content, $yank_type)
    };
}

/// Assert current mode.
///
/// # Example
///
/// ```ignore
/// assert_mode!(result, "normal");
/// assert_mode!(result, "insert");
/// assert_mode!(result, "visual");
/// ```
#[macro_export]
macro_rules! assert_mode {
    ($result:expr, "normal") => {
        $result.assert_normal_mode()
    };
    ($result:expr, "insert") => {
        $result.assert_insert_mode()
    };
    ($result:expr, "visual") => {
        $result.assert_visual_mode()
    };
}

/// Assert operation completes within duration.
///
/// # Example
///
/// ```ignore
/// assert_completes_within!(Duration::from_secs(1), async_operation().await);
/// ```
#[macro_export]
macro_rules! assert_completes_within {
    ($duration:expr, $operation:expr) => {{
        let start = std::time::Instant::now();
        let result = $operation;
        let elapsed = start.elapsed();
        assert!(elapsed < $duration, "Operation took {:?}, expected < {:?}", elapsed, $duration);
        result
    }};
}

// Re-export for use in mod.rs
pub use {assert_buffer_eq, assert_completes_within, assert_cursor, assert_mode, assert_register};
