//! Find-char state extension for session.
//!
//! Provides shared find-char state that both the coordinator and repeat
//! handlers can access. Stores the last find-char operation for `;`/`,` repeat.
//!
//! Follows the [`SearchState`](super::SearchState) precedent.

use crate::SessionExtension;

/// Record of a find-char operation for repeat.
///
/// Policy-free: uses raw booleans, not vim-specific enum names.
/// Any editor paradigm with "find character on line" can use this.
///
/// Mapping from vim concepts:
///   f -> forward=true,  inclusive=true
///   F -> forward=false, inclusive=true
///   t -> forward=true,  inclusive=false
///   T -> forward=false, inclusive=false
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindCharRecord {
    /// The character that was searched for.
    char: char,
    /// true = forward (f/t), false = backward (F/T).
    forward: bool,
    /// true = land on char (f/F), false = land before char (t/T).
    inclusive: bool,
}

impl FindCharRecord {
    /// Create a new find-char record.
    #[must_use]
    pub const fn new(char: char, forward: bool, inclusive: bool) -> Self {
        Self {
            char,
            forward,
            inclusive,
        }
    }

    /// The character that was searched for.
    #[must_use]
    pub const fn char(&self) -> char {
        self.char
    }

    /// Whether the search was forward (true) or backward (false).
    #[must_use]
    pub const fn forward(&self) -> bool {
        self.forward
    }

    /// Whether the motion lands on the char (true) or before it (false).
    #[must_use]
    pub const fn inclusive(&self) -> bool {
        self.inclusive
    }

    /// Reverse direction for `,` (repeat in opposite direction).
    #[must_use]
    pub const fn reversed(&self) -> Self {
        Self {
            forward: !self.forward,
            char: self.char,
            inclusive: self.inclusive,
        }
    }
}

/// Per-client find-char state for repeat (`;`/`,`).
///
/// Written by the `DISPATCH_FIND_CHAR` coordinator command.
/// Read by `REPEAT_FIND_SAME` and `REPEAT_FIND_REVERSE` handlers.
///
/// Follows `SearchState` precedent -- accessor methods, private fields.
#[derive(Debug, Default)]
pub struct FindCharState {
    last: Option<FindCharRecord>,
}

impl SessionExtension for FindCharState {
    fn create() -> Self {
        Self::default()
    }
}

impl FindCharState {
    /// Record a find-char operation for later repeat.
    pub const fn record(&mut self, char: char, forward: bool, inclusive: bool) {
        self.last = Some(FindCharRecord::new(char, forward, inclusive));
    }

    /// Get the last find-char record for repeat (`;`).
    #[must_use]
    pub const fn last(&self) -> Option<&FindCharRecord> {
        self.last.as_ref()
    }
}
#[cfg(test)]
#[path = "tests/find_char.rs"]
mod tests;
