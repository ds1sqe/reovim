#![cfg_attr(coverage_nightly, allow(unused_features))]
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
        BracketConfig, BracketConfigStore, CommentTokens, LanguageInfo, LanguageInfoStore,
        SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore,
    },
    reovim_driver_syntax_treesitter::{Language, Query, TreeSitterDriver},
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

/// Rust context query (embedded from queries/context.scm)
/// Provides scope hierarchy for statusline breadcrumbs
const RUST_CONTEXT_QUERY: &str = include_str!("queries/context.scm");

/// Rust textobjects query (embedded from queries/textobjects.scm)
/// Provides semantic text object resolution (function, class, argument, etc.)
const RUST_TEXTOBJECTS_QUERY: &str = include_str!("queries/textobjects.scm");

/// Factory for creating Rust syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Rust syntax highlighting, fold detection, doc comment injection,
/// and indentation hints using the tree-sitter-rust grammar.
#[allow(clippy::struct_field_names)]
pub struct RustSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
    /// Pre-compiled injections query (doc comments → Markdown)
    injections_query: Arc<Query>,
    /// Pre-compiled indents query (indentation hints)
    indents_query: Arc<Query>,
    /// Pre-compiled context query (scope hierarchy)
    context_query: Arc<Query>,
    /// Pre-compiled textobjects query (semantic text objects)
    textobjects_query: Arc<Query>,
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

        let context_query = Query::new(&language, RUST_CONTEXT_QUERY)
            .expect("Failed to compile Rust context query");

        let textobjects_query = Query::new(&language, RUST_TEXTOBJECTS_QUERY)
            .expect("Failed to compile Rust textobjects query");

        Self {
            highlight_query: Arc::new(highlight_query),
            folds_query: Arc::new(folds_query),
            injections_query: Arc::new(injections_query),
            indents_query: Arc::new(indents_query),
            context_query: Arc::new(context_query),
            textobjects_query: Arc::new(textobjects_query),
        }
    }

    /// Get the shared folds query.
    #[must_use]
    pub const fn folds_query(&self) -> &Arc<Query> {
        &self.folds_query
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

        TreeSitterDriver::builder("rust", &language, self.highlight_query.clone())
            .folds_query(self.folds_query.clone())
            .injections_query(self.injections_query.clone())
            .indents_query(self.indents_query.clone())
            .context_query(self.context_query.clone())
            .textobjects_query(self.textobjects_query.clone())
            .build()
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
        let factory = Arc::new(RustSyntaxFactory::new());

        // Register as SyntaxDriverFactory (for creating Rust drivers and injection children)
        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        // Register language metadata for detection
        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("rust", "Rust")
                .with_extensions(["rs"])
                .with_mime_types(["text/x-rust"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        // Register bracket config for Rust
        // Note: <> excluded (operator conflict: ->, =>, <=, >=, <<, >>)
        // Note: ' excluded (lifetime annotations: 'a, 'static)
        let bracket_store = ctx.services.get_or_create::<BracketConfigStore>();
        bracket_store.add(
            BracketConfig::new("rust")
                .with_rainbow([('(', ')'), ('[', ']'), ('{', '}')])
                .with_autopair([('(', ')'), ('[', ']'), ('{', '}'), ('"', '"')])
                .with_highlight([('(', ')'), ('[', ']'), ('{', '}')]),
        );

        tracing::info!("TreesitterRustModule: registered Rust syntax and injection factories");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterRustModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::SYNTAX_HIGHLIGHTING]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TreesitterRustModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
