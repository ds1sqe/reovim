//! Completion context provided to sources.
//!
//! Contains all information a source needs to produce completion items.

/// Context provided to completion sources.
///
/// Immutable snapshot of editor state at the time of completion trigger.
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// Full buffer content.
    pub content: String,
    /// Cursor byte offset in `content`.
    pub cursor_offset: usize,
    /// Line number (0-indexed).
    pub line: usize,
    /// Column (0-indexed, character offset in line).
    pub col: usize,
    /// The word prefix at cursor (text from word start to cursor).
    pub prefix: String,
    /// Buffer ID.
    pub buffer_id: usize,
    /// File path (if the buffer has one).
    pub file_path: Option<String>,
    /// Language ID (e.g., "rust", "python").
    pub language_id: Option<String>,
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
