//! Treesitter integration for syntax highlighting and semantic features

mod edit;
mod grammar;
mod highlighter;
mod parser;
mod queries;
mod text_objects;
mod theme;

use std::collections::HashMap;
use std::time::Instant;

pub use edit::BufferEdit;
pub use grammar::{BundledGrammar, GrammarRegistry, LanguageId};
pub use highlighter::Highlighter;
pub use parser::BufferParser;
pub use queries::{QueryCache, QueryType};
pub use text_objects::{Position, TextObjectBounds, TextObjectResolver};
pub use theme::TreesitterTheme;

use crate::folding::{FoldKind, FoldRange};
use crate::highlight::Highlight;
use crate::textobject::{SemanticTextObject, TextObjectScope};

/// Manages treesitter state for all buffers
pub struct TreesitterManager {
    /// Per-buffer parser state
    parsers: HashMap<usize, BufferParser>,
    /// Grammar registry for language detection
    grammars: GrammarRegistry,
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
            grammars: GrammarRegistry::new(),
            queries: QueryCache::new(),
            highlighter: Highlighter::new(),
            pending_parses: HashMap::new(),
        }
    }

    /// Initialize treesitter for a buffer based on file path
    ///
    /// Detects language from file extension and sets up the parser.
    /// Returns the detected language ID.
    pub fn init_buffer(&mut self, buffer_id: usize, file_path: Option<&str>) -> LanguageId {
        let language_id = file_path
            .map_or(LanguageId::Unknown, |p| self.grammars.detect_language(p));

        if language_id == LanguageId::Unknown {
            return language_id;
        }

        // Get the grammar for this language
        let Some(ts_language) = self.grammars.get_language(language_id) else {
            return LanguageId::Unknown;
        };

        // Create parser for this buffer
        if let Some(parser) = BufferParser::new(&ts_language, language_id) {
            self.parsers.insert(buffer_id, parser);
        }

        language_id
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
        let Some(parser) = self.parsers.get_mut(&buffer_id) else {
            return Vec::new();
        };

        let language_id = parser.language_id();

        // Parse the content
        let Some(tree) = parser.parse_full(content) else {
            return Vec::new();
        };

        // Get the tree-sitter language for query compilation
        let Some(ts_language) = self.grammars.get_language(language_id) else {
            return Vec::new();
        };

        // Get or compile the highlight query
        let Some(query) = self.queries.get_or_compile(language_id, QueryType::Highlights, &ts_language) else {
            return Vec::new();
        };

        // Generate highlights
        self.highlighter.highlight_range(tree, query, content, start_line, end_line)
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
    pub fn buffer_language(&self, buffer_id: usize) -> Option<LanguageId> {
        self.parsers.get(&buffer_id).map(BufferParser::language_id)
    }

    /// Remove parser state for a buffer
    pub fn remove_buffer(&mut self, buffer_id: usize) {
        self.parsers.remove(&buffer_id);
        self.pending_parses.remove(&buffer_id);
    }

    /// Get the grammar registry
    #[must_use]
    pub const fn grammars(&self) -> &GrammarRegistry {
        &self.grammars
    }

    /// Perform incremental parse after an edit and return highlights
    ///
    /// This is more efficient than `parse_and_highlight` when only small edits occurred.
    pub fn parse_incremental_and_highlight(
        &mut self,
        buffer_id: usize,
        content: &str,
        edit: &BufferEdit,
        start_line: u32,
        end_line: u32,
    ) -> Vec<Highlight> {
        let Some(parser) = self.parsers.get_mut(&buffer_id) else {
            return Vec::new();
        };

        let language_id = parser.language_id();
        let input_edit = edit.to_input_edit();

        // Perform incremental parse
        let Some(tree) = parser.parse_incremental(content, &input_edit) else {
            return Vec::new();
        };

        // Get the tree-sitter language for query compilation
        let Some(ts_language) = self.grammars.get_language(language_id) else {
            return Vec::new();
        };

        // Get or compile the highlight query
        let Some(query) =
            self.queries
                .get_or_compile(language_id, QueryType::Highlights, &ts_language)
        else {
            return Vec::new();
        };

        // Generate highlights for the visible range
        self.highlighter
            .highlight_range(tree, query, content, start_line, end_line)
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

    /// Find bounds for a semantic text object at the cursor position
    ///
    /// Uses treesitter queries to locate language constructs like functions,
    /// classes, parameters, loops, etc.
    ///
    /// # Arguments
    /// * `buffer_id` - The buffer ID
    /// * `content` - The buffer content
    /// * `cursor_row` - Cursor row (0-indexed)
    /// * `cursor_col` - Cursor column (0-indexed)
    /// * `kind` - The semantic text object type (function, class, etc.)
    /// * `scope` - Inner or Around scope
    ///
    /// # Returns
    /// The bounds of the text object if found
    pub fn find_text_object_bounds(
        &mut self,
        buffer_id: usize,
        content: &str,
        cursor_row: u32,
        cursor_col: u32,
        kind: SemanticTextObject,
        scope: TextObjectScope,
    ) -> Option<TextObjectBounds> {
        let parser = self.parsers.get_mut(&buffer_id)?;
        let language_id = parser.language_id();

        // Parse if needed (should usually have a valid tree)
        let tree = parser.parse_full(content)?;

        // Get the tree-sitter language for query compilation
        let ts_language = self.grammars.get_language(language_id)?;

        // Get or compile the text objects query
        let query = self
            .queries
            .get_or_compile(language_id, QueryType::TextObjects, &ts_language)?;

        // Resolve the text object bounds
        TextObjectResolver::resolve(tree, query, content, cursor_row, cursor_col, kind, scope)
    }

    /// Compute fold ranges for a buffer
    ///
    /// Uses treesitter fold queries to identify foldable regions.
    ///
    /// # Arguments
    /// * `buffer_id` - The buffer ID
    /// * `content` - The buffer content
    ///
    /// # Returns
    /// Vector of fold ranges sorted by start line
    pub fn compute_fold_ranges(&mut self, buffer_id: usize, content: &str) -> Vec<FoldRange> {
        let Some(parser) = self.parsers.get_mut(&buffer_id) else {
            return Vec::new();
        };

        let language_id = parser.language_id();

        // Parse if needed
        let Some(tree) = parser.parse_full(content) else {
            return Vec::new();
        };

        // Get the tree-sitter language for query compilation
        let Some(ts_language) = self.grammars.get_language(language_id) else {
            return Vec::new();
        };

        // Get or compile the fold query
        let Some(query) = self
            .queries
            .get_or_compile(language_id, QueryType::Folds, &ts_language)
        else {
            return Vec::new();
        };

        Self::extract_fold_ranges(tree, query, content)
    }

    /// Extract fold ranges from a parsed tree using a fold query
    #[allow(clippy::cast_possible_truncation)]
    fn extract_fold_ranges(
        tree: &tree_sitter::Tree,
        query: &tree_sitter::Query,
        content: &str,
    ) -> Vec<FoldRange> {
        use tree_sitter::StreamingIterator;

        let mut ranges = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let node = capture.node;
                let start_line = node.start_position().row as u32;
                let end_line = node.end_position().row as u32;

                // Only include folds that span multiple lines
                if end_line <= start_line {
                    continue;
                }

                // Get the capture name to determine fold kind
                let capture_name = query.capture_names()[capture.index as usize];
                let kind = Self::capture_to_fold_kind(capture_name);

                // Get preview text (first line of the folded region)
                let preview = content
                    .lines()
                    .nth(start_line as usize)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                ranges.push(FoldRange::new(start_line, end_line, kind, preview));
            }
        }

        // Sort by start line and remove duplicates
        ranges.sort_by_key(|r| (r.start_line, r.end_line));
        ranges.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);

        ranges
    }

    /// Convert a capture name to a fold kind
    const fn capture_to_fold_kind(capture_name: &str) -> FoldKind {
        // Note: We can't use match on &str in const fn, so we check prefixes
        // Capture names are like "fold", "fold.function", "fold.class", etc.
        // For now, we'll use a simple default
        _ = capture_name; // Suppress unused warning
        FoldKind::Block
    }
}
