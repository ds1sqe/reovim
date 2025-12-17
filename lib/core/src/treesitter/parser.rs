//! Buffer parser state management and incremental parsing

use tree_sitter::{InputEdit, Parser, Tree};

use super::grammar::LanguageId;

/// Parser state for a single buffer
pub struct BufferParser {
    /// The tree-sitter parser instance
    parser: Parser,
    /// Current parse tree (None if never parsed or parse failed)
    tree: Option<Tree>,
    /// Language of this parser
    language_id: LanguageId,
    /// Version counter for cache invalidation
    version: u64,
}

impl BufferParser {
    /// Create a new parser for the given language
    ///
    /// Returns None if the language is unknown or parser setup fails
    #[must_use]
    pub fn new(language: &tree_sitter::Language, language_id: LanguageId) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(language).ok()?;

        Some(Self {
            parser,
            tree: None,
            language_id,
            version: 0,
        })
    }

    /// Get the language ID
    #[must_use]
    pub const fn language_id(&self) -> LanguageId {
        self.language_id
    }

    /// Get the current version
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }

    /// Get the current parse tree
    #[must_use]
    pub const fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// Perform a full parse of the buffer content
    ///
    /// This replaces any existing parse tree
    pub fn parse_full(&mut self, content: &str) -> Option<&Tree> {
        self.tree = self.parser.parse(content, None);
        self.version = self.version.wrapping_add(1);
        self.tree.as_ref()
    }

    /// Perform an incremental parse after an edit
    ///
    /// The `edit` describes what changed in the document.
    /// Returns the new parse tree, or None if parsing failed.
    pub fn parse_incremental(&mut self, content: &str, edit: &InputEdit) -> Option<&Tree> {
        // Apply the edit to the existing tree if we have one
        if let Some(ref mut tree) = self.tree {
            tree.edit(edit);
        }

        // Re-parse with the old tree for incremental parsing
        self.tree = self.parser.parse(content, self.tree.as_ref());
        self.version = self.version.wrapping_add(1);
        self.tree.as_ref()
    }

    /// Check if we have a valid parse tree
    #[must_use]
    pub const fn has_tree(&self) -> bool {
        self.tree.is_some()
    }
}
