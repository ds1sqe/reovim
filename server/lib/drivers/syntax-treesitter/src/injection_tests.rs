use std::sync::Arc;

use reovim_driver_syntax::{Annotation, Injection, SyntaxDriver, SyntaxDriverFactory};
use tree_sitter::Query;

use super::*;

// ========================================================================
// Test factory for creating real tree-sitter drivers
// ========================================================================

struct TestRustFactory {
    language: tree_sitter::Language,
    highlight_query: Arc<Query>,
}

impl TestRustFactory {
    fn new() -> Self {
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        let highlight_query =
            Arc::new(Query::new(&language, "(identifier) @variable").unwrap());
        Self {
            language,
            highlight_query,
        }
    }
}

impl SyntaxDriverFactory for TestRustFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        use crate::TreeSitterDriver;
        if language_id != "rust" {
            return None;
        }
        TreeSitterDriver::new("rust", &self.language, self.highlight_query.clone())
            .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["rust"]
    }
}

// ========================================================================
// InjectionManager basic tests
// ========================================================================

#[test]
fn test_injection_manager_default() {
    let manager = InjectionManager::default();
    assert_eq!(manager.child_count(), 0);
    assert_eq!(manager.depth(), 0);
}

#[test]
fn test_injection_manager_new() {
    let manager = InjectionManager::new();
    assert_eq!(manager.child_count(), 0);
    assert_eq!(manager.depth(), 0);
    assert!(!manager.has_child("rust"));
}

#[test]
fn test_injection_manager_with_factory() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let manager = InjectionManager::with_factory(factory, 0);
    assert_eq!(manager.child_count(), 0);
    assert_eq!(manager.depth(), 0);
}

#[test]
fn test_injection_manager_set_factory() {
    let mut manager = InjectionManager::new();
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    manager.set_factory(factory);
    // Factory is now set (verified indirectly via highlight_injections)
    assert_eq!(manager.child_count(), 0);
}

#[test]
fn test_injection_manager_set_depth() {
    let mut manager = InjectionManager::new();
    assert_eq!(manager.depth(), 0);
    manager.set_depth(3);
    assert_eq!(manager.depth(), 3);
}

#[test]
fn test_injection_manager_has_child() {
    let manager = InjectionManager::new();
    assert!(!manager.has_child("rust"));
    assert!(!manager.has_child("python"));
}

#[test]
fn test_injection_manager_debug() {
    let manager = InjectionManager::new();
    let debug = format!("{manager:?}");
    assert!(debug.contains("InjectionManager"));
    assert!(debug.contains("child_count"));
    assert!(debug.contains("has_factory"));
    assert!(debug.contains("depth"));
}

#[test]
fn test_injection_manager_invalidate() {
    let mut manager = InjectionManager::new();
    manager.invalidate();
    assert_eq!(manager.child_count(), 0);
}

#[test]
fn test_injection_manager_highlight_injections_empty() {
    let mut manager = InjectionManager::new();
    let injections: Vec<Injection> = vec![];
    let highlights = manager.highlight_injections(&injections, "content", 0..100);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_highlight_injections_no_child() {
    let mut manager = InjectionManager::new();
    // Injection for a language we don't have a factory for
    let injections = vec![Injection::new("rust", 10..50, 0, 0, 2, 10)];
    let highlights = manager.highlight_injections(&injections, "fn main() {}", 0..100);
    // No factory, no child created
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_highlight_non_overlapping() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    // Injection range 100..200, query range 0..50 => no overlap
    let injections = vec![Injection::new("rust", 100..200, 5, 0, 10, 0)];
    let highlights = manager.highlight_injections(&injections, "fn main() {}", 0..50);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_highlight_injection_end_at_start_of_range() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    // Injection ends exactly where query range starts: no overlap
    let injections = vec![Injection::new("rust", 0..10, 0, 0, 0, 10)];
    let highlights = manager.highlight_injections(&injections, "let x = 1;", 10..20);
    assert!(highlights.is_empty());
}

#[test]
fn test_injection_manager_highlight_injection_start_at_end_of_range() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    // Injection starts exactly where query range ends: no overlap
    let injections = vec![Injection::new("rust", 20..30, 0, 0, 0, 10)];
    let highlights =
        manager.highlight_injections(&injections, "let x = 1;let y = 2;let z = 3;", 0..20);
    assert!(highlights.is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_injection_manager_dynamic_child_creation() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    let full_content = "# Title\n\nfn main() { let x = 1; }extra";
    let injection = Injection::new("rust", 10..34, 1, 0, 1, 24);

    assert_eq!(manager.child_count(), 0);

    let highlights =
        manager.highlight_injections(&[injection], full_content, 0..full_content.len());

    // Child was dynamically created
    assert_eq!(manager.child_count(), 1);
    assert!(manager.has_child("rust"));
    assert!(!highlights.is_empty(), "Expected highlights from Rust injection");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_injection_manager_reuses_cached_child() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    let full_content = "# Title\n\nfn main() { let x = 1; }extra";
    let injection = Injection::new("rust", 10..34, 1, 0, 1, 24);

    // First call creates child
    let _ = manager.highlight_injections(std::slice::from_ref(&injection), full_content, 0..full_content.len());
    assert_eq!(manager.child_count(), 1);

    // Second call reuses cached child
    let highlights =
        manager.highlight_injections(&[injection], full_content, 0..full_content.len());
    assert_eq!(manager.child_count(), 1); // Not duplicated
    assert!(!highlights.is_empty());
}

#[test]
fn test_injection_manager_skips_unsupported_language() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    let content = "print('hello')";
    let injection = Injection::new("python", 0..14, 0, 0, 0, 14);
    let highlights = manager.highlight_injections(&[injection], content, 0..content.len());

    assert!(highlights.is_empty());
    assert_eq!(manager.child_count(), 0);
}

#[test]
fn test_injection_manager_depth_limit() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    // At max depth - 1, children should still get factory
    let manager = InjectionManager::with_factory(factory.clone(), MAX_INJECTION_DEPTH - 1);
    assert_eq!(manager.depth(), MAX_INJECTION_DEPTH - 1);

    // At max depth, no more recursion
    let manager_at_max = InjectionManager::with_factory(factory, MAX_INJECTION_DEPTH);
    assert_eq!(manager_at_max.depth(), MAX_INJECTION_DEPTH);
}

#[test]
fn test_injection_manager_invalidate_with_children() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    let content = "fn main() { let x = 1; }";
    let injection = Injection::new("rust", 0..24, 0, 0, 0, 24);
    let _ = manager.highlight_injections(&[injection], content, 0..content.len());
    assert_eq!(manager.child_count(), 1);

    manager.invalidate();
    assert_eq!(manager.child_count(), 0);
}

#[test]
fn test_injection_manager_debug_with_children() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 2);

    let content = "fn main() {}";
    let injection = Injection::new("rust", 0..12, 0, 0, 0, 12);
    let _ = manager.highlight_injections(&[injection], content, 0..content.len());

    let debug = format!("{manager:?}");
    assert!(debug.contains("InjectionManager"));
    assert!(debug.contains("rust"));
    assert!(debug.contains("has_factory: true"));
    assert!(debug.contains("depth: 2"));
}

// ========================================================================
// highlight_single_injection tests
// ========================================================================

#[test]
fn test_highlight_single_injection_empty_ranges() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    let injection = Injection::combined("rust", vec![], 0, 0, 0, 0);
    let highlights =
        manager.highlight_injections(&[injection], "fn main() {}", 0..100);
    // Empty ranges -> no highlights (but child may be created if factory returns Some)
    assert!(highlights.is_empty());
}

#[test]
fn test_highlight_single_injection_out_of_bounds() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    let injection = Injection::new("rust", 100..200, 0, 0, 0, 0);
    let highlights = manager.highlight_injections(&[injection], "short", 0..200);
    assert!(highlights.is_empty());
}

// ========================================================================
// Doc comment prefix stripping tests
// ========================================================================

#[test]
fn test_strip_doc_comment_prefix_outer_with_space() {
    let (stripped, len) = strip_doc_comment_prefix("/// Hello world");
    assert_eq!(stripped, "Hello world");
    assert_eq!(len, 4);
}

#[test]
fn test_strip_doc_comment_prefix_outer_no_space() {
    let (stripped, len) = strip_doc_comment_prefix("///Hello");
    assert_eq!(stripped, "Hello");
    assert_eq!(len, 3);
}

#[test]
fn test_strip_doc_comment_prefix_empty_outer() {
    let (stripped, len) = strip_doc_comment_prefix("///");
    assert_eq!(stripped, "");
    assert_eq!(len, 3);
}

#[test]
fn test_strip_doc_comment_prefix_inner_with_space() {
    let (stripped, len) = strip_doc_comment_prefix("//! Module docs");
    assert_eq!(stripped, "Module docs");
    assert_eq!(len, 4);
}

#[test]
fn test_strip_doc_comment_prefix_inner_no_space() {
    let (stripped, len) = strip_doc_comment_prefix("//!Hello");
    assert_eq!(stripped, "Hello");
    assert_eq!(len, 3);
}

#[test]
fn test_strip_doc_comment_prefix_regular_comment() {
    let (stripped, len) = strip_doc_comment_prefix("// regular comment");
    assert_eq!(stripped, "// regular comment");
    assert_eq!(len, 0);
}

#[test]
fn test_strip_doc_comment_prefix_quadruple_slash() {
    // //// is a regular comment, NOT a doc comment
    let (stripped, len) = strip_doc_comment_prefix("////not doc");
    assert_eq!(stripped, "////not doc");
    assert_eq!(len, 0);
}

#[test]
fn test_strip_doc_comment_prefix_no_prefix() {
    let (stripped, len) = strip_doc_comment_prefix("fn main() {}");
    assert_eq!(stripped, "fn main() {}");
    assert_eq!(len, 0);
}

// ========================================================================
// translate_combined_highlight tests
// ========================================================================

#[test]
fn test_translate_combined_highlight_single_chunk() {
    let highlight = Annotation::new(0, 5, reovim_driver_syntax::HighlightCategory::new("variable"));
    let range_offsets = [(100, 0, 4)]; // src_start=100, dst_start=0, prefix_len=4
    #[allow(clippy::single_range_in_vec_init)]
    let source_ranges = vec![100..120];

    let result = translate_combined_highlight(&highlight, &range_offsets, &source_ranges);
    assert!(result.is_some());
    let ann = result.unwrap();
    assert_eq!(ann.start_byte, 104); // 100 + 4 + 0
    assert_eq!(ann.end_byte, 109); // 100 + 4 + 5
}

#[test]
fn test_translate_combined_highlight_second_chunk() {
    let range_offsets = vec![
        (10, 0, 4),  // first chunk: src 10..20, dst 0..6, prefix 4
        (30, 7, 4),  // second chunk: src 30..40, dst 7..13, prefix 4
    ];
    let source_ranges = vec![10..20, 30..40];

    // Highlight in the second chunk at dst position 8 (offset 1 within second chunk)
    let highlight = Annotation::new(8, 10, reovim_driver_syntax::HighlightCategory::new("variable"));

    let result = translate_combined_highlight(&highlight, &range_offsets, &source_ranges);
    assert!(result.is_some());
    let ann = result.unwrap();
    assert_eq!(ann.start_byte, 35); // 30 + 4 + 1
    assert_eq!(ann.end_byte, 37); // 30 + 4 + 3
}

#[test]
fn test_translate_combined_highlight_not_found() {
    let highlight = Annotation::new(100, 105, reovim_driver_syntax::HighlightCategory::new("variable"));
    let range_offsets = [(10, 0, 4)];
    #[allow(clippy::single_range_in_vec_init)]
    let source_ranges = vec![10..20];

    let result = translate_combined_highlight(&highlight, &range_offsets, &source_ranges);
    assert!(result.is_none());
}

// ========================================================================
// Combined injection highlighting test
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_highlight_combined_injection() {
    let factory: Arc<dyn SyntaxDriverFactory> = Arc::new(TestRustFactory::new());
    let mut manager = InjectionManager::with_factory(factory, 0);

    // Simulate doc comments:
    // "/// let x = 1;\n/// let y = 2;\n"
    let content = "/// let x = 1;\n/// let y = 2;\n";
    let injection = Injection::combined(
        "rust",
        vec![0..14, 15..29], // Two comment lines
        0, 0, 1, 14,
    );

    let highlights =
        manager.highlight_injections(&[injection], content, 0..content.len());

    // After prefix stripping, child sees "let x = 1;\nlet y = 2;\n"
    // Should produce highlights for identifiers x, y
    assert!(
        !highlights.is_empty(),
        "Expected highlights from combined injection"
    );

    // Verify highlights are in parent document coordinates
    for h in &highlights {
        assert!(
            h.start_byte < content.len(),
            "Highlight start {start} should be within content bounds {len}",
            start = h.start_byte,
            len = content.len()
        );
    }
}
