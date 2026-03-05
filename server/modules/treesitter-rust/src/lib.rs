#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Rust syntax highlighting module for reovim.
//!
//! This module provides Rust language support for syntax highlighting
//! using tree-sitter-rust and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-syntax           (trait definitions + SyntaxFactoryStore)
//!         ^
//!         |
//! reovim-driver-syntax-treesitter  (generic tree-sitter driver)
//!         ^
//!         |
//! reovim-module-treesitter-rust    (THIS CRATE - Module + Rust grammar)
//! ```
//!
//! # Self-Registration Pattern
//!
//! This module follows the self-registration pattern (like `VimModule`):
//! - Implements `Module` trait
//! - Registers `RustSyntaxFactory` into `SyntaxFactoryStore` during `init()`
//! - Added to `DefaultsModule::create_modules()` for auto-loading
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_rust::RustSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = RustSyntaxFactory::new();
//! let mut driver = factory.create("rust").expect("Rust is supported");
//!
//! driver.parse("fn main() { println!(\"Hello!\"); }");
//! let highlights = driver.highlights(0..100);
//!
//! assert!(!highlights.is_empty());
//! ```

use std::sync::Arc;

use {
    reovim_driver_syntax::{
        CommentTokens, LanguageInfo, LanguageInfoStore, SyntaxDriver, SyntaxDriverFactory,
        SyntaxFactoryStore,
    },
    reovim_driver_syntax_treesitter::{
        CaptureMapper, InjectionLayer, InjectionLayerFactory, InjectionLayerStore, Language, Query,
        TreeSitterDriver,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Rust highlights query (embedded from queries/highlights.scm)
const RUST_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Rust folds query (embedded from queries/folds.scm)
const RUST_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// Rust injections query (embedded from queries/injections.scm)
/// Injects Markdown highlighting into doc comments (/// and //!)
const RUST_INJECTIONS_QUERY: &str = include_str!("queries/injections.scm");

/// Rust indents query (embedded from queries/indents.scm)
/// Provides indentation hints for smart auto-indent
const RUST_INDENTS_QUERY: &str = include_str!("queries/indents.scm");

/// Factory for creating Rust syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Rust syntax highlighting, fold detection, doc comment injection,
/// and indentation hints using the tree-sitter-rust grammar.
pub struct RustSyntaxFactory {
    /// Shared capture mapper (reused across driver instances)
    capture_mapper: Arc<CaptureMapper>,
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
    /// Pre-compiled injections query (doc comments → Markdown)
    injections_query: Arc<Query>,
    /// Pre-compiled indents query (indentation hints)
    indents_query: Arc<Query>,
}

impl RustSyntaxFactory {
    /// Create a new Rust syntax factory.
    ///
    /// Pre-compiles the highlights, folds, and injections queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    /// This should never happen with correctly bundled queries.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_rust::LANGUAGE.into();

        let highlight_query = Query::new(&language, RUST_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Rust highlights query");

        let folds_query =
            Query::new(&language, RUST_FOLDS_QUERY).expect("Failed to compile Rust folds query");

        let injections_query = Query::new(&language, RUST_INJECTIONS_QUERY)
            .expect("Failed to compile Rust injections query");

        let indents_query = Query::new(&language, RUST_INDENTS_QUERY)
            .expect("Failed to compile Rust indents query");

        Self {
            capture_mapper: Arc::new(CaptureMapper::new()),
            highlight_query: Arc::new(highlight_query),
            folds_query: Arc::new(folds_query),
            injections_query: Arc::new(injections_query),
            indents_query: Arc::new(indents_query),
        }
    }

    /// Get the shared folds query.
    #[must_use]
    pub const fn folds_query(&self) -> &Arc<Query> {
        &self.folds_query
    }

    /// Get the shared capture mapper.
    #[must_use]
    pub const fn capture_mapper(&self) -> &Arc<CaptureMapper> {
        &self.capture_mapper
    }
}

impl Default for RustSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for RustSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        // Only support "rust" language ID
        if language_id != "rust" {
            return None;
        }

        let language: Language = tree_sitter_rust::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "rust",
            &language,
            self.highlight_query.clone(),
            Some(self.folds_query.clone()),
            Some(self.injections_query.clone()), // Doc comments → Markdown injection
            Some(self.indents_query.clone()),    // Indentation hints
            self.capture_mapper.clone(),
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["rust"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "rust"
    }
}

impl InjectionLayerFactory for RustSyntaxFactory {
    fn create_layer(&self, capture_mapper: Arc<CaptureMapper>) -> Option<InjectionLayer> {
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        InjectionLayer::new("rust", &language, self.highlight_query.clone(), capture_mapper)
    }

    fn language_id(&self) -> &'static str {
        "rust"
    }
}

// ============================================================================
// Module Implementation (Self-Registration Pattern)
// ============================================================================

/// Treesitter Rust syntax module.
///
/// Follows the self-registration pattern (like `VimModule`):
/// - Implements `Module` trait
/// - Registers `RustSyntaxFactory` into `SyntaxFactoryStore` during `init()`
/// - Added to `DefaultsModule::create_modules()` for auto-loading
pub struct TreesitterRustModule;

impl TreesitterRustModule {
    /// Create a new Treesitter Rust module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterRustModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterRustModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-rust")
    }

    fn name(&self) -> &'static str {
        "Treesitter Rust"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(RustSyntaxFactory::new());

        // Register as SyntaxDriverFactory (for creating Rust drivers)
        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory.clone());

        // Register as InjectionLayerFactory (for embedding Rust in other languages)
        let injection_store = ctx.services.get_or_create::<InjectionLayerStore>();
        injection_store.add(factory);

        // Register language metadata for detection
        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("rust", "Rust")
                .with_extensions(["rs"])
                .with_mime_types(["text/x-rust"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        tracing::info!("TreesitterRustModule: registered Rust syntax and injection factories");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterRustModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_creation() {
        let factory = RustSyntaxFactory::new();
        assert!(factory.supports("rust"));
        assert!(!factory.supports("python"));
        assert_eq!(factory.supported_languages(), vec!["rust"]);
    }

    #[test]
    fn test_factory_default() {
        let factory = RustSyntaxFactory::default();
        assert!(factory.supports("rust"));
        assert!(!factory.supports("markdown"));
    }

    #[test]
    fn test_factory_supports_negative_cases() {
        let factory = RustSyntaxFactory::new();
        assert!(!factory.supports(""));
        assert!(!factory.supports("Rust"));
        assert!(!factory.supports("RUST"));
        assert!(!factory.supports("javascript"));
        assert!(!factory.supports("c++"));
    }

    #[test]
    fn test_factory_folds_query_accessor() {
        let factory = RustSyntaxFactory::new();
        let folds_query = factory.folds_query();
        // The folds query should be a valid Arc<Query> (not null/empty)
        // We just verify it is accessible and shared
        let _clone = Arc::clone(folds_query);
    }

    #[test]
    fn test_factory_capture_mapper_accessor() {
        let factory = RustSyntaxFactory::new();
        let mapper = factory.capture_mapper();
        let _clone = Arc::clone(mapper);
    }

    #[test]
    fn test_create_driver() {
        let factory = RustSyntaxFactory::new();

        let driver = factory.create("rust");
        assert!(driver.is_some());

        let driver = factory.create("python");
        assert!(driver.is_none());
    }

    #[test]
    fn test_create_driver_unsupported_languages() {
        let factory = RustSyntaxFactory::new();
        assert!(factory.create("").is_none());
        assert!(factory.create("Rust").is_none());
        assert!(factory.create("python").is_none());
        assert!(factory.create("markdown").is_none());
    }

    #[test]
    fn test_driver_not_parsed_before_parse() {
        let factory = RustSyntaxFactory::new();
        let driver = factory.create("rust").unwrap();
        assert!(!driver.is_parsed());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_driver_parse_and_highlights() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Parse simple Rust code
        driver.parse("fn main() { let x = 1; }");
        assert!(driver.is_parsed());

        // Get highlights
        let highlights = driver.highlights(0..100);

        // Should have highlights for: fn, main, let, x, =, 1
        assert!(!highlights.is_empty(), "Expected highlights for Rust code");

        // Check that 'fn' is highlighted as keyword.function
        let fn_highlight = highlights
            .iter()
            .find(|h| h.start_byte == 0 && h.end_byte == 2);
        assert!(fn_highlight.is_some(), "Expected 'fn' to be highlighted");
    }

    #[test]
    fn test_driver_language_id() {
        let factory = RustSyntaxFactory::new();
        let driver = factory.create("rust").unwrap();
        assert_eq!(driver.language(), "rust");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_highlights_contain_keyword_function() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("fn main() {}");
        let highlights = driver.highlights(0..100);

        // Find the 'fn' keyword (bytes 0-2)
        let fn_highlight = highlights
            .iter()
            .find(|h| h.start_byte == 0 && h.end_byte == 2);

        assert!(fn_highlight.is_some(), "Expected 'fn' highlight");
        assert_eq!(
            fn_highlight.unwrap().group,
            HighlightGroup::KeywordFunction,
            "'fn' should be KeywordFunction"
        );
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_highlights_contain_function_name() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("fn main() {}");
        let highlights = driver.highlights(0..100);

        // Find 'main' (bytes 3-7)
        let main_highlight = highlights
            .iter()
            .find(|h| h.start_byte == 3 && h.end_byte == 7);

        assert!(main_highlight.is_some(), "Expected 'main' highlight");
        assert_eq!(
            main_highlight.unwrap().group,
            HighlightGroup::Function,
            "'main' should be Function"
        );
    }

    #[test]
    fn test_highlights_contain_string() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse(r#"let s = "hello";"#);
        let highlights = driver.highlights(0..100);

        // Find a string highlight
        let string_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::String);

        assert!(string_highlight.is_some(), "Expected string highlight");
    }

    #[test]
    fn test_highlights_contain_comment() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("// comment\nfn main() {}");
        let highlights = driver.highlights(0..100);

        // Find comment highlight
        let comment_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::Comment);

        assert!(comment_highlight.is_some(), "Expected comment highlight");
    }

    #[test]
    fn test_highlights_contain_number() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("let x = 42;");
        let highlights = driver.highlights(0..100);

        let number_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::Number);

        assert!(number_highlight.is_some(), "Expected number highlight");
    }

    #[test]
    fn test_highlights_contain_type() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("let x: i32 = 1;");
        let highlights = driver.highlights(0..100);

        // i32 should be a builtin type
        let type_highlight = highlights.iter().find(|h| h.group.is_type());

        assert!(type_highlight.is_some(), "Expected type highlight for i32");
    }

    #[test]
    fn test_highlights_contain_variable() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("fn main() { let x = 1; }");
        let highlights = driver.highlights(0..100);

        // 'x' should be highlighted as a variable
        let var_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::Variable);

        assert!(var_highlight.is_some(), "Expected variable highlight");
    }

    #[test]
    fn test_highlights_contain_mutable_keyword() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // 'mut' is captured as @keyword by the highlights query
        driver.parse("fn main() { let mut x = 1; }");
        let highlights = driver.highlights(0..100);

        let keyword_highlight = highlights.iter().find(|h| h.group.is_keyword());

        assert!(keyword_highlight.is_some(), "Expected keyword highlight for 'mut' or 'fn'");
    }

    #[test]
    fn test_highlights_contain_macro() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("fn main() { println!(\"test\"); }");
        let highlights = driver.highlights(0..100);

        let macro_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::FunctionMacro);

        assert!(macro_highlight.is_some(), "Expected macro highlight for println!");
    }

    #[test]
    fn test_highlights_utf8_safety() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Parse code with Unicode
        let code = "let \u{03bb} = 1; // \u{00e9}moji \u{1f389}";
        driver.parse(code);
        let highlights = driver.highlights(0..code.len());

        // All byte ranges should be valid UTF-8 boundaries
        for span in &highlights {
            assert!(
                code.is_char_boundary(span.start_byte),
                "start_byte {} is not a char boundary in '{code}'",
                span.start_byte,
            );
            assert!(
                code.is_char_boundary(span.end_byte),
                "end_byte {} is not a char boundary in '{code}'",
                span.end_byte,
            );
        }
    }

    #[test]
    fn test_empty_file() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("");
        assert!(driver.is_parsed());

        let highlights = driver.highlights(0..0);
        assert!(highlights.is_empty());
    }

    #[test]
    fn test_highlights_beyond_content() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "fn main() {}";
        driver.parse(code);
        // Querying a range beyond the content should not panic
        let highlights = driver.highlights(code.len()..code.len() + 100);
        // May be empty or contain highlights at the boundary - just verify no panic
        let _ = highlights;
    }

    #[test]
    fn test_highlights_partial_range() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // "fn main() {}\nfn bar() {}" - two lines
        let code = "fn main() {}\nfn bar() {}";
        driver.parse(code);

        // Get highlights for only the first line (bytes 0..12)
        let highlights = driver.highlights(0..12);

        // All highlights should be within the requested range or overlap it
        for h in &highlights {
            assert!(
                h.start_byte < 12,
                "Highlight starts at {} which is outside requested range 0..12",
                h.start_byte
            );
        }
    }

    #[test]
    fn test_reparse_updates_highlights() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // First parse
        driver.parse("let x = 1;");
        let highlights1 = driver.highlights(0..100);
        assert!(
            highlights1
                .iter()
                .any(|h| h.group == HighlightGroup::Number),
            "Should have number highlight"
        );

        // Re-parse with different code
        driver.parse("fn foo() {}");
        let highlights2 = driver.highlights(0..100);
        assert!(
            highlights2
                .iter()
                .any(|h| h.group == HighlightGroup::KeywordFunction),
            "Should have function keyword after re-parse"
        );
        // Number should no longer appear
        assert!(
            !highlights2
                .iter()
                .any(|h| h.group == HighlightGroup::Number),
            "Number should not appear after re-parse to function code"
        );
    }

    #[test]
    fn test_incremental_update() {
        use reovim_driver_syntax::SyntaxEdit;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Initial parse
        let initial = "fn main() {}";
        driver.parse(initial);
        assert!(driver.is_parsed());

        // Simulate inserting " let x = 1;" inside the braces
        // "fn main() { let x = 1; }"
        let updated = "fn main() { let x = 1; }";
        let edit = SyntaxEdit::insert(
            11, // start_byte: after '{'
            0,  // start_row
            11, // start_col
            23, // new_end_byte: position after " let x = 1;"
            0,  // new_end_row
            23, // new_end_col
        );
        driver.update(updated, &edit);
        assert!(driver.is_parsed());

        let highlights = driver.highlights(0..updated.len());
        assert!(!highlights.is_empty(), "Should have highlights after incremental update");
    }

    #[test]
    fn test_injection_layer_factory() {
        let factory = RustSyntaxFactory::new();
        let mapper = Arc::new(CaptureMapper::new());

        // Should create a valid injection layer
        let layer = factory.create_layer(mapper);
        assert!(layer.is_some(), "Should create Rust injection layer");

        let layer = layer.unwrap();
        assert_eq!(layer.language_id(), "rust");
    }

    #[test]
    fn test_injection_layer_factory_language_id() {
        let factory = RustSyntaxFactory::new();
        assert_eq!(factory.language_id(), "rust");
    }

    #[test]
    fn test_injection_layer_factory_shared_mapper() {
        let factory = RustSyntaxFactory::new();
        let mapper1 = Arc::new(CaptureMapper::new());
        let mapper2 = Arc::new(CaptureMapper::new());

        // Should be able to create layers with different mappers
        let layer1 = factory.create_layer(mapper1);
        let layer2 = factory.create_layer(mapper2);
        assert!(layer1.is_some());
        assert!(layer2.is_some());
    }

    // ========================================================================
    // Doc Comment Injection Tests (Phase 12.4)
    // ========================================================================

    #[test]
    fn test_injections_detects_doc_comments() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("/// This is a doc comment\nfn main() {}");
        let injections = driver.injections();

        // Should detect one injection for the doc comment
        assert!(!injections.is_empty(), "Expected injection for doc comment");
        assert_eq!(injections[0].language_id, "markdown", "Doc comments should inject Markdown");
    }

    #[test]
    fn test_injections_inner_doc_comments() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("//! Module documentation\n\nfn foo() {}");
        let injections = driver.injections();

        assert!(!injections.is_empty(), "Expected injection for inner doc comment");
        assert_eq!(injections[0].language_id, "markdown");
    }

    #[test]
    fn test_no_injection_for_regular_comments() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("// Regular comment\nfn main() {}");
        let injections = driver.injections();

        // Regular comments should NOT produce injections
        assert!(injections.is_empty(), "Regular comments should not have injections");
    }

    #[test]
    fn test_multiple_doc_comments() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("/// First doc\n/// Second doc\nfn main() {}");
        let injections = driver.injections();

        // Each doc comment line should be a separate injection
        assert_eq!(injections.len(), 2, "Expected 2 injections for 2 doc comment lines");
    }

    #[test]
    fn test_injections_empty_for_no_doc_comments() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("fn main() { let x = 1; }");
        let injections = driver.injections();
        assert!(injections.is_empty(), "Code without doc comments should have no injections");
    }

    // ========================================================================
    // Indentation Hints Tests (Phase 12.4)
    // ========================================================================

    #[test]
    fn test_indent_for_function_body() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "fn main() {\n    let x = 1;\n}";
        driver.parse(code);

        // Line 0: fn main() { - top level, should be 0 or small
        let indent_0 = driver.indent_for(0);
        assert!(indent_0.is_some(), "Should have indent for line 0");

        // Line 1: let x = 1; - inside function, should be indented
        let indent_1 = driver.indent_for(1);
        assert!(indent_1.is_some(), "Should have indent for line 1");
        assert!(
            indent_1.unwrap() > indent_0.unwrap(),
            "Line inside function should have more indent"
        );
    }

    #[test]
    fn test_indent_for_nested_blocks() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "fn main() {\n    if true {\n        let x = 1;\n    }\n}";
        driver.parse(code);

        // Line 2: let x = 1; - should be double indented (function + if)
        let indent_2 = driver.indent_for(2);
        assert!(indent_2.is_some(), "Should have indent for nested line");
        assert!(indent_2.unwrap() >= 8, "Doubly nested should have at least 8 spaces indent");
    }

    #[test]
    fn test_indent_for_top_level() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "fn foo() {}\n\nfn bar() {}";
        driver.parse(code);

        // Top level functions should have minimal indent
        let indent = driver.indent_for(0);
        assert!(indent.is_some());
        // Function item itself doesn't increase indent (body does)
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_indent_for_out_of_range_line() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("fn main() {}");

        // Requesting indent for a line beyond the file should return None or 0
        let indent = driver.indent_for(100);
        // Either None or 0 is acceptable for out-of-range lines
        if let Some(level) = indent {
            assert_eq!(level, 0, "Out-of-range line should have zero indent");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_indent_for_empty_file() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("");

        let indent = driver.indent_for(0);
        // Empty file: either None or 0
        if let Some(level) = indent {
            assert_eq!(level, 0, "Empty file should have zero indent");
        }
    }

    // ========================================================================
    // Fold Detection Tests (Phase 12.3)
    // ========================================================================

    #[test]
    fn test_folds_function() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Multi-line function should be foldable
        let code = "fn main() {\n    let x = 1;\n    let y = 2;\n}";
        driver.parse(code);
        let folds = driver.folds();

        assert!(!folds.is_empty(), "Expected fold for function body");

        // Find the function fold
        let fn_fold = folds.iter().find(|f| f.kind == FoldKind::Function);
        assert!(fn_fold.is_some(), "Expected Function fold kind");

        let fold = fn_fold.unwrap();
        assert_eq!(fold.start_line, 0, "Fold should start at line 0");
        assert!(fold.end_line > fold.start_line, "Fold should span multiple lines");
    }

    #[test]
    fn test_folds_impl_block() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = r#"struct Foo;

impl Foo {
    fn bar(&self) {
        println!("bar");
    }
}"#;
        driver.parse(code);
        let folds = driver.folds();

        // Should have folds for both impl block and function
        assert!(folds.len() >= 2, "Expected folds for impl and function");

        // Find the impl fold (Class kind)
        let impl_fold = folds.iter().find(|f| f.kind == FoldKind::Class);
        assert!(impl_fold.is_some(), "Expected Class fold for impl block");
    }

    #[test]
    fn test_folds_single_line_ignored() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Single-line function should NOT create a fold
        let code = "fn single() {}";
        driver.parse(code);
        let folds = driver.folds();

        // Single-line constructs should not be foldable
        assert!(folds.is_empty(), "Single-line function should not create fold, got {folds:?}");
    }

    #[test]
    fn test_folds_preview_extracted() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "fn calculate_sum(a: i32, b: i32) -> i32 {\n    a + b\n}";
        driver.parse(code);
        let folds = driver.folds();

        assert!(!folds.is_empty(), "Expected fold for function");

        // Preview should contain meaningful content from first line
        let fold = &folds[0];
        assert!(!fold.preview.is_empty(), "Fold preview should not be empty");
        // Preview is the first line of the fold region (the block body)
        // It should be trimmed and limited
        assert!(fold.preview.len() <= 80, "Preview should be limited to 80 chars");
    }

    #[test]
    fn test_folds_struct_with_fields() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "struct Point {\n    x: i32,\n    y: i32,\n}";
        driver.parse(code);
        let folds = driver.folds();

        assert!(!folds.is_empty(), "Expected fold for struct fields");

        let struct_fold = folds.iter().find(|f| f.kind == FoldKind::Class);
        assert!(struct_fold.is_some(), "Expected Class fold for struct");
    }

    #[test]
    fn test_folds_enum_variants() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "enum Color {\n    Red,\n    Green,\n    Blue,\n}";
        driver.parse(code);
        let folds = driver.folds();

        assert!(!folds.is_empty(), "Expected fold for enum variants");

        let enum_fold = folds.iter().find(|f| f.kind == FoldKind::Class);
        assert!(enum_fold.is_some(), "Expected Class fold for enum");
    }

    #[test]
    fn test_folds_block_comment() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "/*\n * Multi-line\n * block comment\n */\nfn main() {}";
        driver.parse(code);
        let folds = driver.folds();

        let comment_fold = folds.iter().find(|f| f.kind == FoldKind::Comment);
        assert!(comment_fold.is_some(), "Expected Comment fold for block comment");
    }

    #[test]
    fn test_folds_match_expression() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = r#"fn example() {
    match x {
        1 => "one",
        _ => "other",
    }
}"#;
        driver.parse(code);
        let folds = driver.folds();

        // Should have folds for function and match
        assert!(folds.len() >= 2, "Expected folds for function and match, got {}", folds.len());

        // Match block should be foldable (as Block kind)
        assert!(
            folds.iter().any(|f| f.kind == FoldKind::Block),
            "Expected Block fold for match expression"
        );
    }

    #[test]
    fn test_folds_empty_file() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("");
        let folds = driver.folds();
        assert!(folds.is_empty(), "Empty file should have no folds");
    }

    #[test]
    fn test_folds_trait_definition() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "pub trait Foo {\n    fn bar(&self);\n    fn baz(&self);\n}";
        driver.parse(code);
        let folds = driver.folds();

        let trait_fold = folds.iter().find(|f| f.kind == FoldKind::Class);
        assert!(trait_fold.is_some(), "Expected Class fold for trait definition");
    }

    // ========================================================================
    // Module Trait Tests
    // ========================================================================

    #[test]
    fn test_module_new() {
        let module = TreesitterRustModule::new();
        // Just verify construction does not panic
        let _id = module.id();
    }

    #[test]
    fn test_module_default() {
        fn takes_default<T: Default>(val: T) -> T {
            drop(val);
            T::default()
        }
        let module = takes_default(TreesitterRustModule::new());
        assert_eq!(module.name(), "Treesitter Rust");
    }

    #[test]
    fn test_module_id() {
        let module = TreesitterRustModule::new();
        assert_eq!(module.id(), ModuleId::new("treesitter-rust"));
    }

    #[test]
    fn test_module_name() {
        let module = TreesitterRustModule::new();
        assert_eq!(module.name(), "Treesitter Rust");
    }

    #[test]
    fn test_module_version() {
        let module = TreesitterRustModule::new();
        let version = module.version();
        assert_eq!(version, Version::new(0, 9, 0));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_init() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let mut module = TreesitterRustModule::new();
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services.clone(),
            std::path::PathBuf::from("/tmp/test-data"),
            std::path::PathBuf::from("/tmp/test-cache"),
        );
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify that the factory was registered
        let syntax_store = services.get_or_create::<SyntaxFactoryStore>();
        let factory = syntax_store.find("rust");
        assert!(factory.is_some(), "Rust factory should be available after module init");
    }

    #[test]
    fn test_module_exit() {
        let mut module = TreesitterRustModule::new();
        let result = module.exit();
        assert!(result.is_ok());
    }

    // ========================================================================
    // Integration Tests - Realistic Rust Code
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_folds_realistic_rust_file() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Realistic Rust code with various foldable constructs
        let code = r#"//! Module documentation

use std::collections::HashMap;

/// A simple struct
pub struct Config {
    name: String,
    settings: HashMap<String, String>,
}

impl Config {
    /// Create a new config
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            settings: HashMap::new(),
        }
    }

    /// Get a setting value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.settings.get(key)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new("default")
    }
}

/// Trait for serializable types
pub trait Serialize {
    fn serialize(&self) -> String;
}

impl Serialize for Config {
    fn serialize(&self) -> String {
        format!("Config({})", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = Config::new("test");
        assert!(config.settings.is_empty());
    }
}
"#;
        driver.parse(code);
        let folds = driver.folds();

        // Count by kind
        let function_folds: Vec<_> = folds
            .iter()
            .filter(|f| f.kind == FoldKind::Function)
            .collect();
        let class_folds: Vec<_> = folds.iter().filter(|f| f.kind == FoldKind::Class).collect();

        // Should have multiple function folds (new, get, default, serialize, test_config_new)
        assert!(
            function_folds.len() >= 5,
            "Expected at least 5 function folds, got {}",
            function_folds.len()
        );

        // Should have class folds (struct, impl blocks, trait)
        assert!(
            class_folds.len() >= 5,
            "Expected at least 5 class folds (struct, impls, trait, mod), got {}",
            class_folds.len()
        );

        // Verify folds are sorted by line
        for i in 1..folds.len() {
            assert!(
                folds[i - 1].start_line <= folds[i].start_line,
                "Folds should be sorted by start_line"
            );
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_highlights_and_folds_combined() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = r#"fn main() {
    let message = "Hello, world!";
    println!("{}", message);
}"#;
        driver.parse(code);

        // Get highlights
        let highlights = driver.highlights(0..code.len());
        assert!(!highlights.is_empty(), "Expected highlights");

        // Verify we have function keyword
        let fn_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::KeywordFunction);
        assert!(fn_highlight.is_some(), "Expected KeywordFunction highlight");

        // Verify we have string
        let string_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::String);
        assert!(string_highlight.is_some(), "Expected String highlight");

        // Get folds
        let folds = driver.folds();
        assert!(!folds.is_empty(), "Expected folds for multi-line function");

        // Both systems should work together
        assert!(
            !highlights.is_empty() && !folds.is_empty(),
            "Both highlights and folds should be populated"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_nested_folds() {
        use reovim_driver_syntax::FoldKind;

        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        let code = "impl Foo {\n    fn bar(&self) {\n        if condition {\n            loop {\n                // nested code\n            }\n        }\n    }\n}";
        driver.parse(code);
        let folds = driver.folds();

        // Should have nested folds
        assert!(
            folds.len() >= 3,
            "Expected nested folds (impl, fn, if, loop), got {}",
            folds.len()
        );

        // Outer fold (impl) should start at line 0
        let impl_fold = folds.iter().find(|f| f.kind == FoldKind::Class);
        assert!(impl_fold.is_some(), "Expected Class fold for impl");
        assert_eq!(impl_fold.unwrap().start_line, 0, "Impl fold should start at line 0");
    }

    // ========================================================================
    // Send + Sync Tests
    // ========================================================================

    #[test]
    fn test_factory_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RustSyntaxFactory>();
    }

    #[test]
    fn test_driver_send_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
        let factory = RustSyntaxFactory::new();
        let driver = factory.create("rust").unwrap();
        // SyntaxDriver requires Send + Sync
        assert_send_sync(&*driver);
    }

    // ========================================================================
    // Multiple Factory Instances
    // ========================================================================

    #[test]
    fn test_multiple_factory_instances() {
        let factory1 = RustSyntaxFactory::new();
        let factory2 = RustSyntaxFactory::new();

        // Both should independently create working drivers
        let mut driver1 = factory1.create("rust").unwrap();
        let mut driver2 = factory2.create("rust").unwrap();

        driver1.parse("fn foo() {}");
        driver2.parse("let x = 1;");

        assert!(driver1.is_parsed());
        assert!(driver2.is_parsed());

        let h1 = driver1.highlights(0..100);
        let h2 = driver2.highlights(0..100);

        assert!(!h1.is_empty());
        assert!(!h2.is_empty());
    }
}
