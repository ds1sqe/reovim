//! Decoration provider traits for extensible decoration sources.
//!
//! Plugins implement `DecorationProvider` to supply decorations for their
//! specific functionality (e.g., markdown concealment, search highlighting).

use super::types::{Decoration, DecorationGroup};

/// Trait for decoration sources.
///
/// Plugins implement this trait to provide decorations to the rendering system.
/// Each provider belongs to a priority group that determines layering order.
///
/// # Example
///
/// ```ignore
/// struct MarkdownConcealProvider {
///     conceals: Vec<Decoration>,
/// }
///
/// impl DecorationProvider for MarkdownConcealProvider {
///     fn name(&self) -> &str { "markdown-conceal" }
///     fn group(&self) -> DecorationGroup { DecorationGroup::Language }
///
///     fn decorations_for_range(&self, start: u32, end: u32) -> Vec<Decoration> {
///         self.conceals.iter()
///             .filter(|d| d.affects_line(start) || d.affects_line(end))
///             .cloned()
///             .collect()
///     }
///
///     fn refresh(&mut self, content: &str) {
///         self.conceals = parse_markdown_links(content);
///     }
/// }
/// ```
pub trait DecorationProvider: Send + Sync {
    /// Provider name for debugging and logging.
    fn name(&self) -> &'static str;

    /// Priority group for layering.
    ///
    /// Decorations from higher-priority groups override those from lower groups.
    fn group(&self) -> DecorationGroup;

    /// Get decorations for a line range.
    ///
    /// Returns all decorations that affect lines in `[start_line, end_line]` (inclusive).
    fn decorations_for_range(&self, start_line: u32, end_line: u32) -> Vec<Decoration>;

    /// Refresh decorations after content changes.
    ///
    /// Called by the saturator when buffer content changes.
    /// The provider should re-parse the content and update its internal state.
    fn refresh(&mut self, content: &str);

    /// Check if the provider has valid state.
    ///
    /// Returns `false` if the provider needs to be refreshed before use.
    fn is_valid(&self) -> bool {
        true
    }
}

/// Factory for creating decoration providers.
///
/// Registered factories are consulted when a buffer is opened to determine
/// which providers should be attached based on the file type.
pub trait DecorationProviderFactory: Send + Sync {
    /// Factory name for debugging.
    fn name(&self) -> &str;

    /// Create a provider for the given language ID.
    ///
    /// Returns `None` if this factory doesn't handle the language.
    fn create(&self, language_id: &str) -> Option<Box<dyn DecorationProvider>>;

    /// List of language IDs this factory supports.
    fn supported_languages(&self) -> &[&str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider {
        decorations: Vec<Decoration>,
        valid: bool,
    }

    impl DecorationProvider for TestProvider {
        fn name(&self) -> &'static str {
            "test"
        }

        fn group(&self) -> DecorationGroup {
            DecorationGroup::Language
        }

        fn decorations_for_range(&self, start_line: u32, end_line: u32) -> Vec<Decoration> {
            self.decorations
                .iter()
                .filter(|d| {
                    let start = d.start_line();
                    let end = d.end_line();
                    // Check if decoration overlaps with requested range
                    start <= end_line && end >= start_line
                })
                .cloned()
                .collect()
        }

        fn refresh(&mut self, _content: &str) {
            self.valid = true;
        }

        fn is_valid(&self) -> bool {
            self.valid
        }
    }

    #[test]
    fn test_provider_basic() {
        let provider = TestProvider {
            decorations: vec![],
            valid: true,
        };

        assert_eq!(provider.name(), "test");
        assert_eq!(provider.group(), DecorationGroup::Language);
        assert!(provider.is_valid());
    }

    #[test]
    fn test_provider_decorations_for_range() {
        use super::super::types::Span;

        let provider = TestProvider {
            decorations: vec![
                Decoration::conceal(Span::line(5, 0, 10), "a", None),
                Decoration::conceal(Span::line(10, 0, 10), "b", None),
                Decoration::conceal(Span::line(15, 0, 10), "c", None),
            ],
            valid: true,
        };

        // Range 0-4 should return nothing
        let result = provider.decorations_for_range(0, 4);
        assert_eq!(result.len(), 0);

        // Range 5-10 should return first two
        let result = provider.decorations_for_range(5, 10);
        assert_eq!(result.len(), 2);

        // Range 10-20 should return last two
        let result = provider.decorations_for_range(10, 20);
        assert_eq!(result.len(), 2);
    }
}
