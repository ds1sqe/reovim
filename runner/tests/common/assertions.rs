//! Assertion macros for cleaner test syntax.

/// Assert buffer content equals expected
#[macro_export]
macro_rules! assert_buffer_eq {
    ($result:expr, $expected:expr) => {
        $result.assert_buffer_eq($expected)
    };
}

/// Assert cursor position (line, col) - matches vim convention
#[macro_export]
macro_rules! assert_cursor {
    ($result:expr, $line:expr, $col:expr) => {
        $result.assert_cursor($line, $col)
    };
}

/// Assert register content and type
#[macro_export]
macro_rules! assert_register {
    ($result:expr, $reg:expr, $content:expr, $yank_type:expr) => {
        $result.assert_register($reg, $content, $yank_type)
    };
}

/// Assert current mode
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

/// Assert operation completes within duration
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
