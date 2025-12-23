//! Tree-sitter based syntax provider implementation
//!
//! This module provides the `TreeSitterSyntax` type which implements the core
//! `SyntaxProvider` trait using tree-sitter for parsing and highlighting.

use std::sync::Arc;

use reovim_core::{
    highlight::Highlight,
    syntax::{EditInfo, SyntaxProvider},
};
use tree_sitter::{InputEdit, Parser, Point, Query, Tree};

use crate::highlighter::Highlighter;

/// Tree-sitter based syntax provider
///
/// Owns a parser, cached parse tree, and shared query for a specific language.
/// Implements `SyntaxProvider` to integrate with the core buffer system.
pub struct TreeSitterSyntax {
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
}

impl TreeSitterSyntax {
    /// Create a new tree-sitter syntax provider
    ///
    /// # Arguments
    /// * `language` - Tree-sitter language grammar
    /// * `language_id` - Language identifier string
    /// * `query` - Pre-compiled highlights query (Arc for cheap sharing)
    ///
    /// # Returns
    /// Some(syntax) if parser setup succeeds, None otherwise
    pub fn new(
        language: &tree_sitter::Language,
        language_id: &str,
        query: Arc<Query>,
    ) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;

        Some(Self {
            language_id: language_id.to_string(),
            parser,
            tree: None,
            query,
            highlighter: Highlighter::new(),
        })
    }

    /// Get the current parse tree
    #[must_use]
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }
}

impl SyntaxProvider for TreeSitterSyntax {
    fn language_id(&self) -> &str {
        &self.language_id
    }

    fn highlight_range(&self, content: &str, start_line: u32, end_line: u32) -> Vec<Highlight> {
        let Some(tree) = &self.tree else {
            return Vec::new();
        };

        self.highlighter
            .highlight_range(tree, &self.query, content, start_line, end_line)
    }

    fn parse(&mut self, content: &str) {
        self.tree = self.parser.parse(content, None);
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
    }

    fn is_parsed(&self) -> bool {
        self.tree.is_some()
    }
}
