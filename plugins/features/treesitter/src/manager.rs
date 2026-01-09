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
    /// Source content cache for context providers
    source_cache: HashMap<usize, String>,
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
            source_cache: HashMap::new(),
        }
    }

    /// Register a language with the manager
    ///
    /// Queries are compiled lazily on first access, not at registration time.
    /// This reduces startup latency significantly (~72ms → <1ms for 8 languages).
    pub fn register_language(&mut self, language: Arc<dyn crate::registry::LanguageSupport>) {
        let lang_id = language.language_id().to_string();
        self.registry.register(language);
        tracing::debug!(language_id = %lang_id, "Registered language (lazy compilation)");
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
        // Cache the source content for context providers
        self.source_cache.insert(buffer_id, content.to_string());

        // Extract language_id and parse tree in a block to release parser borrow
        let (language_id, tree) = {
            let Some(parser) = self.parsers.get_mut(&buffer_id) else {
                return Vec::new();
            };
            let language_id = parser.language_id().to_string();
            let Some(tree) = parser.parse_full(content) else {
                return Vec::new();
            };
            (language_id, tree.clone())
        };

        // Get or compile the query (lazy compilation)
        let Some(query) = self.get_or_compile_query(&language_id, QueryType::Highlights) else {
            return Vec::new();
        };

        // Generate highlights
        self.highlighter
            .highlight_range(&tree, &query, content, start_line, end_line)
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
        self.source_cache.remove(&buffer_id);
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

    /// Get the cached source content for a buffer
    ///
    /// Returns the source text that was last parsed for this buffer.
    /// Used by context providers to analyze buffer content.
    #[must_use]
    pub fn get_source(&self, buffer_id: usize) -> Option<&str> {
        self.source_cache.get(&buffer_id).map(String::as_str)
    }

    /// Get or compile a query for a language (lazy compilation)
    ///
    /// This is the primary method for accessing queries. It checks the cache first,
    /// and compiles the query on first access if not cached.
    fn get_or_compile_query(&self, language_id: &str, query_type: QueryType) -> Option<Arc<Query>> {
        let registered = self.registry.get(language_id)?;

        let source = match query_type {
            QueryType::Highlights => Some(registered.highlights_query()),
            QueryType::Folds => registered.folds_query(),
            QueryType::TextObjects => registered.textobjects_query(),
            QueryType::Decorations => registered.decorations_query(),
            QueryType::Injections => registered.injections_query(),
        }?;

        self.queries
            .get_or_compile(language_id, query_type, registered.language(), source)
    }

    /// Get or compile a query for a language (lazy compilation on first access)
    ///
    /// Public API for accessing queries with lazy compilation.
    #[must_use]
    pub fn get_cached_query(&self, language_id: &str, query_type: QueryType) -> Option<Arc<Query>> {
        self.get_or_compile_query(language_id, query_type)
    }

    /// Pre-compile all queries for all registered languages
    ///
    /// This is called from a background thread during startup to warm the cache.
    /// Returns the number of queries successfully compiled.
    pub fn precompile_all_queries(&self) -> usize {
        let language_ids = self.registry.language_ids();
        let mut count = 0;

        for lang_id in &language_ids {
            // Compile all query types for each language
            for query_type in [
                QueryType::Highlights,
                QueryType::Folds,
                QueryType::TextObjects,
                QueryType::Decorations,
                QueryType::Injections,
            ] {
                if self.get_or_compile_query(lang_id, query_type).is_some() {
                    count += 1;
                }
            }
        }

        tracing::debug!(
            languages = language_ids.len(),
            queries = count,
            "Background query precompilation complete"
        );
        count
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

        // Extract language_id and parse tree in a block to release parser borrow
        let (language_id, tree) = {
            let Some(parser) = self.parsers.get_mut(&buffer_id) else {
                return Vec::new();
            };
            let language_id = parser.language_id().to_string();
            let Some(tree) = parser.parse_full(content) else {
                return Vec::new();
            };
            (language_id, tree.clone())
        };

        // Get or compile the query (lazy compilation)
        let Some(query) = self.get_or_compile_query(&language_id, QueryType::Folds) else {
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
        // Extract language_id and parse tree in a block to release parser borrow
        let (language_id, tree) = {
            let Some(parser) = self.parsers.get_mut(&buffer_id) else {
                return Vec::new();
            };
            let language_id = parser.language_id().to_string();
            let input_edit = edit.to_input_edit();
            let Some(tree) = parser.parse_incremental(content, &input_edit) else {
                return Vec::new();
            };
            (language_id, tree.clone())
        };

        // Get or compile the query (lazy compilation)
        let Some(query) = self.get_or_compile_query(&language_id, QueryType::Highlights) else {
            return Vec::new();
        };

        // Generate highlights
        self.highlighter
            .highlight_range(&tree, &query, content, start_line, end_line)
    }
}
