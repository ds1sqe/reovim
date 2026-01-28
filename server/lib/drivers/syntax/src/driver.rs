//! Syntax driver trait definition.
//!
//! This module defines the [`SyntaxDriver`] trait, the main interface for
//! syntax highlighting implementations. Implementations handle parsing and
//! highlighting for specific languages.

use std::ops::Range;

use crate::{edit::SyntaxEdit, fold::FoldRange, highlight::HighlightSpan, injection::Injection};

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

    /// Get highlights for a byte range.
    ///
    /// Returns all highlights that overlap with the given range.
    /// Results should be sorted by start position.
    ///
    /// # Arguments
    ///
    /// * `byte_range` - The byte range to get highlights for
    ///
    /// # Returns
    ///
    /// A vector of highlights overlapping the requested range.
    fn highlights(&self, byte_range: Range<usize>) -> Vec<HighlightSpan>;

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

    /// Check if the driver has valid parse state.
    ///
    /// Returns true if the driver has successfully parsed content.
    /// This should return false before [`parse()`](Self::parse) is called,
    /// and true after a successful parse.
    fn is_parsed(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use {super::*, crate::highlight::HighlightGroup};

    /// A minimal test implementation of `SyntaxDriver`.
    struct TestDriver {
        language: String,
        parsed: bool,
    }

    impl TestDriver {
        fn new(language: impl Into<String>) -> Self {
            Self {
                language: language.into(),
                parsed: false,
            }
        }
    }

    impl SyntaxDriver for TestDriver {
        fn language(&self) -> &str {
            &self.language
        }

        fn parse(&mut self, _content: &str) {
            self.parsed = true;
        }

        fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
            // No-op for test
        }

        fn highlights(&self, byte_range: Range<usize>) -> Vec<HighlightSpan> {
            if self.parsed {
                // Return a single highlight spanning the range
                vec![HighlightSpan::new(
                    byte_range.start,
                    byte_range.end,
                    HighlightGroup::Comment,
                )]
            } else {
                Vec::new()
            }
        }

        fn is_parsed(&self) -> bool {
            self.parsed
        }
    }

    #[test]
    fn test_syntax_driver_basic() {
        let mut driver = TestDriver::new("test");

        assert_eq!(driver.language(), "test");
        assert!(!driver.is_parsed());

        driver.parse("some content");
        assert!(driver.is_parsed());

        let highlights = driver.highlights(0..10);
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].start_byte, 0);
        assert_eq!(highlights[0].end_byte, 10);
    }

    #[test]
    fn test_syntax_driver_defaults() {
        let driver = TestDriver::new("test");

        // Default implementations return empty/None
        assert!(driver.injections().is_empty());
        assert!(driver.folds().is_empty());
        assert_eq!(driver.indent_for(0), None);
    }
}
