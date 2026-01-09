//! Tree-sitter based syntax provider implementation
//!
//! This module provides the `TreeSitterSyntax` type which implements the core
//! `SyntaxProvider` trait using tree-sitter for parsing and highlighting.

use std::sync::{Arc, RwLock};

use {
    reovim_core::{
        highlight::Highlight,
        syntax::{EditInfo, SyntaxProvider},
    },
    tree_sitter::{InputEdit, Parser, Point, Query, Tree},
};

use crate::{
    highlighter::Highlighter, injection::InjectionManager, state::SharedTreesitterManager,
};

/// Tree-sitter based syntax provider
///
/// Owns a parser, cached parse tree, and shared query for a specific language.
/// Implements `SyntaxProvider` to integrate with the core buffer system.
///
/// Supports language injections (e.g., code blocks in markdown) via the
/// injection manager.
pub struct TreeSitterSyntax {
    /// Buffer ID this syntax provider is for (used for syncing tree to manager)
    buffer_id: usize,
    /// Language identifier (e.g., "rust", "python")
    language_id: String,
    /// Tree-sitter parser instance
    parser: Parser,
    /// Current parse tree (None if never parsed or parse failed)
    tree: Option<Tree>,
    /// Pre-compiled highlights query (shared via Arc)
    query: Arc<Query>,
    /// Highlighter with theme support
    highlighter: Highlighter,
    /// Injection manager for embedded languages (uses RwLock for thread-safe interior mutability)
    injection_manager: RwLock<InjectionManager>,
    /// Shared manager for injection lookups (languages, queries) and tree syncing
    manager: Option<Arc<SharedTreesitterManager>>,
}

impl TreeSitterSyntax {
    /// Create a new tree-sitter syntax provider
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer ID this syntax is for
    /// * `language` - Tree-sitter language grammar
    /// * `language_id` - Language identifier string
    /// * `query` - Pre-compiled highlights query (Arc for cheap sharing)
    ///
    /// # Returns
    /// Some(syntax) if parser setup succeeds, None otherwise
    pub fn new(
        buffer_id: usize,
        language: &tree_sitter::Language,
        language_id: &str,
        query: Arc<Query>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;

        Some(Self {
            buffer_id,
            language_id: language_id.to_string(),
            parser,
            tree: None,
            query,
            highlighter: Highlighter::new(),
            injection_manager: RwLock::new(InjectionManager::new(None)),
            manager: None,
        })
    }

    /// Create a new tree-sitter syntax provider with injection support
    ///
    /// # Arguments
    /// * `buffer_id` - Buffer ID this syntax is for
    /// * `language` - Tree-sitter language grammar
    /// * `language_id` - Language identifier string
    /// * `query` - Pre-compiled highlights query
    /// * `injection_query` - Optional pre-compiled injection query
    /// * `manager` - Shared manager for injection lookups and tree syncing
    ///
    /// # Returns
    /// Some(syntax) if parser setup succeeds, None otherwise
    pub fn with_injections(
        buffer_id: usize,
        language: &tree_sitter::Language,
        language_id: &str,
        query: Arc<Query>,
        injection_query: Option<Arc<Query>>,
        manager: Arc<SharedTreesitterManager>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;

        Some(Self {
            buffer_id,
            language_id: language_id.to_string(),
            parser,
            tree: None,
            query,
            highlighter: Highlighter::new(),
            injection_manager: RwLock::new(InjectionManager::new(injection_query)),
            manager: Some(manager),
        })
    }

    /// Get the current parse tree
    #[must_use]
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// Check if this syntax provider supports injections
    #[must_use]
    pub fn supports_injections(&self) -> bool {
        self.injection_manager.read().unwrap().has_detector()
    }
}

impl SyntaxProvider for TreeSitterSyntax {
    fn language_id(&self) -> &str {
        &self.language_id
    }

    fn highlight_range(&self, content: &str, start_line: u32, end_line: u32) -> Vec<Highlight> {
        let Some(tree) = &self.tree else {
            tracing::debug!(language_id = %self.language_id, "highlight_range: no tree");
            return Vec::new();
        };

        tracing::debug!(
            language_id = %self.language_id,
            has_manager = self.manager.is_some(),
            supports_injections = self.supports_injections(),
            "highlight_range: starting"
        );

        // Get parent language highlights
        let mut highlights =
            self.highlighter
                .highlight_range(tree, &self.query, content, start_line, end_line);

        // Add injection highlights if supported
        if let Some(manager) = &self.manager {
            let mut injection_manager = self.injection_manager.write().unwrap();
            if injection_manager.has_detector() {
                let injection_highlights = injection_manager.highlight_injections(
                    tree,
                    content,
                    start_line,
                    end_line,
                    manager,
                    &self.highlighter,
                );

                // Merge highlights - injection highlights take priority
                // by sorting and letting later (injection) highlights override earlier
                highlights.extend(injection_highlights);

                // Sort by position so injection highlights can properly override
                highlights.sort_by_key(|h| (h.span.start_line, h.span.start_col, h.span.end_col));
            }
        }

        highlights
    }

    fn parse(&mut self, content: &str) {
        self.tree = self.parser.parse(content, None);

        // Sync tree to manager for context queries
        if let (Some(tree), Some(manager)) = (&self.tree, &self.manager) {
            manager.with_mut(|m| {
                m.set_tree(self.buffer_id, &self.language_id, tree.clone());
                m.set_source(self.buffer_id, content.to_string());
            });
        }

        // Invalidate injection regions since the tree changed
        self.injection_manager.write().unwrap().invalidate();
    }

    fn parse_incremental(&mut self, content: &str, edit: &EditInfo) {
        // Apply the edit to the existing tree if we have one
        if let Some(ref mut tree) = self.tree {
            tree.edit(&InputEdit {
                start_byte: edit.start_byte,
                old_end_byte: edit.old_end_byte,
                new_end_byte: edit.new_end_byte,
                start_position: Point::new(edit.start_row as usize, edit.start_col as usize),
                old_end_position: Point::new(edit.old_end_row as usize, edit.old_end_col as usize),
                new_end_position: Point::new(edit.new_end_row as usize, edit.new_end_col as usize),
            });
        }

        // Re-parse with the old tree for incremental parsing
        self.tree = self.parser.parse(content, self.tree.as_ref());

        // Sync tree to manager for context queries
        if let (Some(tree), Some(manager)) = (&self.tree, &self.manager) {
            manager.with_mut(|m| {
                m.set_tree(self.buffer_id, &self.language_id, tree.clone());
                m.set_source(self.buffer_id, content.to_string());
            });
        }

        // Invalidate injection regions since the tree changed
        self.injection_manager.write().unwrap().invalidate();
    }

    fn is_parsed(&self) -> bool {
        self.tree.is_some()
    }

    fn saturate_injections(&mut self, content: &str) {
        if let (Some(tree), Some(manager)) = (&self.tree, &self.manager) {
            self.injection_manager
                .write()
                .unwrap()
                .saturate(tree, content, manager);
        }
    }
}
