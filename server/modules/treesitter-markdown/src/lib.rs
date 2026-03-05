//! Markdown syntax highlighting module for reovim.
//!
//! This module provides Markdown language support for syntax highlighting
//! using tree-sitter-md and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-syntax              (trait definitions + SyntaxFactoryStore)
//!         ^
//!         |
//! reovim-driver-syntax-treesitter   (generic tree-sitter driver)
//!         ^
//!         |
//! reovim-module-treesitter-markdown (THIS CRATE - Module + Markdown grammar)
//! ```
//!
//! # Self-Registration Pattern
//!
//! This module follows the self-registration pattern (like `RustSyntaxFactory`):
//! - Implements `Module` trait
//! - Registers `MarkdownSyntaxFactory` into `SyntaxFactoryStore` during `init()`
//! - Added to `DefaultsModule::create_modules()` for auto-loading
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_markdown::MarkdownSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = MarkdownSyntaxFactory::new();
//! let mut driver = factory.create("markdown").expect("Markdown is supported");
//!
//! driver.parse("# Hello World\n\nSome text.");
//! let highlights = driver.highlights(0..100);
//!
//! assert!(!highlights.is_empty());
//! ```

use std::sync::Arc;

use {
    reovim_driver_syntax::{
        LanguageInfo, LanguageInfoStore, SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore,
    },
    reovim_driver_syntax_treesitter::{
        CaptureMapper, InjectionLayer, InjectionLayerFactory, InjectionLayerStore, Language, Query,
        TreeSitterDriver,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Markdown highlights query (embedded from queries/highlights.scm)
const MARKDOWN_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Markdown injections query (embedded from queries/injections.scm)
const MARKDOWN_INJECTIONS_QUERY: &str = include_str!("queries/injections.scm");

/// Factory for creating Markdown syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Markdown syntax highlighting using the tree-sitter-md grammar.
pub struct MarkdownSyntaxFactory {
    /// Shared capture mapper (reused across driver instances)
    capture_mapper: Arc<CaptureMapper>,
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled injections query (for code block language detection)
    injections_query: Arc<Query>,
}

impl MarkdownSyntaxFactory {
    /// Create a new Markdown syntax factory.
    ///
    /// Pre-compiles the highlights and injections queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    /// This should never happen with correctly bundled queries.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_md::LANGUAGE.into();

        let highlight_query = Query::new(&language, MARKDOWN_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Markdown highlights query");

        let injections_query = Query::new(&language, MARKDOWN_INJECTIONS_QUERY)
            .expect("Failed to compile Markdown injections query");

        Self {
            capture_mapper: Arc::new(CaptureMapper::new()),
            highlight_query: Arc::new(highlight_query),
            injections_query: Arc::new(injections_query),
        }
    }

    /// Get the shared capture mapper.
    #[must_use]
    pub const fn capture_mapper(&self) -> &Arc<CaptureMapper> {
        &self.capture_mapper
    }
}

impl Default for MarkdownSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for MarkdownSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        // Only support "markdown" language ID
        if language_id != "markdown" {
            return None;
        }

        let language: Language = tree_sitter_md::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "markdown",
            &language,
            self.highlight_query.clone(),
            None, // No folds query yet
            Some(self.injections_query.clone()),
            None, // No indents query for Markdown
            self.capture_mapper.clone(),
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["markdown"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "markdown"
    }
}

impl InjectionLayerFactory for MarkdownSyntaxFactory {
    fn create_layer(&self, capture_mapper: Arc<CaptureMapper>) -> Option<InjectionLayer> {
        let language: Language = tree_sitter_md::LANGUAGE.into();
        InjectionLayer::new("markdown", &language, self.highlight_query.clone(), capture_mapper)
    }

    fn language_id(&self) -> &'static str {
        "markdown"
    }
}

// ============================================================================
// Module Implementation (Self-Registration Pattern)
// ============================================================================

/// Treesitter Markdown syntax module.
///
/// Follows the self-registration pattern (like `TreesitterRustModule`):
/// - Implements `Module` trait
/// - Registers `MarkdownSyntaxFactory` into `SyntaxFactoryStore` during `init()`
/// - Added to `DefaultsModule::create_modules()` for auto-loading
pub struct TreesitterMarkdownModule;

impl TreesitterMarkdownModule {
    /// Create a new Treesitter Markdown module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterMarkdownModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterMarkdownModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-markdown")
    }

    fn name(&self) -> &'static str {
        "Treesitter Markdown"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(MarkdownSyntaxFactory::new());

        // Register as SyntaxDriverFactory (for creating Markdown drivers)
        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory.clone());

        // Register as InjectionLayerFactory (for embedding Markdown in other languages)
        // This enables Rust doc comments to inject Markdown highlighting
        let injection_store = ctx.services.get_or_create::<InjectionLayerStore>();
        injection_store.add(factory);

        // Register language metadata for detection
        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("markdown", "Markdown").with_extensions(["md", "markdown", "mdx"]),
        );

        tracing::info!(
            "TreesitterMarkdownModule: registered Markdown syntax and injection factories"
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterMarkdownModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_creation() {
        let factory = MarkdownSyntaxFactory::new();
        assert!(factory.supports("markdown"));
        assert!(!factory.supports("rust"));
        assert_eq!(factory.supported_languages(), vec!["markdown"]);
    }

    #[test]
    fn test_factory_default() {
        let factory = MarkdownSyntaxFactory::default();
        assert!(factory.supports("markdown"));
        assert!(!factory.supports("rust"));
    }

    #[test]
    fn test_factory_supports_negative_cases() {
        let factory = MarkdownSyntaxFactory::new();
        assert!(!factory.supports(""));
        assert!(!factory.supports("Markdown"));
        assert!(!factory.supports("MARKDOWN"));
        assert!(!factory.supports("md"));
        assert!(!factory.supports("html"));
    }

    #[test]
    fn test_factory_capture_mapper_accessor() {
        let factory = MarkdownSyntaxFactory::new();
        let mapper = factory.capture_mapper();
        let _clone = Arc::clone(mapper);
    }

    #[test]
    fn test_create_driver() {
        let factory = MarkdownSyntaxFactory::new();

        let driver = factory.create("markdown");
        assert!(driver.is_some());

        let driver = factory.create("rust");
        assert!(driver.is_none());
    }

    #[test]
    fn test_create_driver_unsupported_languages() {
        let factory = MarkdownSyntaxFactory::new();
        assert!(factory.create("").is_none());
        assert!(factory.create("Markdown").is_none());
        assert!(factory.create("md").is_none());
        assert!(factory.create("rust").is_none());
        assert!(factory.create("html").is_none());
    }

    #[test]
    fn test_driver_not_parsed_before_parse() {
        let factory = MarkdownSyntaxFactory::new();
        let driver = factory.create("markdown").unwrap();
        assert!(!driver.is_parsed());
    }

    #[test]
    fn test_driver_parse_and_highlights() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // Parse simple Markdown
        driver.parse("# Hello World\n\nSome text.");
        assert!(driver.is_parsed());

        // Get highlights
        let highlights = driver.highlights(0..100);

        // Should have highlights for heading
        assert!(!highlights.is_empty(), "Expected highlights for Markdown");
    }

    #[test]
    fn test_driver_language_id() {
        let factory = MarkdownSyntaxFactory::new();
        let driver = factory.create("markdown").unwrap();
        assert_eq!(driver.language(), "markdown");
    }

    #[test]
    fn test_highlights_contain_heading() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# Hello World");
        let highlights = driver.highlights(0..100);

        // Find heading highlight
        let heading_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::MarkupHeading);

        assert!(heading_highlight.is_some(), "Expected heading highlight");
    }

    #[test]
    fn test_highlights_h2_heading() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("## Sub Heading");
        let highlights = driver.highlights(0..100);

        let heading_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::MarkupHeading);

        assert!(heading_highlight.is_some(), "Expected heading highlight for h2");
    }

    #[test]
    fn test_highlights_contain_list_marker() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("- item 1\n- item 2");
        let highlights = driver.highlights(0..100);

        // Find list marker highlight
        let list_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::MarkupList);

        assert!(list_highlight.is_some(), "Expected list marker highlight");
    }

    #[test]
    fn test_highlights_contain_code_block() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("```\ncode\n```");
        let highlights = driver.highlights(0..100);

        // Find code block highlight (markup.raw)
        let code_highlight = highlights
            .iter()
            .find(|h| h.group == HighlightGroup::MarkupRaw);

        assert!(code_highlight.is_some(), "Expected code block highlight");
    }

    #[test]
    fn test_highlights_empty_range() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# Hello");
        let highlights = driver.highlights(0..0);
        assert!(highlights.is_empty());
    }

    #[test]
    fn test_highlights_partial_range() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        let code = "# Heading 1\n\n## Heading 2";
        driver.parse(code);

        // Only query first line
        let highlights = driver.highlights(0..11);
        for h in &highlights {
            assert!(
                h.start_byte < 11,
                "Highlight starts at {} which is outside requested range",
                h.start_byte
            );
        }
    }

    #[test]
    fn test_reparse_updates_highlights() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // First parse: heading (with trailing newline for proper parsing)
        driver.parse("# Hello\n\nSome text.");
        let h1 = driver.highlights(0..100);
        assert!(
            h1.iter().any(|h| h.group == HighlightGroup::MarkupHeading),
            "Should have heading highlight"
        );

        // Re-parse: list
        driver.parse("- item 1\n- item 2");
        let h2 = driver.highlights(0..100);
        assert!(
            h2.iter().any(|h| h.group == HighlightGroup::MarkupList),
            "Should have list highlight after re-parse"
        );
    }

    #[test]
    fn test_incremental_update() {
        use reovim_driver_syntax::SyntaxEdit;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // Initial parse
        driver.parse("# Hello");
        assert!(driver.is_parsed());

        // Insert " World" at the end
        let updated = "# Hello World";
        let edit = SyntaxEdit::insert(7, 0, 7, 13, 0, 13);
        driver.update(updated, &edit);
        assert!(driver.is_parsed());

        let highlights = driver.highlights(0..updated.len());
        assert!(!highlights.is_empty(), "Should have highlights after incremental update");
    }

    #[test]
    fn test_injections_detects_language() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("```rust\nfn main() {}\n```");
        let injections = driver.injections();

        // Should detect one injection for the Rust code block
        assert_eq!(injections.len(), 1, "Expected one injection for Rust code block");
        assert_eq!(injections[0].language_id, "rust", "Expected Rust language ID");
    }

    #[test]
    fn test_injections_multiple_languages() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("```rust\nfn main() {}\n```\n\n```python\nprint('hello')\n```");
        let injections = driver.injections();

        // Should detect two injections
        assert_eq!(injections.len(), 2, "Expected two injections");

        let languages: Vec<&str> = injections.iter().map(|i| i.language_id.as_str()).collect();
        assert!(languages.contains(&"rust"), "Expected Rust injection");
        assert!(languages.contains(&"python"), "Expected Python injection");
    }

    #[test]
    fn test_injections_no_language_tag() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // Code block without language tag should have no injections
        driver.parse("```\nsome code\n```");
        let injections = driver.injections();
        assert!(
            injections.is_empty(),
            "Code block without language tag should have no injections"
        );
    }

    #[test]
    fn test_injections_empty_for_plain_text() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("Just some plain text.\n\nAnother paragraph.");
        let injections = driver.injections();
        assert!(injections.is_empty(), "Plain text should have no injections");
    }

    #[test]
    fn test_folds_returns_empty_for_markdown() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // Markdown factory does not configure folds query
        driver.parse("# Heading\n\nParagraph.\n\n## Sub heading");
        let folds = driver.folds();
        assert!(folds.is_empty(), "Markdown should not have folds (no folds query configured)");
    }

    #[test]
    fn test_indent_returns_none_for_markdown() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# Heading\n\nParagraph.");
        let indent = driver.indent_for(0);
        assert!(indent.is_none(), "Markdown should not have indent hints (no indents query)");
    }

    #[test]
    fn test_empty_file() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("");
        assert!(driver.is_parsed());

        let highlights = driver.highlights(0..0);
        assert!(highlights.is_empty());
    }

    #[test]
    fn test_utf8_safety() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // Parse Markdown with Unicode
        let code = "# \u{65e5}\u{672c}\u{8a9e}\u{898b}\u{51fa}\u{3057}\n\n- \u{00e9}moji \u{1f389}";
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

    // ========================================================================
    // Injection Layer Factory Tests
    // ========================================================================

    #[test]
    fn test_injection_layer_factory() {
        let factory = MarkdownSyntaxFactory::new();
        let mapper = Arc::new(CaptureMapper::new());

        let layer = factory.create_layer(mapper);
        assert!(layer.is_some(), "Should create Markdown injection layer");

        let layer = layer.unwrap();
        assert_eq!(layer.language_id(), "markdown");
    }

    #[test]
    fn test_injection_layer_factory_language_id() {
        let factory = MarkdownSyntaxFactory::new();
        assert_eq!(factory.language_id(), "markdown");
    }

    #[test]
    fn test_injection_layer_factory_multiple_mappers() {
        let factory = MarkdownSyntaxFactory::new();
        let mapper1 = Arc::new(CaptureMapper::new());
        let mapper2 = Arc::new(CaptureMapper::new());

        let layer1 = factory.create_layer(mapper1);
        let layer2 = factory.create_layer(mapper2);
        assert!(layer1.is_some());
        assert!(layer2.is_some());
    }

    // ========================================================================
    // Module Trait Tests
    // ========================================================================

    #[test]
    fn test_module_new() {
        let module = TreesitterMarkdownModule::new();
        let _id = module.id();
    }

    #[test]
    fn test_module_default() {
        fn takes_default<T: Default>(val: T) -> T {
            drop(val);
            T::default()
        }
        let module = takes_default(TreesitterMarkdownModule::new());
        assert_eq!(module.name(), "Treesitter Markdown");
    }

    #[test]
    fn test_module_id() {
        let module = TreesitterMarkdownModule::new();
        assert_eq!(module.id(), ModuleId::new("treesitter-markdown"));
    }

    #[test]
    fn test_module_name() {
        let module = TreesitterMarkdownModule::new();
        assert_eq!(module.name(), "Treesitter Markdown");
    }

    #[test]
    fn test_module_version() {
        let module = TreesitterMarkdownModule::new();
        let version = module.version();
        assert_eq!(version, Version::new(0, 9, 0));
    }

    #[test]
    fn test_module_init() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let mut module = TreesitterMarkdownModule::new();
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
        let factory = syntax_store.find("markdown");
        assert!(factory.is_some(), "Markdown factory should be available after module init");
    }

    #[test]
    fn test_module_exit() {
        let mut module = TreesitterMarkdownModule::new();
        let result = module.exit();
        assert!(result.is_ok());
    }

    // ========================================================================
    // Send + Sync Tests
    // ========================================================================

    #[test]
    fn test_factory_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MarkdownSyntaxFactory>();
    }

    #[test]
    fn test_driver_send_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>(_: &T) {}
        let factory = MarkdownSyntaxFactory::new();
        let driver = factory.create("markdown").unwrap();
        assert_send_sync(&*driver);
    }

    // ========================================================================
    // Multiple Factory Instances
    // ========================================================================

    #[test]
    fn test_multiple_factory_instances() {
        let factory1 = MarkdownSyntaxFactory::new();
        let factory2 = MarkdownSyntaxFactory::new();

        let mut driver1 = factory1.create("markdown").unwrap();
        let mut driver2 = factory2.create("markdown").unwrap();

        driver1.parse("# Heading 1");
        driver2.parse("- list item");

        assert!(driver1.is_parsed());
        assert!(driver2.is_parsed());

        let h1 = driver1.highlights(0..100);
        let h2 = driver2.highlights(0..100);

        assert!(!h1.is_empty());
        assert!(!h2.is_empty());
    }

    // ========================================================================
    // Realistic Markdown Tests
    // ========================================================================

    #[test]
    fn test_realistic_markdown_document() {
        use reovim_driver_syntax::HighlightGroup;

        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        let code = r#"# Project Title

A short description.

## Features

- Feature one
- Feature two
- Feature three

## Code Example

```rust
fn main() {
    println!("Hello!");
}
```

## License

MIT
"#;
        driver.parse(code);
        let highlights = driver.highlights(0..code.len());

        // Should have heading highlights
        assert!(
            highlights
                .iter()
                .any(|h| h.group == HighlightGroup::MarkupHeading),
            "Expected heading highlights in realistic document"
        );

        // Should have list highlights
        assert!(
            highlights
                .iter()
                .any(|h| h.group == HighlightGroup::MarkupList),
            "Expected list highlights in realistic document"
        );

        // Should have code block highlights
        assert!(
            highlights
                .iter()
                .any(|h| h.group == HighlightGroup::MarkupRaw),
            "Expected code block highlights in realistic document"
        );

        // Injection detection
        let injections = driver.injections();
        assert_eq!(injections.len(), 1, "Expected one injection for rust code block");
        assert_eq!(injections[0].language_id, "rust");
    }
}
