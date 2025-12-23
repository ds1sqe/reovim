//! Abstract syntax provider trait for buffer highlighting
//!
//! Core defines the trait, plugins provide implementations.
//! This allows different syntax backends (tree-sitter, regex, LSP semantic tokens, etc.)

use std::sync::Arc;

use crate::highlight::Highlight;

/// Edit information for incremental parsing
///
/// Contains byte offsets and positions needed for incremental re-parsing
/// after a buffer modification.
#[derive(Debug, Clone)]
pub struct EditInfo {
    /// Byte offset where the edit starts
    pub start_byte: usize,
    /// Byte offset where the old content ended
    pub old_end_byte: usize,
    /// Byte offset where the new content ends
    pub new_end_byte: usize,
    /// Row where the edit starts (0-indexed)
    pub start_row: u32,
    /// Column where the edit starts (0-indexed)
    pub start_col: u32,
    /// Row where old content ended
    pub old_end_row: u32,
    /// Column where old content ended
    pub old_end_col: u32,
    /// Row where new content ends
    pub new_end_row: u32,
    /// Column where new content ends
    pub new_end_col: u32,
}

/// Abstract syntax provider for buffer highlighting
///
/// Implementors provide parsing and highlight generation.
/// This allows different backends (tree-sitter, regex, LSP semantic tokens, etc.)
///
/// # Example
///
/// ```ignore
/// impl SyntaxProvider for TreeSitterSyntax {
///     fn language_id(&self) -> &str { &self.language_id }
///     fn highlight_range(&self, content: &str, start: u32, end: u32) -> Vec<Highlight> {
///         // Run tree-sitter query and return highlights
///     }
///     // ...
/// }
/// ```
pub trait SyntaxProvider: Send + Sync {
    /// Language identifier (e.g., "rust", "python", "javascript")
    fn language_id(&self) -> &str;

    /// Generate highlights for a range of lines
    ///
    /// # Arguments
    /// * `content` - Full buffer content
    /// * `start_line` - First line to highlight (0-indexed)
    /// * `end_line` - Last line to highlight (exclusive)
    ///
    /// # Returns
    /// Vector of highlights for the requested range
    fn highlight_range(&self, content: &str, start_line: u32, end_line: u32) -> Vec<Highlight>;

    /// Perform a full parse of buffer content
    ///
    /// Called on initial file open or when incremental parsing isn't possible.
    fn parse(&mut self, content: &str);

    /// Perform incremental parse after an edit
    ///
    /// Called after buffer modifications for efficient re-parsing.
    /// Falls back to full parse if incremental isn't supported.
    fn parse_incremental(&mut self, content: &str, edit: &EditInfo);

    /// Check if syntax provider has valid parse state
    ///
    /// Returns true if the provider has successfully parsed content
    /// and can provide highlights.
    fn is_parsed(&self) -> bool;
}

/// Factory for creating syntax providers
///
/// Plugins implement this trait to provide syntax highlighting for files.
/// The runtime uses this factory during file open to create and attach
/// syntax providers to buffers.
pub trait SyntaxFactory: Send + Sync {
    /// Create a syntax provider for the given file
    ///
    /// # Arguments
    /// * `file_path` - Path to the file being opened
    /// * `content` - Initial content of the file
    ///
    /// # Returns
    /// Some(syntax) if a language was detected and a provider was created,
    /// None if the file type is not supported.
    fn create_syntax(
        &self,
        file_path: &str,
        content: &str,
    ) -> Option<Box<dyn SyntaxProvider>>;

    /// Check if this factory supports the given file
    fn supports_file(&self, file_path: &str) -> bool;
}

/// Shared reference to a syntax factory
pub type SharedSyntaxFactory = Arc<dyn SyntaxFactory>;
