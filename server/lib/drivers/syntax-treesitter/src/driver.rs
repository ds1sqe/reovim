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

// Lock guards need to be held for the duration of tree-sitter operations
#![allow(clippy::significant_drop_tightening)]
// The constructor is complex due to tree-sitter setup with multiple queries
#![allow(clippy::too_many_lines)]
// Doc comments contain tree-sitter specific terms that don't need backticks
#![allow(clippy::doc_markdown)]

use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use {
    parking_lot::{Mutex, RwLock},
    reovim_driver_syntax::{FoldRange, HighlightSpan, Injection, SyntaxDriver, SyntaxEdit},
    streaming_iterator::StreamingIterator,
    tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, Tree},
};

use crate::CaptureMapper;

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
    #[allow(dead_code)]
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
    /// * `capture_mapper` - Capture name to HighlightGroup mapper
    ///
    /// # Errors
    ///
    /// Returns `None` if the parser cannot be configured for the language.
    pub fn new(
        language_id: impl Into<String>,
        language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
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
            folds_query: None,
            injections_query: None,
            capture_mapper,
            query_cursor: Mutex::new(QueryCursor::new()),
            version: AtomicU64::new(0),
            parse_error: RwLock::new(None),
        })
    }

    /// Create a new tree-sitter driver with additional queries.
    ///
    /// # Arguments
    ///
    /// * `language_id` - Unique language identifier
    /// * `language` - Tree-sitter language grammar
    /// * `highlight_query` - Pre-compiled highlights query
    /// * `folds_query` - Optional pre-compiled folds query
    /// * `injections_query` - Optional pre-compiled injections query
    /// * `capture_mapper` - Capture name to HighlightGroup mapper
    pub fn with_queries(
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
        self.parse_error.read().clone()
    }

    /// Check if this driver supports injections.
    #[must_use]
    pub const fn supports_injections(&self) -> bool {
        self.injections_query.is_some()
    }

    /// Check if this driver supports folds.
    #[must_use]
    pub const fn supports_folds(&self) -> bool {
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
            let mut content_guard = self.content.write();
            *content_guard = content.to_string();
        }

        // Parse content
        let tree = {
            let mut parser = self.parser.lock();
            parser.parse(content, None)
        };

        // Update tree and version
        {
            let mut tree_guard = self.tree.write();
            *tree_guard = tree;
        }

        // Clear any previous error
        {
            let mut error_guard = self.parse_error.write();
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
            let mut tree_guard = self.tree.write();
            if let Some(ref mut tree) = *tree_guard {
                tree.edit(&input_edit);
            }
        }

        // Update content
        {
            let mut content_guard = self.content.write();
            *content_guard = content.to_string();
        }

        // Re-parse with old tree for incremental parsing
        let new_tree = {
            let tree_guard = self.tree.read();
            let old_tree = tree_guard.as_ref();

            let mut parser = self.parser.lock();
            parser.parse(content, old_tree)
        };

        // Update tree and version
        {
            let mut tree_guard = self.tree.write();
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
        let tree_guard = self.tree.read();
        let Some(tree) = tree_guard.as_ref() else {
            return Vec::new();
        };

        let content = self.content.read();

        // Prepare cursor
        let mut cursor = self.query_cursor.lock();
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

        // Drop locks before tracing to avoid holding them during logging
        drop(cursor);
        drop(content);
        drop(tree_guard);

        tracing::trace!(
            language = %self.language_id,
            range = ?byte_range,
            highlights_count = highlights.len(),
            "TreeSitterDriver::highlights completed"
        );

        highlights
    }

    fn injections(&self) -> Vec<Injection> {
        let Some(injections_query) = &self.injections_query else {
            return Vec::new();
        };

        // Get read locks
        let tree_guard = self.tree.read();
        let Some(tree) = tree_guard.as_ref() else {
            return Vec::new();
        };

        let content = self.content.read();

        // Prepare cursor
        let mut cursor = self.query_cursor.lock();

        let mut injections = Vec::new();
        let capture_names = injections_query.capture_names();

        // Find capture indices for injection patterns
        let mut content_idx: Option<u32> = None;
        let mut language_idx: Option<u32> = None;

        for (i, name) in capture_names.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            if *name == "injection.content" {
                content_idx = Some(i as u32);
            } else if *name == "injection.language" {
                language_idx = Some(i as u32);
            }
        }

        let Some(content_idx) = content_idx else {
            return Vec::new(); // No @injection.content capture
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
            if injection_language.is_none() {
                for property in injections_query.property_settings(match_.pattern_index) {
                    if &*property.key == "injection.language"
                        && let Some(value) = &property.value
                    {
                        injection_language = Some(value.to_string());
                    }
                }
            }

            // Create injection if we have both content and language
            #[allow(clippy::cast_possible_truncation)]
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

        // Drop locks before tracing to avoid holding them during logging
        drop(cursor);
        drop(content);
        drop(tree_guard);

        tracing::trace!(
            language = %self.language_id,
            injection_count = injections.len(),
            "TreeSitterDriver::injections completed"
        );

        injections
    }

    fn folds(&self) -> Vec<FoldRange> {
        // TODO: Implement fold detection (Phase 12.3)
        Vec::new()
    }

    fn indent_for(&self, _line: usize) -> Option<usize> {
        // TODO: Implement indentation hints (Phase 12.3)
        None
    }

    fn is_parsed(&self) -> bool {
        self.tree.read().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // Integration tests with real languages are in the treesitter-rust module
}
