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
    reovim_driver_syntax::{
        FoldKind, FoldRange, HighlightSpan, Injection, SyntaxDriver, SyntaxEdit,
    },
    streaming_iterator::StreamingIterator,
    tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, Tree},
};

use crate::{CaptureMapper, InjectionManager};

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

    /// Injection manager for embedded language highlighting.
    ///
    /// Present when `injections_query` is provided. Coordinates highlighting
    /// of embedded languages (e.g., Rust code in Markdown fenced blocks).
    injection_manager: Option<Mutex<InjectionManager>>,

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
            injection_manager: None,
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

        // Create injection manager if injections query is provided
        let injection_manager = injections_query
            .as_ref()
            .map(|_| Mutex::new(InjectionManager::new(capture_mapper.clone())));

        Some(Self {
            language_id: language_id.into(),
            parser: Mutex::new(parser),
            tree: RwLock::new(None),
            content: RwLock::new(String::new()),
            highlight_query,
            folds_query,
            injections_query,
            injection_manager,
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
    ///
    /// Returns `true` if both an injections query and injection manager are
    /// present. The injection manager coordinates highlighting of embedded
    /// languages.
    #[must_use]
    pub const fn supports_injections(&self) -> bool {
        self.injections_query.is_some() && self.injection_manager.is_some()
    }

    /// Get a reference to the injection manager (for registering layers).
    ///
    /// Returns `None` if the driver was not created with an injections query.
    #[must_use]
    pub const fn injection_manager(&self) -> Option<&Mutex<InjectionManager>> {
        self.injection_manager.as_ref()
    }

    /// Check if this driver supports folds.
    #[must_use]
    pub const fn supports_folds(&self) -> bool {
        self.folds_query.is_some()
    }

    /// Map a tree-sitter node kind to a `FoldKind`.
    ///
    /// Determines the semantic type of a foldable region based on the parent
    /// node's kind. This allows the UI to display appropriate fold icons.
    fn node_to_fold_kind(node: Option<Node>) -> FoldKind {
        let Some(node) = node else {
            return FoldKind::Block;
        };

        match node.kind() {
            // Functions
            "function_item"
            | "function_definition"
            | "closure_expression"
            | "method_definition" => FoldKind::Function,
            // Classes/structs
            "struct_item" | "enum_item" | "impl_item" | "trait_item" | "union_item"
            | "class_definition" | "class_declaration" => FoldKind::Class,
            // Imports
            "use_declaration" | "import_statement" | "import_from_statement" => FoldKind::Import,
            // Comments
            "line_comment" | "block_comment" | "comment" => FoldKind::Comment,
            // Default: generic block
            _ => FoldKind::Block,
        }
    }

    /// Extract the first line of a node as preview text.
    ///
    /// Used to show a collapsed fold indicator with meaningful context,
    /// e.g., "fn main() { ... }" instead of just "{ ... }".
    fn extract_preview(content: &str, node: Node) -> String {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte().min(content.len());

        if start_byte >= end_byte || start_byte >= content.len() {
            return String::new();
        }

        let text = &content[start_byte..end_byte];
        text.lines()
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(80) // Limit preview length
            .collect()
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
        // ===== STEP 1: Get parent language highlights =====
        let parent_count;
        let mut highlights = {
            let tree_guard = self.tree.read();
            let Some(tree) = tree_guard.as_ref() else {
                return Vec::new();
            };

            let content = self.content.read();

            let mut cursor = self.query_cursor.lock();
            cursor.set_byte_range(byte_range.clone());

            let mut parent_highlights = Vec::new();
            let capture_names = self.highlight_query.capture_names();

            // Query parent highlights
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

                    parent_highlights.push(HighlightSpan::new(
                        node.start_byte(),
                        node.end_byte(),
                        group,
                    ));
                }
            }

            parent_count = parent_highlights.len();
            parent_highlights
            // Locks dropped here: cursor, content, tree_guard
        };

        // ===== STEP 2: Get injection highlights (if manager exists) =====
        let injection_count;
        if let Some(ref manager_mutex) = self.injection_manager {
            // Get injections (re-acquires tree/content/cursor locks internally)
            let injections = self.injections();

            if injections.is_empty() {
                injection_count = 0;
            } else {
                // Get content for injection highlighting
                let content = self.content.read();

                // Get injection highlights from manager
                let mut manager = manager_mutex.lock();
                let injection_highlights =
                    manager.highlight_injections(&injections, &content, byte_range.clone());

                injection_count = injection_highlights.len();
                highlights.extend(injection_highlights);
            }
        } else {
            injection_count = 0;
        }

        // ===== STEP 3: Sort merged highlights by position =====
        highlights.sort_by_key(|h| h.start_byte);

        tracing::trace!(
            language = %self.language_id,
            range = ?byte_range,
            parent_highlights = parent_count,
            injection_highlights = injection_count,
            total_highlights = highlights.len(),
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
        let Some(folds_query) = &self.folds_query else {
            return Vec::new();
        };

        // Get read locks
        let tree_guard = self.tree.read();
        let Some(tree) = tree_guard.as_ref() else {
            return Vec::new();
        };

        let content = self.content.read();

        // Use a new cursor to avoid contention with highlights() cursor
        let mut cursor = QueryCursor::new();

        let mut folds = Vec::new();

        // Query folds
        let mut matches = cursor.matches(folds_query, tree.root_node(), content.as_bytes());
        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let node = capture.node;

                // Get line positions
                let start_line = node.start_position().row;
                let end_line = node.end_position().row;

                // Skip single-line regions (not foldable)
                if end_line <= start_line {
                    continue;
                }

                // Determine fold kind: check node itself first (for comments, macros),
                // then fall back to parent (for blocks inside functions, impl, etc.)
                let node_kind = Self::node_to_fold_kind(Some(node));
                let kind = if node_kind == FoldKind::Block {
                    // Node itself is a generic block, check parent for context
                    Self::node_to_fold_kind(node.parent())
                } else {
                    // Node itself has a specific kind (comment, macro, etc.)
                    node_kind
                };

                // Extract preview (first line of fold region)
                let preview = Self::extract_preview(&content, node);

                #[allow(clippy::cast_possible_truncation)]
                folds.push(FoldRange::new(start_line as u32, end_line as u32, kind, preview));
            }
        }

        // Sort by start line
        folds.sort_by_key(|f| f.start_line);

        // Drop locks before tracing
        drop(content);
        drop(tree_guard);

        tracing::trace!(
            language = %self.language_id,
            fold_count = folds.len(),
            "TreeSitterDriver::folds completed"
        );

        folds
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

    #[test]
    fn test_driver_with_injections_has_manager() {
        // Driver with injections query should have an injection manager
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        // A minimal injections query (doesn't matter what it matches, just needs to compile)
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let mapper = Arc::new(CaptureMapper::new());
        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            Some(injections_query),
            mapper,
        )
        .unwrap();

        assert!(
            driver.supports_injections(),
            "Driver with injections query should support injections"
        );
        assert!(driver.injection_manager().is_some(), "Driver should have an injection manager");
    }

    #[test]
    fn test_driver_without_injections_no_manager() {
        // Basic driver without injections query should not have a manager
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mapper = Arc::new(CaptureMapper::new());

        let driver = TreeSitterDriver::new("rust", &language, highlight_query, mapper).unwrap();

        assert!(!driver.supports_injections(), "Basic driver should not support injections");
        assert!(
            driver.injection_manager().is_none(),
            "Basic driver should not have an injection manager"
        );
    }

    #[test]
    fn test_driver_with_queries_no_injections() {
        // Driver with folds but no injections query should not have a manager
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());
        let mapper = Arc::new(CaptureMapper::new());

        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            Some(folds_query),
            None, // No injections query
            mapper,
        )
        .unwrap();

        assert!(
            !driver.supports_injections(),
            "Driver without injections query should not support injections"
        );
        assert!(
            driver.injection_manager().is_none(),
            "Driver without injections query should not have an injection manager"
        );
    }

    #[test]
    fn test_highlights_with_registered_injection_layer() {
        use crate::InjectionLayer;

        // Create a driver with injection support
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        // A minimal injections query that will match string literals as injection content
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let mapper = Arc::new(CaptureMapper::new());
        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query.clone(),
            None,
            Some(injections_query),
            mapper.clone(),
        )
        .unwrap();

        // Register a Rust injection layer
        {
            let manager = driver.injection_manager().unwrap();
            let mut manager_guard = manager.lock();
            let layer = InjectionLayer::new("rust", &language, highlight_query, mapper).unwrap();
            manager_guard.register_layer(layer);
        }

        // Parse code (the string content won't actually match Rust syntax properly,
        // but this tests the wiring)
        driver.parse("let x = \"hello\";");

        let highlights = driver.highlights(0..100);

        // Should have parent highlights (identifier 'x', etc.)
        assert!(!highlights.is_empty(), "Expected parent highlights at minimum");
    }

    #[test]
    fn test_highlights_no_injection_layer_graceful_skip() {
        // Create a driver with injection support but no layers registered
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        // An injections query that matches something
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let mapper = Arc::new(CaptureMapper::new());
        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            Some(injections_query),
            mapper,
        )
        .unwrap();

        // Don't register any injection layers

        // Parse code - should not panic even though injections are detected but no layer exists
        driver.parse("let x = \"hello\";");

        let highlights = driver.highlights(0..100);

        // Should have parent highlights only (no panic, graceful handling)
        assert!(
            !highlights.is_empty(),
            "Expected parent highlights (injection skipped gracefully)"
        );
    }

    #[test]
    fn test_highlights_sorted_by_position() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mapper = Arc::new(CaptureMapper::new());

        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query, mapper).unwrap();

        driver.parse("let x = y; let z = w;");

        let highlights = driver.highlights(0..100);

        // Verify highlights are sorted by start_byte
        for i in 1..highlights.len() {
            assert!(
                highlights[i - 1].start_byte <= highlights[i].start_byte,
                "Highlights should be sorted by start_byte"
            );
        }
    }
}
