//! Treesitter manager for buffer parsing and highlighting
//!
//! Manages parsers for all buffers and coordinates with the language registry.

use std::{collections::HashMap, sync::Arc, time::Instant};

use {
    reovim_core::{folding::FoldRange, highlight::Highlight},
    tree_sitter::{Query, Tree},
};

use crate::{
    highlighter::Highlighter,
    parser::BufferParser,
    queries::{QueryCache, QueryType},
    registry::{LanguageRegistry, RegisteredLanguage},
    theme::TreesitterTheme,
};

/// Manages treesitter state for all buffers
pub struct TreesitterManager {
    /// Per-buffer parser state
    parsers: HashMap<usize, BufferParser>,
    /// Language registry for language detection
    registry: LanguageRegistry,
    /// Query cache for compiled queries
    queries: QueryCache,
    /// Highlighter with theme
    highlighter: Highlighter,
    /// Pending reparse requests (`buffer_id` -> timestamp)
    pending_parses: HashMap<usize, Instant>,
}

impl Default for TreesitterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TreesitterManager {
    /// Debounce duration for re-parsing after edits (50ms)
    pub const DEBOUNCE_MS: u64 = 50;

    /// Create a new treesitter manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            registry: LanguageRegistry::new(),
            queries: QueryCache::new(),
            highlighter: Highlighter::new(),
            pending_parses: HashMap::new(),
        }
    }

    /// Register a language with the manager
    ///
    /// Pre-compiles the highlights query for immediate use.
    pub fn register_language(&mut self, language: Arc<dyn crate::registry::LanguageSupport>) {
        let lang_id = language.language_id().to_string();

        // Register the language
        self.registry.register(language.clone());

        // Pre-compile the highlights query
        if let Some(registered) = self.registry.get(&lang_id) {
            let source = registered.highlights_query();
            let _ = self.queries.compile_and_cache(
                &lang_id,
                QueryType::Highlights,
                registered.language(),
                source,
            );

            // Also pre-compile optional queries if available
            if let Some(source) = registered.folds_query() {
                let _ = self.queries.compile_and_cache(
                    &lang_id,
                    QueryType::Folds,
                    registered.language(),
                    source,
                );
            }
            if let Some(source) = registered.textobjects_query() {
                let _ = self.queries.compile_and_cache(
                    &lang_id,
                    QueryType::TextObjects,
                    registered.language(),
                    source,
                );
            }
            if let Some(source) = registered.decorations_query() {
                let _ = self.queries.compile_and_cache(
                    &lang_id,
                    QueryType::Decorations,
                    registered.language(),
                    source,
                );
            }
            // Pre-compile injection query if available
            if let Some(source) = registered.injections_query()
                && self
                    .queries
                    .compile_and_cache(
                        &lang_id,
                        QueryType::Injections,
                        registered.language(),
                        source,
                    )
                    .is_some()
            {
                tracing::debug!(language_id = %lang_id, "Compiled injection query");
            }
        }
    }

    /// Initialize treesitter for a buffer based on file path
    ///
    /// Detects language from file extension and sets up the parser.
    /// Returns the detected language ID, or None if no language matched.
    pub fn init_buffer(&mut self, buffer_id: usize, file_path: Option<&str>) -> Option<String> {
        let language_id = file_path.and_then(|p| self.registry.detect_language(p))?;

        // Get the registered language
        let registered = self.registry.get(&language_id)?;

        // Create parser for this buffer
        if let Some(parser) = BufferParser::new(registered.language(), &language_id) {
            self.parsers.insert(buffer_id, parser);
            tracing::debug!(buffer_id, language_id = %language_id, "Initialized buffer parser");
            Some(language_id)
        } else {
            tracing::warn!(buffer_id, language_id = %language_id, "Failed to create parser");
            None
        }
    }

    /// Parse buffer content and return highlights
    ///
    /// Performs a full parse and generates highlights for the visible range.
    pub fn parse_and_highlight(
        &mut self,
        buffer_id: usize,
        content: &str,
        start_line: u32,
        end_line: u32,
    ) -> Vec<Highlight> {
        // Get parser and parse content
        let Some(parser) = self.parsers.get_mut(&buffer_id) else {
            return Vec::new();
        };

        let language_id = parser.language_id().to_string();

        // Parse the content
        let Some(tree) = parser.parse_full(content) else {
            return Vec::new();
        };

        // Get the pre-compiled query (immutable borrow)
        let Some(query) = self.queries.get(&language_id, QueryType::Highlights) else {
            return Vec::new();
        };

        // Generate highlights
        self.highlighter
            .highlight_range(tree, &query, content, start_line, end_line)
    }

    /// Schedule a buffer for reparsing (with debounce)
    pub fn schedule_reparse(&mut self, buffer_id: usize) {
        self.pending_parses.insert(buffer_id, Instant::now());
    }

    /// Get buffer IDs that are ready for reparsing (debounce elapsed)
    #[must_use]
    pub fn get_ready_parses(&mut self) -> Vec<usize> {
        let now = Instant::now();
        let debounce = std::time::Duration::from_millis(Self::DEBOUNCE_MS);

        let ready: Vec<usize> = self
            .pending_parses
            .iter()
            .filter(|(_, time)| now.duration_since(**time) >= debounce)
            .map(|(id, _)| *id)
            .collect();

        for id in &ready {
            self.pending_parses.remove(id);
        }

        ready
    }

    /// Check if a buffer has treesitter enabled
    #[must_use]
    pub fn has_parser(&self, buffer_id: usize) -> bool {
        self.parsers.contains_key(&buffer_id)
    }

    /// Get the language ID for a buffer
    #[must_use]
    pub fn buffer_language(&self, buffer_id: usize) -> Option<String> {
        self.parsers
            .get(&buffer_id)
            .map(|p| p.language_id().to_string())
    }

    /// Remove parser state for a buffer
    pub fn remove_buffer(&mut self, buffer_id: usize) {
        self.parsers.remove(&buffer_id);
        self.pending_parses.remove(&buffer_id);
    }

    /// Check if there are any pending parses
    #[must_use]
    pub fn has_pending_parses(&self) -> bool {
        !self.pending_parses.is_empty()
    }

    /// Cancel a pending reparse for a buffer
    pub fn cancel_reparse(&mut self, buffer_id: usize) {
        self.pending_parses.remove(&buffer_id);
    }

    /// Set the treesitter syntax theme
    pub fn set_theme(&mut self, theme: TreesitterTheme) {
        self.highlighter.set_theme(theme);
    }

    /// Get the parsed tree for a buffer
    #[must_use]
    pub fn get_tree(&self, buffer_id: usize) -> Option<&Tree> {
        self.parsers.get(&buffer_id).and_then(|p| p.tree())
    }

    /// Get a cached query for a language
    ///
    /// Returns None if the query is not cached. Queries are pre-compiled
    /// when languages are registered.
    #[must_use]
    pub fn get_cached_query(&self, language_id: &str, query_type: QueryType) -> Option<Arc<Query>> {
        self.queries.get(language_id, query_type)
    }

    /// Get the language registry
    #[must_use]
    pub fn registry(&self) -> &LanguageRegistry {
        &self.registry
    }

    /// Get the query cache
    #[must_use]
    pub fn query_cache(&self) -> &QueryCache {
        &self.queries
    }

    /// Get a registered language
    #[must_use]
    pub fn get_language(&self, id: &str) -> Option<Arc<RegisteredLanguage>> {
        self.registry.get(id)
    }

    /// Compute fold ranges for a buffer
    ///
    /// Uses treesitter fold queries to identify foldable regions.
    pub fn compute_fold_ranges(&mut self, buffer_id: usize, content: &str) -> Vec<FoldRange> {
        use {reovim_core::folding::FoldKind, tree_sitter::StreamingIterator};

        // Get parser and parse content
        let Some(parser) = self.parsers.get_mut(&buffer_id) else {
            return Vec::new();
        };

        let language_id = parser.language_id().to_string();

        // Parse if needed
        let Some(tree) = parser.parse_full(content) else {
            return Vec::new();
        };

        // Get the pre-compiled query (immutable borrow)
        let Some(query) = self.queries.get(&language_id, QueryType::Folds) else {
            return Vec::new();
        };

        let mut ranges = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

        #[allow(clippy::cast_possible_truncation)]
        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let node = capture.node;
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;

                // Only include folds that span multiple lines
                if end_line <= start_line {
                    continue;
                }

                // Get preview text (first line of the folded region)
                let preview = content
                    .lines()
                    .nth(start_line as usize)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                ranges.push(FoldRange::new(start_line, end_line, FoldKind::Block, preview));
            }
        }

        // Sort by start line and remove duplicates
        ranges.sort_by_key(|r| (r.start_line, r.end_line));
        ranges.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);

        ranges
    }

    /// Perform incremental parse and return highlights
    pub fn parse_incremental_and_highlight(
        &mut self,
        buffer_id: usize,
        content: &str,
        edit: &crate::edit::BufferEdit,
        start_line: u32,
        end_line: u32,
    ) -> Vec<Highlight> {
        let Some(parser) = self.parsers.get_mut(&buffer_id) else {
            return Vec::new();
        };

        let language_id = parser.language_id().to_string();
        let input_edit = edit.to_input_edit();

        // Perform incremental parse
        let Some(tree) = parser.parse_incremental(content, &input_edit) else {
            return Vec::new();
        };

        // Get the pre-compiled query (immutable borrow)
        let Some(query) = self.queries.get(&language_id, QueryType::Highlights) else {
            return Vec::new();
        };

        // Generate highlights
        self.highlighter
            .highlight_range(tree, &query, content, start_line, end_line)
    }
}
