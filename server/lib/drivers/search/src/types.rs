//! Search types - common types for search operations.

use reovim_types_text::Position;

/// Search result with match position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Start position of the match.
    pub start: Position,
    /// End position of the match.
    pub end: Position,
}

/// Search direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Search forward from cursor.
    #[default]
    Forward,
    /// Search backward from cursor.
    Backward,
}

/// Search error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// Invalid regex pattern.
    InvalidPattern(String),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPattern(msg) => write!(f, "E486: Invalid pattern: {msg}"),
        }
    }
}

impl std::error::Error for SearchError {}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
