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
