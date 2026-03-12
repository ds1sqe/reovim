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

use crate::{InjectionManager, provider::DecorationProvider};

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

    /// Imperative decoration providers (table rendering, list bullets, etc.)
    decoration_providers: Vec<Box<dyn DecorationProvider>>,

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
            decoration_providers: Vec::new(),
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
            decoration_providers: Vec::new(),
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

                    parent_highlights.push(Annotation::new(
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

                let mut matches = cursor.matches(deco_query, tree.root_node(), content.as_bytes());
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
                cursor.set_byte_range(byte_range.clone());

                let capture_names = inline_query.capture_names();
                let mut captures = Vec::new();

                let mut matches =
                    cursor.matches(inline_query, inline_tree.root_node(), content.as_bytes());
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

        // 3. Imperative providers (tables, list bullets, etc.)
        if !self.decoration_providers.is_empty()
            && let Some(provider_decos) = self.with_tree(|tree, content| {
                self.decoration_providers
                    .iter()
                    .flat_map(|p| p.decorations(tree, content, byte_range.clone()))
                    .collect::<Vec<_>>()
            })
        {
            result.extend(provider_decos);
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
    decoration_providers: Vec<Box<dyn DecorationProvider>>,
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
            decoration_providers: Vec::new(),
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Add an imperative decoration provider.
    ///
    /// Providers run after declarative rules during `decorations()`.
    /// Multiple providers can be added and will be called in order.
    #[must_use]
    pub fn decoration_provider(mut self, provider: Box<dyn DecorationProvider>) -> Self {
        self.decoration_providers.push(provider);
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
            decoration_providers: self.decoration_providers,
            query_cursor: Mutex::new(QueryCursor::new()),
            version: AtomicU64::new(0),
            parse_error: RwLock::new(None),
        })
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
