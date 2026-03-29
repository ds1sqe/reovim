//! Syntax driver trait definition.
//!
//! This module defines the [`SyntaxDriver`] trait, the main interface for
//! syntax highlighting implementations. Implementations handle parsing and
//! highlighting for specific languages.

use std::{ops::Range, sync::Arc};

use crate::{
    edit::SyntaxEdit,
    factory::SyntaxDriverFactory,
    fold::FoldRange,
    highlight::{Annotation, SyntaxContext},
    injection::Injection,
    scope::ContextHierarchy,
    textobject::{TextObjectKind, TextObjectRange, TextObjectScope},
};

/// Main parsing interface for syntax highlighting.
///
/// Implementors provide language-specific parsing and highlighting.
/// This trait is designed to be parser-agnostic - tree-sitter is just
/// one possible implementation.
///
/// # Lifecycle
///
/// 1. Create driver via [`SyntaxDriverFactory`](crate::SyntaxDriverFactory)
/// 2. Call [`parse()`](Self::parse) with initial content
/// 3. Call [`update()`](Self::update) for incremental edits
/// 4. Call [`highlights()`](Self::highlights) to get highlights for rendering
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
/// The driver may be shared between multiple views of the same buffer.
///
/// # Example
///
/// ```ignore
/// // Get driver from factory
/// let mut driver = factory.create("rust")?;
///
/// // Initial parse
/// driver.parse("fn main() {}");
/// assert!(driver.is_parsed());
///
/// // Get highlights for visible range
/// let highlights = driver.highlights(0..100);
///
/// // After edit, update incrementally
/// driver.update("fn main() { println!(); }", &edit);
/// ```
pub trait SyntaxDriver: Send + Sync {
    /// Get the language identifier.
    ///
    /// Returns a unique identifier like "rust", "python", "javascript".
    /// This should match the language ID used to create the driver.
    fn language(&self) -> &str;

    /// Perform a full parse of the content.
    ///
    /// Called on initial file open or when incremental update isn't possible.
    /// After calling this, [`is_parsed()`](Self::is_parsed) should return true.
    fn parse(&mut self, content: &str);

    /// Apply an incremental edit.
    ///
    /// For efficient re-parsing after buffer modifications.
    /// The edit describes what changed; the driver updates its internal state.
    ///
    /// # Arguments
    ///
    /// * `content` - The new full content after the edit
    /// * `edit` - Description of what changed
    fn update(&mut self, content: &str, edit: &SyntaxEdit);

    /// Get annotations for a byte range.
    ///
    /// Returns all annotations that overlap with the given range.
    /// Results should be sorted by start position.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range to get annotations for
    ///
    /// # Returns
    ///
    /// A vector of annotations overlapping the requested range.
    fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation>;

    /// Get language injection points.
    ///
    /// Returns regions where a different language should be highlighted.
    /// Used for embedded languages (markdown code blocks, HTML scripts, etc.).
    ///
    /// # Default
    ///
    /// Returns an empty vector. Override for languages that support injections.
    fn injections(&self) -> Vec<Injection> {
        Vec::new()
    }

    /// Get decoration annotations for a byte range.
    ///
    /// Returns annotations from decoration queries (e.g., markdown heading
    /// markers, list bullets, code blocks). These carry semantic categories
    /// like `markup.heading.1` that clients map to render behaviors.
    /// Use-sites call this separately from [`highlights()`](Self::highlights)
    /// to opt in to decoration rendering.
    ///
    /// # Default
    ///
    /// Returns an empty vector. Override for languages with decoration support.
    fn decorations(&self, _byte_range: Range<usize>) -> Vec<Annotation> {
        Vec::new()
    }

    /// Get foldable ranges.
    ///
    /// Returns all foldable regions in the document.
    ///
    /// # Default
    ///
    /// Returns an empty vector. Override to provide folding support.
    fn folds(&self) -> Vec<FoldRange> {
        Vec::new()
    }

    /// Get suggested indentation for a line.
    ///
    /// Returns the suggested indent level (in spaces) for a given line,
    /// or None if indentation cannot be determined.
    ///
    /// # Arguments
    ///
    /// * `line` - The 0-indexed line number
    ///
    /// # Default
    ///
    /// Returns None. Override to provide indentation hints.
    fn indent_for(&self, _line: usize) -> Option<usize> {
        None
    }

    /// Configure injection support with the given factory.
    ///
    /// Called by the runtime after driver creation to enable recursive
    /// injection highlighting. Implementations that support injections
    /// use this factory to create child drivers for embedded languages.
    ///
    /// # Default
    ///
    /// No-op. Override for drivers that support injection highlighting.
    fn set_injection_factory(&mut self, _factory: Arc<dyn SyntaxDriverFactory>) {}

    /// Set the injection nesting depth for this driver.
    ///
    /// Used to propagate depth through the injection hierarchy so that
    /// `MAX_INJECTION_DEPTH` is correctly enforced. Depth 0 = root driver,
    /// depth 1 = direct child, etc.
    ///
    /// # Default
    ///
    /// No-op. Override for drivers that support injection highlighting.
    fn set_injection_depth(&mut self, _depth: u8) {}

    /// Get the enclosing scope hierarchy at a position.
    ///
    /// Returns the scope boundaries (function, class, module, etc.)
    /// that contain the given line and column. Items are ordered
    /// outermost-first.
    ///
    /// # Arguments
    ///
    /// * `line` - 0-indexed line number
    /// * `col` - 0-indexed column number
    ///
    /// # Default
    ///
    /// Returns an empty hierarchy. Override for scope-aware behavior.
    fn scopes(&self, _line: u32, _col: u32) -> ContextHierarchy {
        ContextHierarchy::empty()
    }

    /// Get the syntax context at a byte position.
    ///
    /// Returns the syntactic context (code, string, comment) at the given
    /// byte offset. Used by features like auto-pair to skip insertion
    /// inside strings or comments.
    ///
    /// # Default
    ///
    /// Returns `SyntaxContext::Code`. Override for context-aware behavior.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn context_at_byte(&self, _byte_offset: usize) -> SyntaxContext {
        SyntaxContext::Code
    }

    /// Get the range of a semantic text object at a position.
    ///
    /// Resolves language-aware text objects like functions, classes, arguments,
    /// etc. at the given cursor position. Returns the byte and position range
    /// of the matching construct, or `None` if no match exists.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type of text object (function, class, etc.)
    /// * `scope` - Inner (body only) or outer (full construct)
    /// * `line` - 0-indexed cursor line
    /// * `col` - 0-indexed cursor column
    ///
    /// # Default
    ///
    /// Returns `None`. Override for drivers with treesitter query support.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn textobject_range(
        &self,
        _kind: TextObjectKind,
        _scope: TextObjectScope,
        _line: u32,
        _col: u32,
    ) -> Option<TextObjectRange> {
        None
    }

    /// Check if the driver has valid parse state.
    ///
    /// Returns true if the driver has successfully parsed content.
    /// This should return false before [`parse()`](Self::parse) is called,
    /// and true after a successful parse.
    fn is_parsed(&self) -> bool;
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
