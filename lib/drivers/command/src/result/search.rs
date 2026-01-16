//! Search-related types for command results.
//!
//! # Policy-Adjacent Types
//!
//! These types (`SearchDirection`, `SearchAction`) are closer to policy than
//! pure mechanism. They remain here because:
//! 1. No dedicated search module exists yet
//! 2. They are callback intents (commands declare WHAT, runner decides HOW)
//! 3. Moving them would create circular dependencies
//!
//! Future refactoring may extract these to dedicated modules.

/// Search direction for / and ? commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchDirection {
    /// Search forward from cursor (/)
    #[default]
    Forward,
    /// Search backward from cursor (?)
    Backward,
}

/// Search action intent returned by commands.
///
/// Commands return this to request search operations. The runner handles
/// the actual search execution, input mode management, and pattern storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    /// Enter search input mode (/ or ?)
    EnterSearchMode { direction: SearchDirection },
    /// Go to next match in the same direction (n)
    Next,
    /// Go to previous match / reverse direction (N)
    Previous,
    /// Search word under cursor (* or #)
    WordUnderCursor { direction: SearchDirection },
    /// Clear search highlighting (:noh)
    ClearHighlight,
}
