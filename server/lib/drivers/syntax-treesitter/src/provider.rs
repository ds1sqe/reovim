//! Imperative decoration provider trait.
//!
//! Language modules implement [`DecorationProvider`] when declarative
//! [`DecorationRule`](reovim_driver_syntax::DecorationRule) mapping is
//! insufficient — for example, table rendering requires cross-row column
//! width analysis that cannot be expressed as a simple capture-name-to-kind
//! mapping.
//!
//! Providers are registered via
//! [`TreeSitterDriverBuilder::decoration_provider()`](crate::TreeSitterDriverBuilder::decoration_provider)
//! and called during [`SyntaxDriver::decorations()`](reovim_driver_syntax::SyntaxDriver::decorations)
//! with read access to the parsed tree and buffer content.

use std::ops::Range;

use {reovim_driver_syntax::Annotation, tree_sitter::Tree};

/// Imperative decoration provider for complex decoration logic.
///
/// Language modules implement this trait when declarative `DecorationRule`
/// mapping is insufficient (e.g., table rendering requires cross-row
/// column width analysis).
///
/// Providers are called during `TreeSitterDriver::decorations()` with
/// read access to the parsed tree and buffer content.
///
/// # Caching
///
/// Implementations SHOULD cache results keyed by content hash to avoid
/// redundant analysis on every `decorations()` call. The recommended
/// pattern is content-hash caching: hash the relevant source text and
/// return cached annotations when the hash matches.
///
/// # Thread Safety
///
/// Providers must be `Send + Sync`. Interior mutability (e.g., `RwLock`
/// for cache) is the standard approach.
pub trait DecorationProvider: Send + Sync {
    /// Generate decoration annotations from the parsed tree.
    ///
    /// Called on every `decorations()` invocation with:
    /// - `tree`: the current parse tree (read-only)
    /// - `content`: the full buffer content
    /// - `byte_range`: the visible byte range to generate decorations for
    ///
    /// Implementations should filter to nodes within `byte_range` and
    /// return only annotations that overlap it.
    fn decorations(&self, tree: &Tree, content: &str, byte_range: Range<usize>) -> Vec<Annotation>;
}

#[cfg(test)]
mod tests {
    use reovim_driver_syntax::HighlightCategory;

    use super::*;

    /// Verify `DecorationProvider` is object-safe.
    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn trait_is_object_safe() {
        fn _accepts_ref(_: &dyn DecorationProvider) {}
        fn _accepts_box(_: Box<dyn DecorationProvider>) {}
    }

    /// A no-op provider for testing.
    struct EmptyProvider;

    impl DecorationProvider for EmptyProvider {
        fn decorations(
            &self,
            _tree: &Tree,
            _content: &str,
            _byte_range: Range<usize>,
        ) -> Vec<Annotation> {
            Vec::new()
        }
    }

    #[test]
    fn empty_provider_returns_empty() {
        // We need a tree to call the provider — use a minimal one
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();

        let provider = EmptyProvider;
        let result = provider.decorations(&tree, "fn main() {}", 0..12);
        assert!(result.is_empty());
    }

    /// Provider that returns a fixed set of annotations.
    struct FixedProvider {
        annotations: Vec<Annotation>,
    }

    impl DecorationProvider for FixedProvider {
        fn decorations(
            &self,
            _tree: &Tree,
            _content: &str,
            _byte_range: Range<usize>,
        ) -> Vec<Annotation> {
            self.annotations.clone()
        }
    }

    #[test]
    fn fixed_provider_returns_annotations() {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();

        let provider = FixedProvider {
            annotations: vec![Annotation::new(
                0,
                2,
                HighlightCategory::new("markup.raw.block"),
            )],
        };
        let result = provider.decorations(&tree, "fn main() {}", 0..12);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_byte, 0);
        assert_eq!(result[0].end_byte, 2);
        assert_eq!(result[0].category.as_str(), "markup.raw.block");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn provider_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Box<dyn DecorationProvider>>();
        assert_sync::<Box<dyn DecorationProvider>>();
    }
}
