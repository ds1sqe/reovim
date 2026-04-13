//! Line number display mode configuration.
//!
//! This enum controls how line numbers are displayed in the gutter.
//! It is a pure data type with no presentation dependencies.

/// Line number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberMode {
    /// No line numbers.
    #[default]
    None,
    /// Absolute line numbers (1, 2, 3...).
    Absolute,
    /// Relative line numbers (distance from cursor).
    Relative,
    /// Hybrid: absolute for cursor line, relative for others.
    Hybrid,
}

#[cfg(test)]
#[path = "line_number_mode_tests.rs"]
mod tests;
