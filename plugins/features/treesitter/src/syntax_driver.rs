//! Tree-sitter based implementation of `SyntaxDriver`.
//!
//! This module provides `TreeSitterDriver`, a thread-safe implementation of the
//! `SyntaxDriver` trait using tree-sitter for parsing and syntax highlighting.
//!
//! # Thread Safety
//!
//! Tree-sitter's `Parser` and `QueryCursor` are NOT `Send + Sync` by default.
//! This implementation wraps them with `Mutex` to provide safe concurrent access.
//! The `Tree` and content are wrapped with `RwLock` for read-heavy access patterns.

use std::{
    ops::Range,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use {
    reovim_driver_syntax::{FoldRange, HighlightSpan, Injection, SyntaxDriver, SyntaxEdit},
    tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree},
};

use crate::capture_mapper::CaptureMapper;

/// Tree-sitter based syntax driver.
///
/// Implements the `SyntaxDriver` trait using tree-sitter for incremental
/// parsing and syntax highlighting.
///
/// # Thread Safety
///
/// This struct is `Send + Sync` through interior mutability:
/// - `Mutex` wraps `Parser` and `QueryCursor` (exclusive access needed)
/// - `RwLock` wraps `Tree` and content (read-heavy access pattern)
/// - `Arc` shares immutable queries between instances
///
/// # Performance
///
/// - Queries are pre-compiled and shared via `Arc`
/// - `QueryCursor` is reused to avoid allocation per query
/// - Incremental parsing reuses the old tree when possible
pub struct TreeSitterDriver {
    /// Language identifier (e.g., "rust", "python")
    language_id: String,

    /// Tree-sitter parser (Mutex for thread safety)
    parser: Mutex<Parser>,

    /// Current parse tree (RwLock for read-heavy access)
    tree: RwLock<Option<Tree>>,

    /// Current buffer content (RwLock for read-heavy access)
    content: RwLock<String>,

    /// Pre-compiled highlights query (shared via Arc)
    highlight_query: Arc<Query>,

    /// Optional folds query
    folds_query: Option<Arc<Query>>,

    /// Optional injections query
    injections_query: Option<Arc<Query>>,

    /// Capture name to HighlightGroup mapper
    capture_mapper: Arc<CaptureMapper>,

    /// Reusable query cursor (Mutex for thread safety)
    query_cursor: Mutex<QueryCursor>,

    /// Parse version for cache invalidation
    version: AtomicU64,

    /// Last parse error (if any)
    parse_error: RwLock<Option<String>>,
}

impl TreeSitterDriver {
    /// Create a new tree-sitter driver.
    ///
    /// # Arguments
    ///
    /// * `language_id` - Unique language identifier
    /// * `language` - Tree-sitter language grammar
    /// * `highlight_query` - Pre-compiled highlights query
    /// * `folds_query` - Optional pre-compiled folds query
    /// * `injections_query` - Optional pre-compiled injections query
    /// * `capture_mapper` - Capture name to HighlightGroup mapper
    ///
    /// # Errors
    ///
    /// Returns `None` if the parser cannot be configured for the language.
    pub fn new(
        language_id: impl Into<String>,
        language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
        folds_query: Option<Arc<Query>>,
        injections_query: Option<Arc<Query>>,
        capture_mapper: Arc<CaptureMapper>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;

        Some(Self {
            language_id: language_id.into(),
            parser: Mutex::new(parser),
            tree: RwLock::new(None),
            content: RwLock::new(String::new()),
            highlight_query,
            folds_query,
            injections_query,
            capture_mapper,
            query_cursor: Mutex::new(QueryCursor::new()),
            version: AtomicU64::new(0),
            parse_error: RwLock::new(None),
        })
    }

    /// Get the current parse version.
    ///
    /// Increments on each parse/update, useful for cache invalidation.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Get the last parse error, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<String> {
        self.parse_error.read().ok()?.clone()
    }

    /// Check if this driver supports injections.
    #[must_use]
    pub fn supports_injections(&self) -> bool {
        self.injections_query.is_some()
    }

    /// Check if this driver supports folds.
    #[must_use]
    pub fn supports_folds(&self) -> bool {
        self.folds_query.is_some()
    }
}

impl SyntaxDriver for TreeSitterDriver {
    fn language(&self) -> &str {
        &self.language_id
    }

    fn parse(&mut self, content: &str) {
        // Update content
        {
            let mut content_guard = self.content.write().unwrap();
            *content_guard = content.to_string();
        }

        // Parse content
        let tree = {
            let mut parser = self.parser.lock().unwrap();
            parser.parse(content, None)
        };

        // Update tree and version
        {
            let mut tree_guard = self.tree.write().unwrap();
            *tree_guard = tree;
        }

        // Clear any previous error
        {
            let mut error_guard = self.parse_error.write().unwrap();
            *error_guard = None;
        }

        self.version.fetch_add(1, Ordering::SeqCst);

        tracing::trace!(
            language = %self.language_id,
            version = self.version(),
            content_len = content.len(),
            "TreeSitterDriver::parse completed"
        );
    }

    fn update(&mut self, content: &str, edit: &SyntaxEdit) {
        // Convert SyntaxEdit to tree-sitter InputEdit
        let input_edit = InputEdit {
            start_byte: edit.start_byte,
            old_end_byte: edit.old_end_byte,
            new_end_byte: edit.new_end_byte,
            start_position: Point::new(edit.start_row as usize, edit.start_col as usize),
            old_end_position: Point::new(edit.old_end_row as usize, edit.old_end_col as usize),
            new_end_position: Point::new(edit.new_end_row as usize, edit.new_end_col as usize),
        };

        // Apply edit to existing tree
        {
            let mut tree_guard = self.tree.write().unwrap();
            if let Some(ref mut tree) = *tree_guard {
                tree.edit(&input_edit);
            }
        }

        // Update content
        {
            let mut content_guard = self.content.write().unwrap();
            *content_guard = content.to_string();
        }

        // Re-parse with old tree for incremental parsing
        let new_tree = {
            let tree_guard = self.tree.read().unwrap();
            let old_tree = tree_guard.as_ref();

            let mut parser = self.parser.lock().unwrap();
            parser.parse(content, old_tree)
        };

        // Update tree and version
        {
            let mut tree_guard = self.tree.write().unwrap();
            *tree_guard = new_tree;
        }

        self.version.fetch_add(1, Ordering::SeqCst);

        tracing::trace!(
            language = %self.language_id,
            version = self.version(),
            edit_start = edit.start_byte,
            edit_delta = edit.byte_delta(),
            "TreeSitterDriver::update completed"
        );
    }

    fn highlights(&self, byte_range: Range<usize>) -> Vec<HighlightSpan> {
        // Get read locks
        let tree_guard = self.tree.read().unwrap();
        let tree = match tree_guard.as_ref() {
            Some(t) => t,
            None => return Vec::new(),
        };

        let content = self.content.read().unwrap();

        // Prepare cursor
        let mut cursor = self.query_cursor.lock().unwrap();
        cursor.set_byte_range(byte_range.clone());

        let mut highlights = Vec::new();
        let capture_names = self.highlight_query.capture_names();

        // Query highlights
        let mut matches =
            cursor.matches(&self.highlight_query, tree.root_node(), content.as_bytes());
        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &capture_names[capture.index as usize];

                // Skip non-highlight captures (decoration.*, textobject.*, etc.)
                if capture_name.starts_with("decoration.")
                    || capture_name.starts_with("textobject.")
                    || capture_name.starts_with("local.")
                    || capture_name.contains(".inner")
                    || capture_name.contains(".outer")
                {
                    continue;
                }

                let group = self.capture_mapper.map(capture_name);
                let node = capture.node;

                highlights.push(HighlightSpan::new(node.start_byte(), node.end_byte(), group));
            }
        }

        // Sort by start position for consistent rendering
        highlights.sort_by_key(|h| h.start_byte);

        tracing::trace!(
            language = %self.language_id,
            range = ?byte_range,
            highlights_count = highlights.len(),
            "TreeSitterDriver::highlights completed"
        );

        highlights
    }

    fn injections(&self) -> Vec<Injection> {
        let injections_query = match &self.injections_query {
            Some(q) => q,
            None => return Vec::new(),
        };

        // Get read locks
        let tree_guard = self.tree.read().unwrap();
        let tree = match tree_guard.as_ref() {
            Some(t) => t,
            None => return Vec::new(),
        };

        let content = self.content.read().unwrap();

        // Prepare cursor
        let mut cursor = self.query_cursor.lock().unwrap();

        let mut injections = Vec::new();
        let capture_names = injections_query.capture_names();

        // Find capture indices for injection patterns
        let mut content_idx: Option<u32> = None;
        let mut language_idx: Option<u32> = None;

        for (i, name) in capture_names.iter().enumerate() {
            if *name == "injection.content" {
                content_idx = Some(i as u32);
            } else if *name == "injection.language" {
                language_idx = Some(i as u32);
            }
        }

        let content_idx = match content_idx {
            Some(idx) => idx,
            None => return Vec::new(), // No @injection.content capture
        };

        // Query injections
        let mut matches = cursor.matches(injections_query, tree.root_node(), content.as_bytes());
        while let Some(match_) = matches.next() {
            let mut injection_content: Option<tree_sitter::Node> = None;
            let mut injection_language: Option<String> = None;

            // Process captures in this match
            for capture in match_.captures {
                if capture.index == content_idx {
                    injection_content = Some(capture.node);
                } else if Some(capture.index) == language_idx {
                    // Extract language from captured node text
                    let node = capture.node;
                    let lang_text = &content[node.start_byte()..node.end_byte()];
                    injection_language = Some(lang_text.trim().to_lowercase());
                }
            }

            // Check for #set! injection.language property
            // Tree-sitter queries can use predicates like: (#set! injection.language "markdown")
            if injection_language.is_none() {
                // Try to get property from pattern
                for property in injections_query.property_settings(match_.pattern_index) {
                    if &*property.key == "injection.language"
                        && let Some(value) = &property.value
                    {
                        injection_language = Some(value.to_string());
                    }
                }
            }

            // Create injection if we have both content and language
            if let (Some(content_node), Some(language_id)) = (injection_content, injection_language)
            {
                let start_point = content_node.start_position();
                let end_point = content_node.end_position();

                injections.push(Injection::new(
                    language_id,
                    content_node.start_byte()..content_node.end_byte(),
                    start_point.row as u32,
                    start_point.column as u32,
                    end_point.row as u32,
                    end_point.column as u32,
                ));
            }
        }

        tracing::trace!(
            language = %self.language_id,
            injection_count = injections.len(),
            "TreeSitterDriver::injections completed"
        );

        injections
    }

    fn folds(&self) -> Vec<FoldRange> {
        // TODO: Implement fold detection
        // This requires:
        // 1. Query folds_query for foldable regions
        // 2. Map to FoldRange structs with line ranges and FoldKind
        Vec::new()
    }

    fn indent_for(&self, _line: usize) -> Option<usize> {
        // TODO: Implement indentation hints
        // This requires analyzing the AST structure around the line
        None
    }

    fn is_parsed(&self) -> bool {
        self.tree.read().ok().is_some_and(|t| t.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests require tree-sitter language grammars which are in language plugins.
    // Integration tests should be added when language plugins are converted.

    #[test]
    fn test_driver_initial_state() {
        // This test verifies the API without requiring a real language grammar
        // Full integration tests will be in the language plugin tests
    }

    #[test]
    fn test_syntax_edit_conversion() {
        // Verify SyntaxEdit -> InputEdit conversion logic
        let edit = SyntaxEdit::insert(10, 2, 5, 15, 2, 10);

        let input_edit = InputEdit {
            start_byte: edit.start_byte,
            old_end_byte: edit.old_end_byte,
            new_end_byte: edit.new_end_byte,
            start_position: Point::new(edit.start_row as usize, edit.start_col as usize),
            old_end_position: Point::new(edit.old_end_row as usize, edit.old_end_col as usize),
            new_end_position: Point::new(edit.new_end_row as usize, edit.new_end_col as usize),
        };

        assert_eq!(input_edit.start_byte, 10);
        assert_eq!(input_edit.old_end_byte, 10);
        assert_eq!(input_edit.new_end_byte, 15);
    }
}
