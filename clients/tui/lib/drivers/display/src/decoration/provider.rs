//! Decoration provider traits for extensible decoration sources.
//!
//! Plugins implement `DecorationProvider` to supply decorations for their
//! specific functionality (e.g., markdown concealment, search highlighting).
//!
//! Two provider types are available:
//!
//! - `DecorationProvider`: Per-buffer provider, refreshed with buffer content
//! - `BufferDecorationSource`: Global provider that takes `buffer_id` in queries
//!
//! # Architecture
//!
//! ```text
//! Per-buffer: DecorationProvider (created by factory for each buffer)
//!   - Markdown concealment
//!   - Syntax highlighting
//!
//! Global: BufferDecorationSource (single instance, queries with buffer_id)
//!   - Rainbow brackets (SharedPairState)
//!   - Search highlighting
//! ```

use {
    super::types::{Decoration, DecorationGroup},
    reovim_kernel::api::v1::BufferId,
};

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

// ============================================================================
// Buffer Decoration Source (global providers with buffer_id awareness)
// ============================================================================

/// Trait for global decoration sources that manage state across all buffers.
///
/// Unlike `DecorationProvider` which is per-buffer, `BufferDecorationSource`
/// maintains global state and accepts `buffer_id` in queries. This is suitable
/// for features like rainbow brackets where a single service tracks all buffers.
///
/// # Example
///
/// ```ignore
/// // Pair module implements this trait
/// impl BufferDecorationSource for SharedPairState {
///     fn name(&self) -> &'static str { "rainbow-brackets" }
///     fn group(&self) -> DecorationGroup { DecorationGroup::Syntax }
///
///     fn decorations_for_buffer(
///         &self,
///         buffer_id: BufferId,
///         content: &str,
///         cursor: (usize, usize),
///     ) -> Vec<Decoration> {
///         // Generate rainbow bracket decorations for this buffer
///     }
/// }
/// ```
///
/// # Architecture
///
/// This follows mechanism/policy separation:
/// - **Mechanism** (this driver): Trait definition
/// - **Policy** (modules): Implementations like `SharedPairState`
pub trait BufferDecorationSource: Send + Sync {
    /// Source name for debugging and logging.
    fn name(&self) -> &'static str;

    /// Priority group for layering.
    ///
    /// Decorations from higher-priority groups override those from lower groups.
    fn group(&self) -> DecorationGroup;

    /// Get decorations for a specific buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to get decorations for
    /// * `content` - Current buffer content (for computing bracket positions)
    /// * `cursor` - Current cursor position (line, col) for matched pair highlighting
    ///
    /// # Returns
    ///
    /// Vector of decorations for the buffer
    fn decorations_for_buffer(
        &self,
        buffer_id: BufferId,
        content: &str,
        cursor: (usize, usize),
    ) -> Vec<Decoration>;
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

    #[test]
    fn test_provider_refresh() {
        let mut provider = TestProvider {
            decorations: vec![],
            valid: false,
        };

        assert!(!provider.is_valid());
        provider.refresh("some content");
        assert!(provider.is_valid());
    }

    #[test]
    fn test_provider_default_is_valid() {
        // The default implementation of is_valid returns true
        struct MinimalProvider;
        impl DecorationProvider for MinimalProvider {
            fn name(&self) -> &'static str {
                "minimal"
            }
            fn group(&self) -> DecorationGroup {
                DecorationGroup::Language
            }
            fn decorations_for_range(&self, _start_line: u32, _end_line: u32) -> Vec<Decoration> {
                vec![]
            }
            fn refresh(&mut self, _content: &str) {}
        }

        let provider = MinimalProvider;
        assert!(provider.is_valid());
    }

    #[test]
    fn test_factory_default_supported_languages() {
        struct MinimalFactory;
        impl DecorationProviderFactory for MinimalFactory {
            #[allow(clippy::unnecessary_literal_bound)]
            fn name(&self) -> &str {
                "minimal"
            }
            fn create(&self, _language_id: &str) -> Option<Box<dyn DecorationProvider>> {
                None
            }
        }

        let factory = MinimalFactory;
        assert!(factory.supported_languages().is_empty());
    }
}
