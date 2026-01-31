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
    reovim_driver_syntax::{SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore},
    reovim_driver_syntax_treesitter::{CaptureMapper, Language, Query, TreeSitterDriver},
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
        // Self-register factory into SyntaxFactoryStore
        // (like TreesitterRustModule does)
        let store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        store.add(Arc::new(MarkdownSyntaxFactory::new()));

        tracing::info!("TreesitterMarkdownModule: registered Markdown syntax factory");
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
    fn test_create_driver() {
        let factory = MarkdownSyntaxFactory::new();

        let driver = factory.create("markdown");
        assert!(driver.is_some());

        let driver = factory.create("rust");
        assert!(driver.is_none());
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

        tracing::debug!("Highlights: {highlights:?}");
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
        let code = "# 日本語見出し\n\n- émoji 🎉";
        driver.parse(code);
        let highlights = driver.highlights(0..code.len());

        // All byte ranges should be valid UTF-8 boundaries
        for span in &highlights {
            assert!(
                code.is_char_boundary(span.start_byte),
                "start_byte {} is not a char boundary in '{}'",
                span.start_byte,
                code
            );
            assert!(
                code.is_char_boundary(span.end_byte),
                "end_byte {} is not a char boundary in '{}'",
                span.end_byte,
                code
            );
        }
    }
}
