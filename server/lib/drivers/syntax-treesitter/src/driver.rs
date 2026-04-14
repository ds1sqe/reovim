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
    reovim_driver_text_syntax::{
        Annotation, ContextHierarchy, DecorationCapture, DecorationRule, FoldKind, FoldRange,
        HighlightCategory, Injection, ScopeKind, ScopeRange, SyntaxContext, SyntaxDriver,
        SyntaxEdit, TextObjectKind, TextObjectRange, TextObjectScope, decoration::apply_rules,
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

    /// Optional context query for scope boundaries
    context_query: Option<Arc<Query>>,

    /// Optional textobjects query for semantic text object resolution
    textobjects_query: Option<Arc<Query>>,

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

    /// Default injection language for bare code blocks (no language tag).
    ///
    /// Set by the injection pipeline to the parent driver's language ID.
    /// Used as fallback in `injections()` when `@injection.language` is absent.
    default_injection_language: Option<String>,
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
            context_query: None,
            textobjects_query: None,
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
            default_injection_language: None,
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
            context_query: None,
            textobjects_query: None,
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
            default_injection_language: None,
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

    /// Check if this driver supports scope context queries.
    #[must_use]
    pub const fn supports_context(&self) -> bool {
        self.context_query.is_some()
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

    /// Get a reference to the injection manager.
    ///
    /// Returns `None` if the driver was not created with an injections query.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn injection_manager(&self) -> Option<&Mutex<InjectionManager>> {
        self.injection_manager.as_ref()
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
                let injection_highlights = manager.highlight_injections(
                    &injections,
                    &content,
                    byte_range.clone(),
                    &self.language_id,
                );

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

    #[allow(clippy::cast_possible_truncation)]
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
            if *name == "injection.content" {
                content_idx = Some(i as u32);
            } else if *name == "injection.language" {
                language_idx = Some(i as u32);
            }
        }

        let Some(content_idx) = content_idx else {
            return Vec::new(); // No @injection.content capture
        };

        // Detect combined patterns (patterns with #set! injection.combined)
        let mut combined_patterns = std::collections::HashSet::new();
        for pattern_idx in 0..injections_query.pattern_count() {
            for property in injections_query.property_settings(pattern_idx) {
                if &*property.key == "injection.combined" {
                    combined_patterns.insert(pattern_idx);
                }
            }
        }

        // Accumulate combined injections by language
        #[allow(clippy::type_complexity)]
        let mut combined_injections: std::collections::HashMap<
            String,
            Vec<(std::ops::Range<usize>, u32, u32, u32, u32)>,
        > = std::collections::HashMap::new();

        // Track seen content ranges for dedup (bare fence patterns may match
        // annotated fences too — skip if already captured with explicit language).
        let mut seen_ranges: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::new();

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

            // Fallback: use default injection language (parent's language ID)
            // for bare code blocks with no explicit language tag.
            if injection_language.is_none() {
                injection_language.clone_from(&self.default_injection_language);
            }

            // Create injection if we have both content and language
            if let (Some(content_node), Some(language_id)) = (injection_content, injection_language)
            {
                // Dedup: skip if this content range was already captured by
                // an earlier (more specific) pattern match.
                let range_key = (content_node.start_byte(), content_node.end_byte());
                if !seen_ranges.insert(range_key) {
                    continue;
                }

                let start_point = content_node.start_position();
                let end_point = content_node.end_position();
                let byte_range = content_node.start_byte()..content_node.end_byte();

                if combined_patterns.contains(&match_.pattern_index) {
                    // Accumulate into combined map
                    combined_injections.entry(language_id).or_default().push((
                        byte_range,
                        start_point.row as u32,
                        start_point.column as u32,
                        end_point.row as u32,
                        end_point.column as u32,
                    ));
                } else {
                    // Normal (non-combined) injection -- emit immediately
                    injections.push(Injection::new(
                        language_id,
                        byte_range,
                        start_point.row as u32,
                        start_point.column as u32,
                        end_point.row as u32,
                        end_point.column as u32,
                    ));
                }
            }
        }

        // Flush combined injections as single entries with multiple ranges
        for (language_id, entries) in combined_injections {
            if entries.is_empty() {
                continue;
            }
            let ranges: Vec<std::ops::Range<usize>> =
                entries.iter().map(|(r, ..)| r.clone()).collect();
            let (_, sr, sc, ..) = &entries[0];
            let (.., er, ec) = entries.last().expect("entries is not empty");

            injections.push(Injection::combined(language_id, ranges, *sr, *sc, *er, *ec));
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

        // 4. Injection decorations (child driver decorations, e.g., markdown in doc comments)
        if let Some(ref manager_mutex) = self.injection_manager {
            let injections = self.injections();

            if !injections.is_empty() {
                let content = self.content.read();
                let mut manager = manager_mutex.lock();
                let injection_decorations = manager.decorate_injections(
                    &injections,
                    &content,
                    byte_range,
                    &self.language_id,
                );
                result.extend(injection_decorations);
            }
        }

        result
    }

    fn set_injection_factory(
        &mut self,
        factory: Arc<dyn reovim_driver_text_syntax::SyntaxDriverFactory>,
    ) {
        if let Some(ref manager_mutex) = self.injection_manager {
            let mut manager = manager_mutex.lock();
            manager.set_factory(factory);
        }
    }

    fn set_injection_depth(&mut self, depth: u8) {
        if let Some(ref manager_mutex) = self.injection_manager {
            let mut manager = manager_mutex.lock();
            manager.set_depth(depth);
        }
    }

    fn set_default_injection_language(&mut self, language: &str) {
        self.default_injection_language = Some(language.to_owned());
    }

    #[allow(clippy::cast_possible_truncation)]
    fn scopes(&self, line: u32, col: u32) -> ContextHierarchy {
        let Some(context_query) = &self.context_query else {
            return ContextHierarchy::empty();
        };

        let Some(scopes) = self.with_tree(|tree, content| {
            let mut cursor = QueryCursor::new();

            let capture_names = context_query.capture_names();
            let context_idx = capture_names.iter().position(|n| *n == "context");
            let name_idx = capture_names.iter().position(|n| *n == "name");

            let Some(context_idx) = context_idx else {
                return Vec::new();
            };

            let mut scopes = Vec::new();
            let cursor_point = Point::new(line as usize, col as usize);

            let mut matches = cursor.matches(context_query, tree.root_node(), content.as_bytes());
            while let Some(match_) = matches.next() {
                let mut context_node = None;
                let mut name_text = None;

                for capture in match_.captures {
                    if capture.index == context_idx as u32 {
                        context_node = Some(capture.node);
                    } else if Some(capture.index as usize) == name_idx {
                        let node = capture.node;
                        let text = &content[node.start_byte()..node.end_byte()];
                        name_text = Some(text.to_string());
                    }
                }

                let Some(node) = context_node else {
                    continue;
                };

                // Filter: only include nodes that contain the cursor position
                let start = node.start_position();
                let end = node.end_position();
                if cursor_point < start || cursor_point > end {
                    continue;
                }

                let kind = node_to_scope_kind(node.kind());
                let display = build_scope_display_text(node, name_text.as_deref(), content);
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;

                scopes.push(ScopeRange::new(start_line, end_line, kind, display, name_text));
            }

            // Sort: outermost first (start_line asc, then end_line desc for same start)
            scopes.sort_by(|a, b| {
                a.start_line
                    .cmp(&b.start_line)
                    .then(b.end_line.cmp(&a.end_line))
            });

            // Dedup: remove duplicates at same start_line
            scopes.dedup_by_key(|s| s.start_line);

            scopes
        }) else {
            return ContextHierarchy::empty();
        };

        ContextHierarchy::new(0, line, col, scopes)
    }

    fn context_at_byte(&self, byte_offset: usize) -> SyntaxContext {
        // Use highlights to determine context — parser-agnostic approach.
        // Check if any highlight annotation at this byte has a category
        // starting with "string" or "comment".
        let range = byte_offset..byte_offset.saturating_add(1);
        let highlights = self.highlights(range);

        for ann in &highlights {
            if ann.contains(byte_offset) {
                let cat = ann.category.as_str();
                if cat.starts_with("string") || cat == "character" {
                    return SyntaxContext::String;
                }
                if cat.starts_with("comment") {
                    return SyntaxContext::Comment;
                }
            }
        }

        SyntaxContext::Code
    }

    #[allow(clippy::cast_possible_truncation)]
    fn textobject_range(
        &self,
        kind: TextObjectKind,
        scope: TextObjectScope,
        line: u32,
        col: u32,
    ) -> Option<TextObjectRange> {
        let textobjects_query = self.textobjects_query.as_ref()?;

        self.with_tree(|tree, content| {
            let mut cursor = QueryCursor::new();
            let capture_names = textobjects_query.capture_names();

            // Build target capture name: "function.inner", "class.outer", etc.
            let target = format!("{}.{}", kind.capture_name(), scope.suffix());

            // Find capture index for the target name
            let target_idx = capture_names.iter().position(|n| *n == target)?;

            let cursor_point = Point::new(line as usize, col as usize);
            let mut best: Option<(usize, TextObjectRange)> = None;

            let mut matches =
                cursor.matches(textobjects_query, tree.root_node(), content.as_bytes());
            while let Some(match_) = matches.next() {
                for capture in match_.captures {
                    if capture.index as usize != target_idx {
                        continue;
                    }

                    let node = capture.node;
                    let start = node.start_position();
                    let end = node.end_position();

                    // Filter: node must contain cursor position
                    if cursor_point < start || cursor_point > end {
                        continue;
                    }

                    // Pick smallest (most specific) node
                    let size = node.end_byte() - node.start_byte();
                    if best.as_ref().is_none_or(|(best_size, _)| size < *best_size) {
                        best = Some((
                            size,
                            TextObjectRange::new(
                                node.start_byte(),
                                node.end_byte(),
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32,
                            ),
                        ));
                    }
                }
            }

            best.map(|(_, range)| range)
        })
        .flatten()
    }

    fn is_parsed(&self) -> bool {
        self.tree.read().is_some()
    }
}

// ============================================================================
// Scope Helpers
// ============================================================================

/// Map a tree-sitter node kind to a `ScopeKind`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn node_to_scope_kind(node_kind: &str) -> ScopeKind {
    match node_kind {
        "function_item"
        | "function_definition"
        | "closure_expression"
        | "method_definition"
        | "arrow_function"
        | "method_declaration"
        | "function_declaration" => ScopeKind::Function,

        "struct_item"
        | "enum_item"
        | "impl_item"
        | "trait_item"
        | "union_item"
        | "class_definition"
        | "class_declaration"
        | "class_specifier"
        | "interface_declaration" => ScopeKind::Class,

        "mod_item" | "module" => ScopeKind::Module,

        "atx_heading" | "setext_heading" => ScopeKind::Heading,

        "namespace_definition" => ScopeKind::Namespace,

        _ => ScopeKind::Block,
    }
}

/// Simplify a tree-sitter node kind for display.
fn simplify_kind(kind: &str) -> &str {
    match kind {
        "function_item"
        | "function_definition"
        | "function_declaration"
        | "method_definition"
        | "method_declaration"
        | "arrow_function" => "fn",
        "closure_expression" => "closure",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "impl_item" => "impl",
        "trait_item" => "trait",
        "union_item" => "union",
        "class_definition" | "class_declaration" | "class_specifier" => "class",
        "interface_declaration" => "interface",
        "mod_item" | "module" => "mod",
        "namespace_definition" => "namespace",
        "atx_heading" | "setext_heading" => "heading",
        other => other,
    }
}

/// Build display text for a scope node.
///
/// Produces human-readable text like "fn main", "impl Foo", "struct Bar".
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_scope_display_text(node: Node, name_text: Option<&str>, content: &str) -> String {
    let simplified = simplify_kind(node.kind());

    if let Some(name) = name_text {
        return format!("{simplified} {name}");
    }

    // For impl items, try to find the type identifier child
    if node.kind() == "impl_item" {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i)
                && (child.kind() == "type_identifier" || child.kind() == "generic_type")
            {
                let text = &content[child.start_byte()..child.end_byte()];
                return format!("impl {text}");
            }
        }
    }

    // Fallback: first line of node text, truncated
    let start = node.start_byte();
    let end = node.end_byte().min(content.len());
    if start < end {
        let text = &content[start..end];
        let first_line = text.lines().next().unwrap_or("").trim();
        let truncated: String = first_line.chars().take(60).collect();
        if truncated.len() < first_line.len() {
            format!("{truncated}...")
        } else {
            truncated
        }
    } else {
        simplified.to_string()
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
    context_query: Option<Arc<Query>>,
    textobjects_query: Option<Arc<Query>>,
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
            context_query: None,
            textobjects_query: None,
            decoration_query: None,
            decoration_rules: Vec::new(),
            inline_parser: None,
            inline_decoration_query: None,
            inline_decoration_rules: Vec::new(),
            decoration_providers: Vec::new(),
        }
    }

    /// Set the context query for scope boundaries.
    #[must_use]
    pub fn context_query(mut self, query: Arc<Query>) -> Self {
        self.context_query = Some(query);
        self
    }

    /// Set the textobjects query for semantic text object resolution.
    #[must_use]
    pub fn textobjects_query(mut self, query: Arc<Query>) -> Self {
        self.textobjects_query = Some(query);
        self
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
            context_query: self.context_query,
            textobjects_query: self.textobjects_query,
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
            default_injection_language: None,
        })
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
