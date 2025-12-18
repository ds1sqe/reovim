//! Motion module for cursor movement types
//!
//! This module defines the `Motion` enum representing all possible cursor movements.
//! Motion calculations are implemented in `buffer::cursor::calculate_motion`.

/// All possible cursor motions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    // Character motions
    Left,
    Right,
    Up,
    Down,

    // Line motions
    LineStart,
    LineEnd,

    // Word motions
    WordForward,
    WordBackward,

    // Document motions
    DocumentStart,
    DocumentEnd,
}

impl Motion {
    /// Check if this motion operates on whole lines (linewise)
    ///
    /// Linewise motions delete/yank entire lines rather than character ranges.
    /// Used by operators like `dj` (delete current + next line) vs `dw` (delete word).
    #[must_use]
    pub const fn is_linewise(&self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::DocumentStart | Self::DocumentEnd)
    }

    /// Parse a key string into a motion
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "h" => Some(Self::Left),
            "l" => Some(Self::Right),
            "j" => Some(Self::Down),
            "k" => Some(Self::Up),
            "w" => Some(Self::WordForward),
            "b" => Some(Self::WordBackward),
            "0" => Some(Self::LineStart),
            "$" => Some(Self::LineEnd),
            "G" => Some(Self::DocumentEnd),
            "gg" => Some(Self::DocumentStart),
            _ => None,
        }
    }
}
