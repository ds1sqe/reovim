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
    reovim_driver_syntax::{SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore},
    reovim_driver_syntax_treesitter::{CaptureMapper, Language, Query, TreeSitterDriver},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Rust highlights query (embedded from queries/highlights.scm)
const RUST_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Factory for creating Rust syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Rust syntax highlighting using the tree-sitter-rust grammar.
pub struct RustSyntaxFactory {
    /// Shared capture mapper (reused across driver instances)
    capture_mapper: Arc<CaptureMapper>,
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
}

impl RustSyntaxFactory {
    /// Create a new Rust syntax factory.
    ///
    /// Pre-compiles the highlights query for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded highlights query fails to compile.
    /// This should never happen with a correctly bundled query.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_rust::LANGUAGE.into();

        let highlight_query = Query::new(&language, RUST_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Rust highlights query");

        Self {
            capture_mapper: Arc::new(CaptureMapper::new()),
            highlight_query: Arc::new(highlight_query),
        }
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

        TreeSitterDriver::new(
            "rust",
            &language,
            self.highlight_query.clone(),
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
        // Self-register factory into SyntaxFactoryStore
        // (like VimModule does for resolvers, keybindings, etc.)
        let store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        store.add(Arc::new(RustSyntaxFactory::new()));

        tracing::info!("TreesitterRustModule: registered Rust syntax factory");
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
    fn test_create_driver() {
        let factory = RustSyntaxFactory::new();

        let driver = factory.create("rust");
        assert!(driver.is_some());

        let driver = factory.create("python");
        assert!(driver.is_none());
    }

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

        tracing::debug!("Highlights: {highlights:?}");
    }

    #[test]
    fn test_driver_language_id() {
        let factory = RustSyntaxFactory::new();
        let driver = factory.create("rust").unwrap();
        assert_eq!(driver.language(), "rust");
    }

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
    fn test_highlights_utf8_safety() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        // Parse code with Unicode
        let code = "let λ = 1; // émoji 🎉";
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

    #[test]
    fn test_empty_file() {
        let factory = RustSyntaxFactory::new();
        let mut driver = factory.create("rust").unwrap();

        driver.parse("");
        assert!(driver.is_parsed());

        let highlights = driver.highlights(0..0);
        assert!(highlights.is_empty());
    }
}
