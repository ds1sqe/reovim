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
        Annotation, DecorationCapture, DecorationRule, FoldKind, FoldRange, HighlightCategory,
        Injection, SyntaxDriver, SyntaxEdit, decoration::apply_rules,
    },
    streaming_iterator::StreamingIterator,
    tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, Tree},
};

use crate::InjectionManager;

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

    /// Optional indents query for indentation hints
    indents_query: Option<Arc<Query>>,

    /// Injection manager for embedded language highlighting.
    ///
    /// Present when `injections_query` is provided. Coordinates highlighting
    /// of embedded languages (e.g., Rust code in Markdown fenced blocks).
    injection_manager: Option<Mutex<InjectionManager>>,

    /// Optional decoration query for conceal/background/virtual text
    decoration_query: Option<Arc<Query>>,

    /// Declarative rules mapping capture names to annotation kinds
    decoration_rules: Vec<DecorationRule>,

    /// Optional inline/secondary parser (for dual-grammar languages like markdown)
    inline_parser: Option<Mutex<Parser>>,

    /// Cached inline parse tree
    inline_tree: RwLock<Option<Tree>>,

    /// Optional inline decoration query
    inline_decoration_query: Option<Arc<Query>>,

    /// Inline decoration rules
    inline_decoration_rules: Vec<DecorationRule>,

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
    ///
    /// # Errors
    ///
    /// Returns `None` if the parser cannot be configured for the language.
    pub fn new(
        language_id: impl Into<String>,
        language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
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
            indents_query: None,
            injection_manager: None,
            decoration_query: None,
            decoration_rules: Vec::new(),
            inline_parser: None,
            inline_tree: RwLock::new(None),
            inline_decoration_query: None,
            inline_decoration_rules: Vec::new(),
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
    /// * `indents_query` - Optional pre-compiled indents query for indentation hints
    pub fn with_queries(
        language_id: impl Into<String>,
        language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
        folds_query: Option<Arc<Query>>,
        injections_query: Option<Arc<Query>>,
        indents_query: Option<Arc<Query>>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;

        // Create injection manager if injections query is provided
        let injection_manager = injections_query
            .as_ref()
            .map(|_| Mutex::new(InjectionManager::new()));

        Some(Self {
            language_id: language_id.into(),
            parser: Mutex::new(parser),
            tree: RwLock::new(None),
            content: RwLock::new(String::new()),
            highlight_query,
            folds_query,
            injections_query,
            indents_query,
            injection_manager,
            decoration_query: None,
            decoration_rules: Vec::new(),
            inline_parser: None,
            inline_tree: RwLock::new(None),
            inline_decoration_query: None,
            inline_decoration_rules: Vec::new(),
            query_cursor: Mutex::new(QueryCursor::new()),
            version: AtomicU64::new(0),
            parse_error: RwLock::new(None),
        })
    }

    /// Create a builder for configuring a tree-sitter driver.
    ///
    /// The builder pattern is the recommended way to create drivers with
    /// decoration queries and other optional features.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let driver = TreeSitterDriver::builder("markdown", &language, highlight_query)
    ///     .injections_query(injections_query)
    ///     .decoration_query(deco_query)
    ///     .decoration_rules(rules)
    ///     .build()?;
    /// ```
    pub fn builder(
        language_id: impl Into<String>,
        language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
    ) -> TreeSitterDriverBuilder {
        TreeSitterDriverBuilder::new(language_id, language, highlight_query)
    }

    /// Check if this driver supports decorations.
    #[must_use]
    pub const fn supports_decorations(&self) -> bool {
        self.decoration_query.is_some()
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Set the injection layer store for dynamic layer creation.
    ///
    /// When set, the injection manager will lazily create layers for embedded
    /// languages by querying the store during `highlight_injections()`.
    ///
    /// Does nothing if this driver has no injection manager (i.e., was not
    /// created with an injections query).
    pub fn set_injection_layer_store(&self, store: Arc<crate::InjectionLayerStore>) {
        if let Some(ref manager_mutex) = self.injection_manager {
            manager_mutex.lock().set_store(store);
        }
    }

    /// Check if this driver supports folds.
    #[must_use]
    pub const fn supports_folds(&self) -> bool {
        self.folds_query.is_some()
    }

    /// Check if this driver supports indentation hints.
    #[must_use]
    pub const fn supports_indents(&self) -> bool {
        self.indents_query.is_some()
    }

    /// Map a tree-sitter node kind to a `FoldKind`.
    ///
    /// Determines the semantic type of a foldable region based on the parent
    /// node's kind. This allows the UI to display appropriate fold icons.
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Run a closure with read access to the parsed tree and buffer content.
    ///
    /// Returns `None` if no tree has been parsed yet (i.e., `parse()` was never
    /// called or parsing failed). Lock management is internal to the driver —
    /// the closure receives borrowed references, not lock guards.
    ///
    /// # Usage
    ///
    /// ```ignore
    /// let result = driver.with_tree(|tree, content| {
    ///     let mut cursor = QueryCursor::new();
    ///     let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
    ///     // process matches...
    ///     annotations
    /// });
    /// ```
    pub fn with_tree<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&Tree, &str) -> R,
    {
        let tree_guard = self.tree.read();
        let content_guard = self.content.read();
        tree_guard.as_ref().map(|tree| f(tree, &content_guard))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

        // Parse with inline parser if configured (dual-grammar support)
        if let Some(ref inline_parser) = self.inline_parser {
            let inline_tree = {
                let mut parser = inline_parser.lock();
                parser.parse(content, None)
            };
            let mut inline_tree_guard = self.inline_tree.write();
            *inline_tree_guard = inline_tree;
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

    fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation> {
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

                    let node = capture.node;

                    parent_highlights.push(Annotation::highlight(
                        node.start_byte(),
                        node.end_byte(),
                        HighlightCategory::new(*capture_name),
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

    fn indent_for(&self, line: usize) -> Option<usize> {
        let indents_query = self.indents_query.as_ref()?;

        // Get tree and content
        let tree_guard = self.tree.read();
        let tree = tree_guard.as_ref()?;
        let content = self.content.read();

        // Find the node at the start of this line
        let point = Point::new(line, 0);
        let node = tree.root_node().descendant_for_point_range(point, point)?;

        // Get capture names to identify @indent
        let capture_names = indents_query.capture_names();
        #[allow(clippy::cast_possible_truncation)]
        let indent_idx = capture_names
            .iter()
            .position(|n| *n == "indent")
            .map(|i| i as u32);

        let Some(indent_idx) = indent_idx else {
            return Some(0); // No @indent capture defined
        };

        // Count indent levels by walking up the tree
        let mut indent_level = 0;
        let mut current = Some(node);

        while let Some(n) = current {
            // Check if this node matches @indent
            let mut cursor = QueryCursor::new();
            cursor.set_point_range(n.start_position()..n.end_position());

            let mut matches = cursor.matches(indents_query, n, content.as_bytes());
            while let Some(m) = matches.next() {
                for cap in m.captures {
                    // Only count if this capture is for the current node
                    if cap.index == indent_idx && cap.node.id() == n.id() {
                        indent_level += 1;
                    }
                }
            }

            current = n.parent();
        }

        // Return indent level in spaces (4 spaces per level)
        // Note: The caller should handle configuration for different indent widths
        Some(indent_level * 4)
    }

    fn decorations(&self, byte_range: Range<usize>) -> Vec<Annotation> {
        let mut result = Vec::new();

        // Block-level decorations (primary grammar)
        if let Some(deco_query) = &self.decoration_query
            && let Some(block_decos) = self.with_tree(|tree, content| {
                let mut cursor = QueryCursor::new();
                cursor.set_byte_range(byte_range.clone());

                let capture_names = deco_query.capture_names();
                let mut captures = Vec::new();

                let mut matches =
                    cursor.matches(deco_query, tree.root_node(), content.as_bytes());
                while let Some(match_) = matches.next() {
                    for capture in match_.captures {
                        let name = &capture_names[capture.index as usize];
                        let node = capture.node;
                        captures.push(DecorationCapture {
                            name: Arc::from(*name),
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                        });
                    }
                }

                apply_rules(&captures, &self.decoration_rules)
            })
        {
            result.extend(block_decos);
        }

        // Inline decorations (secondary grammar, e.g., markdown inline)
        if let Some(inline_query) = &self.inline_decoration_query {
            let inline_tree_guard = self.inline_tree.read();
            if let Some(inline_tree) = inline_tree_guard.as_ref() {
                let content = self.content.read();
                let mut cursor = QueryCursor::new();
                cursor.set_byte_range(byte_range);

                let capture_names = inline_query.capture_names();
                let mut captures = Vec::new();

                let mut matches = cursor.matches(
                    inline_query,
                    inline_tree.root_node(),
                    content.as_bytes(),
                );
                while let Some(match_) = matches.next() {
                    for capture in match_.captures {
                        let name = &capture_names[capture.index as usize];
                        let node = capture.node;
                        captures.push(DecorationCapture {
                            name: Arc::from(*name),
                            start_byte: node.start_byte(),
                            end_byte: node.end_byte(),
                        });
                    }
                }

                result.extend(apply_rules(&captures, &self.inline_decoration_rules));
            }
        }

        result
    }

    fn is_parsed(&self) -> bool {
        self.tree.read().is_some()
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for creating `TreeSitterDriver` with optional capabilities.
///
/// Avoids growing positional parameter lists. Use via
/// [`TreeSitterDriver::builder()`].
pub struct TreeSitterDriverBuilder {
    language_id: String,
    language: tree_sitter::Language,
    highlight_query: Arc<Query>,
    folds_query: Option<Arc<Query>>,
    injections_query: Option<Arc<Query>>,
    indents_query: Option<Arc<Query>>,
    decoration_query: Option<Arc<Query>>,
    decoration_rules: Vec<DecorationRule>,
    inline_parser: Option<Mutex<Parser>>,
    inline_decoration_query: Option<Arc<Query>>,
    inline_decoration_rules: Vec<DecorationRule>,
}

impl TreeSitterDriverBuilder {
    /// Create a new builder with the required parameters.
    fn new(
        language_id: impl Into<String>,
        language: &tree_sitter::Language,
        highlight_query: Arc<Query>,
    ) -> Self {
        Self {
            language_id: language_id.into(),
            language: language.clone(),
            highlight_query,
            folds_query: None,
            injections_query: None,
            indents_query: None,
            decoration_query: None,
            decoration_rules: Vec::new(),
            inline_parser: None,
            inline_decoration_query: None,
            inline_decoration_rules: Vec::new(),
        }
    }

    /// Set the folds query.
    #[must_use]
    pub fn folds_query(mut self, query: Arc<Query>) -> Self {
        self.folds_query = Some(query);
        self
    }

    /// Set the injections query.
    #[must_use]
    pub fn injections_query(mut self, query: Arc<Query>) -> Self {
        self.injections_query = Some(query);
        self
    }

    /// Set the indents query.
    #[must_use]
    pub fn indents_query(mut self, query: Arc<Query>) -> Self {
        self.indents_query = Some(query);
        self
    }

    /// Set the decoration query and rules.
    ///
    /// The query defines what to capture; the rules define how captures
    /// map to annotation kinds (conceal, background, virtual text).
    #[must_use]
    pub fn decoration(mut self, query: Arc<Query>, rules: Vec<DecorationRule>) -> Self {
        self.decoration_query = Some(query);
        self.decoration_rules = rules;
        self
    }

    /// Set an inline/secondary grammar for decoration queries.
    ///
    /// Some languages have dual grammars (e.g., Markdown's block + inline).
    /// The inline parser runs on the same content as the primary parser, and
    /// its decoration query produces additional annotations (e.g., concealing
    /// emphasis markers `*`, bold markers `**`, link brackets, etc.).
    ///
    /// The inline parser's content is the full document — it parses the entire
    /// buffer (not just regions extracted from the primary tree).
    #[must_use]
    pub fn inline_decoration(
        mut self,
        language: &tree_sitter::Language,
        query: Arc<Query>,
        rules: Vec<DecorationRule>,
    ) -> Self {
        let mut parser = Parser::new();
        if parser.set_language(language).is_ok() {
            self.inline_parser = Some(Mutex::new(parser));
            self.inline_decoration_query = Some(query);
            self.inline_decoration_rules = rules;
        }
        self
    }

    /// Build the `TreeSitterDriver`.
    ///
    /// Returns `None` if the parser cannot be configured for the language.
    #[must_use]
    pub fn build(self) -> Option<TreeSitterDriver> {
        let mut parser = Parser::new();
        parser.set_language(&self.language).ok()?;

        let injection_manager = self
            .injections_query
            .as_ref()
            .map(|_| Mutex::new(InjectionManager::new()));

        Some(TreeSitterDriver {
            language_id: self.language_id,
            parser: Mutex::new(parser),
            tree: RwLock::new(None),
            content: RwLock::new(String::new()),
            highlight_query: self.highlight_query,
            folds_query: self.folds_query,
            injections_query: self.injections_query,
            indents_query: self.indents_query,
            injection_manager,
            decoration_query: self.decoration_query,
            decoration_rules: self.decoration_rules,
            inline_parser: self.inline_parser,
            inline_tree: RwLock::new(None),
            inline_decoration_query: self.inline_decoration_query,
            inline_decoration_rules: self.inline_decoration_rules,
            query_cursor: Mutex::new(QueryCursor::new()),
            version: AtomicU64::new(0),
            parse_error: RwLock::new(None),
        })
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_driver_with_injections_has_manager() {
        // Driver with injections query should have an injection manager
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        // A minimal injections query (doesn't matter what it matches, just needs to compile)
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            Some(injections_query),
            None, // No indents query
        )
        .unwrap();

        assert!(
            driver.supports_injections(),
            "Driver with injections query should support injections"
        );
        assert!(driver.injection_manager().is_some(), "Driver should have an injection manager");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_driver_without_injections_no_manager() {
        // Basic driver without injections query should not have a manager
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        assert!(!driver.supports_injections(), "Basic driver should not support injections");
        assert!(
            driver.injection_manager().is_none(),
            "Basic driver should not have an injection manager"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_driver_with_queries_no_injections() {
        // Driver with folds but no injections query should not have a manager
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());
        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            Some(folds_query),
            None, // No injections query
            None, // No indents query
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_highlights_with_registered_injection_layer() {
        use crate::InjectionLayer;

        // Create a driver with injection support
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        // A minimal injections query that will match string literals as injection content
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query.clone(),
            None,
            Some(injections_query),
            None, // No indents query
        )
        .unwrap();

        // Register a Rust injection layer
        {
            let manager = driver.injection_manager().unwrap();
            let mut manager_guard = manager.lock();
            let layer = InjectionLayer::new("rust", &language, highlight_query).unwrap();
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_highlights_no_injection_layer_graceful_skip() {
        // Create a driver with injection support but no layers registered
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        // An injections query that matches something
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            Some(injections_query),
            None, // No indents query
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_highlights_sorted_by_position() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

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

    #[test]
    fn test_driver_language() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        assert_eq!(driver.language(), "rust");
    }

    #[test]
    fn test_driver_not_parsed_initially() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        assert!(!driver.is_parsed());
    }

    #[test]
    fn test_driver_parsed_after_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("fn main() {}");
        assert!(driver.is_parsed());
    }

    #[test]
    fn test_driver_version_increments() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        assert_eq!(driver.version(), 0);
        driver.parse("fn main() {}");
        assert_eq!(driver.version(), 1);
        driver.parse("fn foo() {}");
        assert_eq!(driver.version(), 2);
    }

    #[test]
    fn test_driver_last_error_initially_none() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        assert!(driver.last_error().is_none());
    }

    #[test]
    fn test_driver_highlights_empty_before_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        let highlights = driver.highlights(0..100);
        assert!(highlights.is_empty());
    }

    #[test]
    fn test_driver_update_increments_version() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("fn main() {}");
        let v1 = driver.version();

        let edit = SyntaxEdit::insert(12, 0, 12, 20, 0, 20);
        driver.update("fn main() { let x; }", &edit);

        assert_eq!(driver.version(), v1 + 1);
    }

    #[test]
    fn test_driver_injections_empty_without_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        driver.parse("fn main() {}");

        assert!(driver.injections().is_empty());
    }

    #[test]
    fn test_driver_folds_empty_without_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        driver.parse("fn main() {\n    let x = 1;\n}");

        // No folds query, so folds should be empty
        assert!(driver.folds().is_empty());
    }

    #[test]
    fn test_driver_indent_for_none_without_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        driver.parse("fn main() {}");

        assert_eq!(driver.indent_for(0), None);
    }

    #[test]
    fn test_driver_supports_folds() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());

        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query.clone(),
            Some(folds_query),
            None,
            None,
        )
        .unwrap();

        assert!(driver.supports_folds());

        let driver2 = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        assert!(!driver2.supports_folds());
    }

    #[test]
    fn test_driver_supports_indents() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());

        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query.clone(),
            None,
            None,
            Some(indents_query),
        )
        .unwrap();

        assert!(driver.supports_indents());

        let driver2 = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();
        assert!(!driver2.supports_indents());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_driver_folds_with_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        // Query that captures blocks as fold regions
        let folds_query = Arc::new(Query::new(&language, "(block) @fold").unwrap());
        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            Some(folds_query),
            None,
            None,
        )
        .unwrap();

        driver.parse("fn main() {\n    let x = 1;\n}");

        let folds = driver.folds();
        // Should have at least the function body fold
        assert!(!folds.is_empty(), "Expected at least one fold for a multi-line function");
    }

    #[test]
    fn test_driver_folds_empty_before_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(block) @fold").unwrap());
        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            Some(folds_query),
            None,
            None,
        )
        .unwrap();

        // Not parsed yet, should return empty
        assert!(driver.folds().is_empty());
    }

    #[test]
    fn test_driver_indent_for_with_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());
        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            None,
            Some(indents_query),
        )
        .unwrap();

        driver.parse("fn main() {\n    let x = 1;\n}");

        // Line 1 is inside the function body (a block), should have some indent level
        let indent = driver.indent_for(1);
        assert!(indent.is_some());
    }

    #[test]
    fn test_driver_indent_for_before_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());
        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            None,
            Some(indents_query),
        )
        .unwrap();

        // Not parsed yet, should return None
        assert_eq!(driver.indent_for(0), None);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_driver_folds_sorted_by_start_line() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(block) @fold").unwrap());
        let mut driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            Some(folds_query),
            None,
            None,
        )
        .unwrap();

        driver.parse("fn foo() {\n    x\n}\n\nfn bar() {\n    y\n}");

        let folds = driver.folds();
        // Verify folds are sorted by start_line
        for i in 1..folds.len() {
            assert!(
                folds[i - 1].start_line <= folds[i].start_line,
                "Folds should be sorted by start_line"
            );
        }
    }

    #[test]
    fn test_driver_update_preserves_highlighting() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("let x = 1;");
        let h1 = driver.highlights(0..100);
        assert!(!h1.is_empty());

        // Update: add more code
        let edit = SyntaxEdit::insert(10, 0, 10, 22, 0, 22);
        driver.update("let x = 1; let y = 2;", &edit);

        let h2 = driver.highlights(0..100);
        // Should have more highlights after adding more identifiers
        assert!(h2.len() >= h1.len());
    }

    #[test]
    fn test_driver_last_error_cleared_after_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("fn main() {}");
        assert!(driver.last_error().is_none());
    }

    #[test]
    fn test_driver_injections_empty_before_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());
        let driver = TreeSitterDriver::with_queries(
            "rust",
            &language,
            highlight_query,
            None,
            Some(injections_query),
            None,
        )
        .unwrap();

        // Not parsed yet, should return empty
        assert!(driver.injections().is_empty());
    }

    #[test]
    fn test_with_tree_returns_none_before_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        let result = driver.with_tree(|_tree, _content| 42);
        assert!(result.is_none(), "with_tree should return None before parse");
    }

    #[test]
    fn test_with_tree_returns_some_after_parse() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("fn main() {}");

        let result = driver.with_tree(|tree, content| {
            assert!(!content.is_empty());
            assert_eq!(content, "fn main() {}");
            tree.root_node().kind().to_string()
        });
        assert_eq!(result, Some("source_file".to_string()));
    }

    #[test]
    fn test_with_tree_can_run_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("let x = 1;");

        let query = Query::new(&language, "(identifier) @name").unwrap();
        let count = driver.with_tree(|tree, content| {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
            let mut count = 0;
            while let Some(_m) = matches.next() {
                count += 1;
            }
            count
        });
        assert_eq!(count, Some(1), "Should find one identifier 'x'");
    }

    // ========================================================================
    // Builder Tests
    // ========================================================================

    #[test]
    fn test_builder_basic() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());

        let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .build()
            .unwrap();

        assert_eq!(driver.language(), "rust");
        assert!(!driver.supports_decorations());
        assert!(!driver.supports_folds());
        assert!(!driver.supports_injections());
        assert!(!driver.supports_indents());
    }

    #[test]
    fn test_builder_with_folds() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());

        let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .folds_query(folds_query)
            .build()
            .unwrap();

        assert!(driver.supports_folds());
    }

    #[test]
    fn test_builder_with_injections() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());

        let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .injections_query(injections_query)
            .build()
            .unwrap();

        assert!(driver.supports_injections());
    }

    #[test]
    fn test_builder_with_decoration() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "fn_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("test.background"),
        }];

        let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        assert!(driver.supports_decorations());
    }

    #[test]
    fn test_builder_all_options() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let folds_query = Arc::new(Query::new(&language, "(function_item) @fold").unwrap());
        let injections_query =
            Arc::new(Query::new(&language, "(string_literal) @injection.content").unwrap());
        let indents_query = Arc::new(Query::new(&language, "(block) @indent").unwrap());
        let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

        let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .folds_query(folds_query)
            .injections_query(injections_query)
            .indents_query(indents_query)
            .decoration(deco_query, Vec::new())
            .build()
            .unwrap();

        assert!(driver.supports_folds());
        assert!(driver.supports_injections());
        assert!(driver.supports_indents());
        assert!(driver.supports_decorations());
    }

    // ========================================================================
    // Decorations Tests
    // ========================================================================

    #[test]
    fn test_decorations_empty_without_query() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let mut driver = TreeSitterDriver::new("rust", &language, highlight_query).unwrap();

        driver.parse("fn main() {}");
        let decorations = driver.decorations(0..100);
        assert!(
            decorations.is_empty(),
            "Driver without decoration query should return empty"
        );
    }

    #[test]
    fn test_decorations_empty_before_parse() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "fn_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("test"),
        }];

        let driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        // Not parsed yet
        let decorations = driver.decorations(0..100);
        assert!(
            decorations.is_empty(),
            "Should return empty before parse"
        );
    }

    #[test]
    fn test_decorations_returns_annotations_after_parse() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        // Capture function_item nodes as background decorations
        let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "fn_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("test.fn_bg"),
        }];

        let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        driver.parse("fn main() {}");
        let decorations = driver.decorations(0..100);

        assert_eq!(decorations.len(), 1, "Should have one decoration for fn main");
        assert_eq!(decorations[0].kind, AnnotationKind::Background);
        assert_eq!(decorations[0].category.as_str(), "test.fn_bg");
        assert_eq!(decorations[0].start_byte, 0);
    }

    #[test]
    fn test_decorations_skips_unmatched_captures() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        // Query captures identifiers, but rule only matches "fn_block"
        let deco_query = Arc::new(Query::new(&language, "(identifier) @ident").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "fn_block".into(), // won't match "ident"
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("test"),
        }];

        let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        driver.parse("let x = 1;");
        let decorations = driver.decorations(0..100);

        assert!(
            decorations.is_empty(),
            "No rules match 'ident' captures, should be empty"
        );
    }

    #[test]
    fn test_decorations_respects_byte_range() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let deco_query = Arc::new(Query::new(&language, "(identifier) @ident").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "ident".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("*".into()),
            },
            category: HighlightCategory::new("test"),
        }];

        let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        // "fn main() {}\nfn other() {}" — two functions, identifiers at different positions
        driver.parse("fn main() {}\nfn other() {}");

        // Query only the first line (bytes 0..13)
        let decorations = driver.decorations(0..13);
        // "main" is at bytes 3..7
        assert!(
            !decorations.is_empty(),
            "Should find identifier in first line"
        );
        for d in &decorations {
            assert!(
                d.start_byte < 13,
                "Decoration at byte {} should be in first line",
                d.start_byte
            );
        }
    }

    #[test]
    fn test_decorations_alongside_highlights() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let deco_query = Arc::new(Query::new(&language, "(function_item) @fn_block").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "fn_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("test.bg"),
        }];

        let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        driver.parse("fn main() {}");

        // Both should work independently
        let highlights = driver.highlights(0..100);
        let decorations = driver.decorations(0..100);

        assert!(!highlights.is_empty(), "Should have highlights");
        assert!(!decorations.is_empty(), "Should have decorations");

        // Highlights should all be Highlight kind
        for h in &highlights {
            assert_eq!(h.kind, AnnotationKind::Highlight);
        }
        // Decorations should all be Background kind (as configured)
        for d in &decorations {
            assert_eq!(d.kind, AnnotationKind::Background);
        }
    }

    #[test]
    fn test_decorations_conceal_with_replacement() {
        use reovim_driver_syntax::AnnotationKind;

        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        let deco_query =
            Arc::new(Query::new(&language, "(let_declaration \"let\" @kw)").unwrap());

        let rules = vec![DecorationRule {
            capture_name: "kw".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("LET".into()),
            },
            category: HighlightCategory::new("keyword.conceal"),
        }];

        let mut driver = TreeSitterDriver::builder("rust", &language, highlight_query)
            .decoration(deco_query, rules)
            .build()
            .unwrap();

        driver.parse("let x = 1;");
        let decorations = driver.decorations(0..100);

        assert_eq!(decorations.len(), 1);
        assert!(matches!(
            &decorations[0].kind,
            AnnotationKind::Conceal { replacement: Some(r) } if r == "LET"
        ));
        assert_eq!(decorations[0].start_byte, 0);
        assert_eq!(decorations[0].end_byte, 3); // "let" is 3 bytes
    }
}
