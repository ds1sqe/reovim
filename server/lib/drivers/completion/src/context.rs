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
mod tests {
    use super::*;

    fn make_context() -> CompletionContext {
        CompletionContext {
            content: "fn main() {}".to_string(),
            cursor_offset: 3,
            line: 0,
            col: 3,
            prefix: "mai".to_string(),
            buffer_id: 1,
            file_path: Some("/tmp/test.rs".to_string()),
            language_id: Some("rust".to_string()),
        }
    }

    #[test]
    fn context_debug() {
        let ctx = make_context();
        let debug = format!("{ctx:?}");
        assert!(debug.contains("CompletionContext"));
        assert!(debug.contains("mai"));
    }

    #[test]
    fn context_clone() {
        let ctx = make_context();
        #[allow(clippy::redundant_clone)]
        let cloned = ctx.clone();
        assert_eq!(cloned.content, "fn main() {}");
        assert_eq!(cloned.cursor_offset, 3);
        assert_eq!(cloned.line, 0);
        assert_eq!(cloned.col, 3);
        assert_eq!(cloned.prefix, "mai");
        assert_eq!(cloned.buffer_id, 1);
        assert_eq!(cloned.file_path.as_deref(), Some("/tmp/test.rs"));
        assert_eq!(cloned.language_id.as_deref(), Some("rust"));
    }

    #[test]
    fn context_no_file_path() {
        let ctx = CompletionContext {
            file_path: None,
            language_id: None,
            ..make_context()
        };
        assert!(ctx.file_path.is_none());
        assert!(ctx.language_id.is_none());
    }

    #[test]
    fn context_empty_prefix() {
        let ctx = CompletionContext {
            prefix: String::new(),
            cursor_offset: 0,
            col: 0,
            ..make_context()
        };
        assert!(ctx.prefix.is_empty());
    }
}
