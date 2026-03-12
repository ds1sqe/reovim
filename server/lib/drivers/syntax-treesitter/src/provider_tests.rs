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
