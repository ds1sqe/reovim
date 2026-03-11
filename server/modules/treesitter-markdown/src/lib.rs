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
        AnnotationKind, DecorationRule, HighlightCategory, LanguageInfo, LanguageInfoStore,
        SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore,
    },
    reovim_driver_syntax_treesitter::{
        InjectionLayer, InjectionLayerFactory, InjectionLayerStore, Language, Query,
        TreeSitterDriver,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Markdown highlights query (embedded from queries/highlights.scm)
const MARKDOWN_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Markdown injections query (embedded from queries/injections.scm)
const MARKDOWN_INJECTIONS_QUERY: &str = include_str!("queries/injections.scm");

/// Markdown decorations query (embedded from queries/decorations.scm)
const MARKDOWN_DECORATIONS_QUERY: &str = include_str!("queries/decorations.scm");

/// Markdown inline decorations query (embedded from `queries_inline/decorations.scm`)
const MARKDOWN_INLINE_DECORATIONS_QUERY: &str = include_str!("queries_inline/decorations.scm");

/// Factory for creating Markdown syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Markdown syntax highlighting using the tree-sitter-md grammar.
pub struct MarkdownSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled injections query (for code block language detection)
    injections_query: Arc<Query>,
    /// Pre-compiled decorations query (conceal, background, virtual text)
    decoration_query: Arc<Query>,
    /// Declarative rules mapping capture names to annotation kinds
    decoration_rules: Vec<DecorationRule>,
    /// Inline language grammar (for emphasis, bold, links, etc.)
    inline_language: Language,
    /// Pre-compiled inline decorations query
    inline_decoration_query: Arc<Query>,
    /// Inline decoration rules
    inline_decoration_rules: Vec<DecorationRule>,
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
        let inline_language: Language = tree_sitter_md::INLINE_LANGUAGE.into();

        let highlight_query = Query::new(&language, MARKDOWN_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Markdown highlights query");

        let injections_query = Query::new(&language, MARKDOWN_INJECTIONS_QUERY)
            .expect("Failed to compile Markdown injections query");

        let decoration_query = Query::new(&language, MARKDOWN_DECORATIONS_QUERY)
            .expect("Failed to compile Markdown decorations query");

        let inline_decoration_query =
            Query::new(&inline_language, MARKDOWN_INLINE_DECORATIONS_QUERY)
                .expect("Failed to compile Markdown inline decorations query");

        Self {
            highlight_query: Arc::new(highlight_query),
            injections_query: Arc::new(injections_query),
            decoration_query: Arc::new(decoration_query),
            decoration_rules: markdown_decoration_rules(),
            inline_language,
            inline_decoration_query: Arc::new(inline_decoration_query),
            inline_decoration_rules: markdown_inline_decoration_rules(),
        }
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

        TreeSitterDriver::builder("markdown", &language, self.highlight_query.clone())
            .injections_query(self.injections_query.clone())
            .decoration(self.decoration_query.clone(), self.decoration_rules.clone())
            .inline_decoration(
                &self.inline_language,
                self.inline_decoration_query.clone(),
                self.inline_decoration_rules.clone(),
            )
            .build()
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
    fn create_layer(&self) -> Option<InjectionLayer> {
        let language: Language = tree_sitter_md::LANGUAGE.into();
        InjectionLayer::new("markdown", &language, self.highlight_query.clone())
    }

    fn language_id(&self) -> &'static str {
        "markdown"
    }
}

// ============================================================================
// Decoration Rules
// ============================================================================

/// Build the declarative decoration rules for Markdown.
///
/// Each rule maps a capture name from `decorations.scm` to an `AnnotationKind`
/// and a `HighlightCategory` for theming.
fn markdown_decoration_rules() -> Vec<DecorationRule> {
    vec![
        // Heading markers → Conceal with level-specific icons (Nerd Font)
        DecorationRule {
            capture_name: "heading.1.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f0965} ".into()),
            },
            category: HighlightCategory::new("markup.heading.1"),
        },
        DecorationRule {
            capture_name: "heading.2.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f096c} ".into()),
            },
            category: HighlightCategory::new("markup.heading.2"),
        },
        DecorationRule {
            capture_name: "heading.3.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f096d} ".into()),
            },
            category: HighlightCategory::new("markup.heading.3"),
        },
        DecorationRule {
            capture_name: "heading.4.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f096e} ".into()),
            },
            category: HighlightCategory::new("markup.heading.4"),
        },
        DecorationRule {
            capture_name: "heading.5.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f096f} ".into()),
            },
            category: HighlightCategory::new("markup.heading.5"),
        },
        DecorationRule {
            capture_name: "heading.6.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{f0970} ".into()),
            },
            category: HighlightCategory::new("markup.heading.6"),
        },
        // List bullets → Conceal with unicode bullet
        DecorationRule {
            capture_name: "list.bullet".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{2022} ".into()),
            },
            category: HighlightCategory::new("markup.list"),
        },
        // Checkboxes
        DecorationRule {
            capture_name: "checkbox.unchecked".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{2610} ".into()),
            },
            category: HighlightCategory::new("markup.list.checkbox"),
        },
        DecorationRule {
            capture_name: "checkbox.checked".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{2713} ".into()),
            },
            category: HighlightCategory::new("markup.list.checkbox.checked"),
        },
        // Code blocks → Background
        DecorationRule {
            capture_name: "code_block".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("markup.raw.block"),
        },
        // Blockquote markers → Conceal with bar
        DecorationRule {
            capture_name: "blockquote.marker".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{2502} ".into()),
            },
            category: HighlightCategory::new("markup.quote.marker"),
        },
        // Horizontal rules → Conceal with line
        DecorationRule {
            capture_name: "horizontal_rule".into(),
            kind: AnnotationKind::Conceal {
                replacement: Some("\u{2500}".repeat(40)),
            },
            category: HighlightCategory::new("punctuation.special"),
        },
    ]
}

/// Build the declarative inline decoration rules for Markdown.
///
/// These rules target the inline grammar (`INLINE_LANGUAGE`) for emphasis,
/// bold, code spans, strikethrough, and links.
fn markdown_inline_decoration_rules() -> Vec<DecorationRule> {
    vec![
        // Code span delimiters (backticks) → Conceal (hide)
        DecorationRule {
            capture_name: "code_span.delimiter".into(),
            kind: AnnotationKind::Conceal { replacement: None },
            category: HighlightCategory::new("markup.raw.inline"),
        },
        // Code span content → Background
        DecorationRule {
            capture_name: "code_span".into(),
            kind: AnnotationKind::Background,
            category: HighlightCategory::new("markup.raw.inline"),
        },
        // Emphasis → Highlight (styled by client as italic)
        DecorationRule {
            capture_name: "emphasis".into(),
            kind: AnnotationKind::Highlight,
            category: HighlightCategory::new("markup.italic"),
        },
        // Strong emphasis → Highlight (styled by client as bold)
        DecorationRule {
            capture_name: "strong".into(),
            kind: AnnotationKind::Highlight,
            category: HighlightCategory::new("markup.bold"),
        },
        // Strikethrough → Highlight (styled by client as strikethrough)
        DecorationRule {
            capture_name: "strikethrough".into(),
            kind: AnnotationKind::Highlight,
            category: HighlightCategory::new("markup.strikethrough"),
        },
        // Link destination (URL) → Conceal (hide the URL)
        DecorationRule {
            capture_name: "link.destination".into(),
            kind: AnnotationKind::Conceal { replacement: None },
            category: HighlightCategory::new("markup.link.url"),
        },
    ]
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
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# Hello World");
        let highlights = driver.highlights(0..100);

        // Find heading highlight
        let heading_highlight = highlights
            .iter()
            .find(|h| h.category.as_str().starts_with("markup.heading"));

        assert!(heading_highlight.is_some(), "Expected heading highlight");
    }

    #[test]
    fn test_highlights_h2_heading() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("## Sub Heading");
        let highlights = driver.highlights(0..100);

        let heading_highlight = highlights
            .iter()
            .find(|h| h.category.as_str().starts_with("markup.heading"));

        assert!(heading_highlight.is_some(), "Expected heading highlight for h2");
    }

    #[test]
    fn test_highlights_contain_list_marker() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("- item 1\n- item 2");
        let highlights = driver.highlights(0..100);

        // Find list marker highlight
        let list_highlight = highlights
            .iter()
            .find(|h| h.category.as_str().starts_with("markup.list"));

        assert!(list_highlight.is_some(), "Expected list marker highlight");
    }

    #[test]
    fn test_highlights_contain_code_block() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("```\ncode\n```");
        let highlights = driver.highlights(0..100);

        // Find code block highlight (markup.raw)
        let code_highlight = highlights
            .iter()
            .find(|h| h.category.as_str().starts_with("markup.raw"));

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
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // First parse: heading (with trailing newline for proper parsing)
        driver.parse("# Hello\n\nSome text.");
        let h1 = driver.highlights(0..100);
        assert!(
            h1.iter()
                .any(|h| h.category.as_str().starts_with("markup.heading")),
            "Should have heading highlight"
        );

        // Re-parse: list
        driver.parse("- item 1\n- item 2");
        let h2 = driver.highlights(0..100);
        assert!(
            h2.iter()
                .any(|h| h.category.as_str().starts_with("markup.list")),
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

        let layer = factory.create_layer();
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
    fn test_injection_layer_factory_creates_independent_layers() {
        let factory = MarkdownSyntaxFactory::new();

        let layer1 = factory.create_layer();
        let layer2 = factory.create_layer();
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
                .any(|h| h.category.as_str().starts_with("markup.heading")),
            "Expected heading highlights in realistic document"
        );

        // Should have list highlights
        assert!(
            highlights
                .iter()
                .any(|h| h.category.as_str().starts_with("markup.list")),
            "Expected list highlights in realistic document"
        );

        // Should have code block highlights
        assert!(
            highlights
                .iter()
                .any(|h| h.category.as_str().starts_with("markup.raw")),
            "Expected code block highlights in realistic document"
        );

        // Injection detection
        let injections = driver.injections();
        assert_eq!(injections.len(), 1, "Expected one injection for rust code block");
        assert_eq!(injections[0].language_id, "rust");
    }

    // ========================================================================
    // Decoration Tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_decorations_heading_markers() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# H1\n\n## H2\n\n### H3\n");
        let decos = driver.decorations(0..200);

        // Should have Conceal annotations for heading markers
        let conceals: Vec<_> = decos
            .iter()
            .filter(|a| matches!(a.kind, AnnotationKind::Conceal { .. }))
            .collect();

        assert!(
            conceals.len() >= 3,
            "Expected at least 3 heading marker conceals, got {}: {conceals:?}",
            conceals.len()
        );

        // Verify the h1 marker is concealed with the correct icon
        let h1 = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.heading.1");
        assert!(h1.is_some(), "Expected markup.heading.1 decoration");
        assert!(
            matches!(&h1.unwrap().kind, AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{f0965}')),
            "H1 marker should conceal with Nerd Font icon"
        );
    }

    #[test]
    fn test_decorations_all_heading_levels() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# H1\n\n## H2\n\n### H3\n\n#### H4\n\n##### H5\n\n###### H6\n");
        let decos = driver.decorations(0..200);

        for level in 1..=6 {
            let category = format!("markup.heading.{level}");
            let found = decos.iter().any(|a| a.category.as_str() == category);
            assert!(found, "Expected decoration for {category}");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_decorations_list_bullets() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("- item one\n- item two\n");
        let decos = driver.decorations(0..100);

        let bullets: Vec<_> = decos
            .iter()
            .filter(|a| a.category.as_str() == "markup.list")
            .collect();

        assert_eq!(bullets.len(), 2, "Expected 2 bullet decorations, got: {bullets:?}");
        for b in &bullets {
            assert!(
                matches!(&b.kind, AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{2022}')),
                "Bullet should conceal with unicode bullet character"
            );
        }
    }

    #[test]
    fn test_decorations_list_bullets_mixed_markers() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("- minus\n+ plus\n* star\n");
        let decos = driver.decorations(0..100);

        let bullet_count = decos
            .iter()
            .filter(|a| a.category.as_str() == "markup.list")
            .count();

        assert_eq!(bullet_count, 3, "Expected 3 bullet decorations for -, +, *");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_decorations_checkboxes() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("- [ ] unchecked\n- [x] checked\n");
        let decos = driver.decorations(0..100);

        let unchecked = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.list.checkbox");
        assert!(unchecked.is_some(), "Expected unchecked checkbox decoration");
        assert!(
            matches!(&unchecked.unwrap().kind, AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{2610}')),
            "Unchecked checkbox should conceal with ballot box"
        );

        let checked = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.list.checkbox.checked");
        assert!(checked.is_some(), "Expected checked checkbox decoration");
        assert!(
            matches!(&checked.unwrap().kind, AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{2713}')),
            "Checked checkbox should conceal with check mark"
        );
    }

    #[test]
    fn test_decorations_code_block() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("```\nsome code\n```\n");
        let decos = driver.decorations(0..100);

        let code_bg = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.raw.block");
        assert!(code_bg.is_some(), "Expected code block background decoration");
        assert!(
            matches!(code_bg.unwrap().kind, AnnotationKind::Background),
            "Code block should use Background annotation kind"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_decorations_blockquote_marker() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("> quoted text\n");
        let decos = driver.decorations(0..100);

        let bq = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.quote.marker");
        assert!(bq.is_some(), "Expected blockquote marker decoration");
        assert!(
            matches!(&bq.unwrap().kind, AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{2502}')),
            "Blockquote marker should conceal with box drawing character"
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_decorations_horizontal_rule() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("---\n");
        let decos = driver.decorations(0..100);

        let hr = decos
            .iter()
            .find(|a| a.category.as_str() == "punctuation.special");
        assert!(hr.is_some(), "Expected horizontal rule decoration");
        assert!(
            matches!(&hr.unwrap().kind, AnnotationKind::Conceal { replacement: Some(r) } if r.contains('\u{2500}')),
            "Horizontal rule should conceal with box drawing line"
        );
    }

    #[test]
    fn test_decorations_empty_when_no_content() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("");
        let decos = driver.decorations(0..0);
        assert!(decos.is_empty(), "Empty document should have no decorations");
    }

    #[test]
    fn test_decorations_partial_range() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        // H1 at byte 0..4, H2 at byte 6..
        driver.parse("# H1\n\n## H2\n");
        let decos = driver.decorations(0..5);

        // Should only get h1 decoration, not h2
        assert!(
            decos
                .iter()
                .any(|a| a.category.as_str() == "markup.heading.1"),
            "Should include h1 in range 0..5"
        );
        assert!(
            !decos
                .iter()
                .any(|a| a.category.as_str() == "markup.heading.2"),
            "Should NOT include h2 in range 0..5"
        );
    }

    #[test]
    fn test_decorations_coexist_with_highlights() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# Hello\n\n- item\n");
        let highlights = driver.highlights(0..100);
        let decos = driver.decorations(0..100);

        // Both should return data
        assert!(!highlights.is_empty(), "Should have highlights");
        assert!(!decos.is_empty(), "Should have decorations");

        // Decorations should have different kinds than plain Highlight
        assert!(
            decos
                .iter()
                .all(|a| !matches!(a.kind, AnnotationKind::Highlight)),
            "Decorations should not use Highlight kind (that's what highlights() returns)"
        );
    }

    #[test]
    fn test_decoration_rules_count() {
        let rules = markdown_decoration_rules();
        assert_eq!(
            rules.len(),
            12,
            "Expected 12 decoration rules (6 headings + bullet + 2 checkboxes + code_block + blockquote + hr)"
        );
    }

    // ========================================================================
    // Inline Decoration Tests
    // ========================================================================

    #[test]
    fn test_inline_decoration_query_compiles() {
        let inline_lang: Language = tree_sitter_md::INLINE_LANGUAGE.into();
        assert!(
            Query::new(&inline_lang, MARKDOWN_INLINE_DECORATIONS_QUERY).is_ok(),
            "Inline decorations query should compile against INLINE_LANGUAGE"
        );
    }

    #[test]
    fn test_inline_decoration_rules_count() {
        let rules = markdown_inline_decoration_rules();
        assert_eq!(
            rules.len(),
            6,
            "Expected 6 inline decoration rules (code_span.delimiter + code_span + emphasis + strong + strikethrough + link.destination)"
        );
    }

    #[test]
    fn test_inline_decorations_emphasis() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("Hello *italic* world\n");
        let decos = driver.decorations(0..200);

        let emphasis = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.italic");
        assert!(emphasis.is_some(), "Expected emphasis decoration for *italic*");
    }

    #[test]
    fn test_inline_decorations_strong() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("Hello **bold** world\n");
        let decos = driver.decorations(0..200);

        let strong = decos.iter().find(|a| a.category.as_str() == "markup.bold");
        assert!(strong.is_some(), "Expected strong emphasis decoration for **bold**");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_inline_decorations_code_span() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("Use `code` here\n");
        let decos = driver.decorations(0..200);

        // Should have Background for code span
        let code_bg = decos.iter().find(|a| {
            a.category.as_str() == "markup.raw.inline"
                && matches!(a.kind, AnnotationKind::Background)
        });
        assert!(code_bg.is_some(), "Expected background decoration for `code`");

        // Should have Conceal for backtick delimiters
        let code_delim = decos.iter().find(|a| {
            a.category.as_str() == "markup.raw.inline"
                && matches!(a.kind, AnnotationKind::Conceal { .. })
        });
        assert!(code_delim.is_some(), "Expected concealed backtick delimiters");
    }

    #[test]
    fn test_inline_decorations_strikethrough() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("~~strikethrough~~\n");
        let decos = driver.decorations(0..200);

        let strike = decos
            .iter()
            .find(|a| a.category.as_str() == "markup.strikethrough");
        assert!(strike.is_some(), "Expected strikethrough decoration");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_inline_decorations_link() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("[click here](https://example.com)\n");
        let decos = driver.decorations(0..200);

        let link_url = decos.iter().find(|a| {
            a.category.as_str() == "markup.link.url"
                && matches!(a.kind, AnnotationKind::Conceal { replacement: None })
        });
        assert!(link_url.is_some(), "Expected concealed link destination for URL");
    }

    #[test]
    fn test_inline_and_block_decorations_coexist() {
        let factory = MarkdownSyntaxFactory::new();
        let mut driver = factory.create("markdown").unwrap();

        driver.parse("# Heading with *emphasis*\n\n- bullet\n");
        let decos = driver.decorations(0..200);

        // Block decoration: heading marker
        let heading = decos
            .iter()
            .any(|a| a.category.as_str() == "markup.heading.1");
        assert!(heading, "Should have block-level heading decoration");

        // Inline decoration: emphasis
        let emphasis = decos.iter().any(|a| a.category.as_str() == "markup.italic");
        assert!(emphasis, "Should have inline emphasis decoration");

        // Block decoration: bullet
        let bullet = decos.iter().any(|a| a.category.as_str() == "markup.list");
        assert!(bullet, "Should have block-level bullet decoration");
    }

    // ========================================================================
    // Injection Highlighting POC (Markdown + Rust)
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_markdown_with_rust_injection_highlighting() {
        use reovim_module_treesitter_rust::RustSyntaxFactory;

        let md_lang: Language = tree_sitter_md::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&md_lang, MARKDOWN_HIGHLIGHTS_QUERY).unwrap());
        let injections_query = Arc::new(Query::new(&md_lang, MARKDOWN_INJECTIONS_QUERY).unwrap());

        // Register Rust factory in injection store
        let store = Arc::new(InjectionLayerStore::new());
        store.add(Arc::new(RustSyntaxFactory::new()));

        // Create concrete Markdown driver with injections support
        let mut driver = TreeSitterDriver::with_queries(
            "markdown",
            &md_lang,
            highlight_query,
            None,
            Some(injections_query),
            None,
        )
        .unwrap();
        driver.set_injection_layer_store(store);

        // Parse Markdown with embedded Rust code block
        driver.parse("# Title\n\n```rust\nfn main() {}\n```\n");
        let highlights = driver.highlights(0..200);

        // Verify Markdown heading highlight exists
        assert!(
            highlights
                .iter()
                .any(|h| h.category.as_str().starts_with("markup.heading")),
            "Expected Markdown heading highlight"
        );

        // Verify Rust keyword highlight exists (from dynamic injection)
        assert!(
            highlights.iter().any(|h| h.category.as_str() == "keyword"
                || h.category.as_str() == "keyword.function"
                || h.category.as_str().starts_with("keyword")),
            "Expected Rust keyword highlight from dynamic injection, got: {highlights:?}"
        );
    }

    #[test]
    fn test_markdown_with_multiple_language_injections() {
        use reovim_module_treesitter_rust::RustSyntaxFactory;

        let md_lang: Language = tree_sitter_md::LANGUAGE.into();
        let highlight_query = Arc::new(Query::new(&md_lang, MARKDOWN_HIGHLIGHTS_QUERY).unwrap());
        let injections_query = Arc::new(Query::new(&md_lang, MARKDOWN_INJECTIONS_QUERY).unwrap());

        // Only Rust is available — Python injection should be silently skipped
        let store = Arc::new(InjectionLayerStore::new());
        store.add(Arc::new(RustSyntaxFactory::new()));

        let mut driver = TreeSitterDriver::with_queries(
            "markdown",
            &md_lang,
            highlight_query,
            None,
            Some(injections_query),
            None,
        )
        .unwrap();
        driver.set_injection_layer_store(store);

        driver.parse("```rust\nfn main() {}\n```\n\n```python\nprint('hello')\n```\n");

        let injections = driver.injections();
        assert_eq!(injections.len(), 2);

        let highlights = driver.highlights(0..200);

        // Rust highlights should be present
        assert!(
            highlights
                .iter()
                .any(|h| h.category.as_str().starts_with("keyword")),
            "Expected Rust highlights from injection"
        );
    }
}
