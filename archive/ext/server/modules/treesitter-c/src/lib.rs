#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! C syntax highlighting module for reovim.
//!
//! Provides C language support for syntax highlighting
//! using tree-sitter-c and the `reovim-driver-syntax-treesitter` driver.
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
//! reovim-module-treesitter-c       (THIS CRATE - Module + C grammar)
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_c::CSyntaxFactory;
//! use reovim_driver_text_syntax::SyntaxDriverFactory;
//!
//! let factory = CSyntaxFactory::new();
//! let mut driver = factory.create("c").expect("C is supported");
//!
//! driver.parse("int main() { return 0; }");
//! let highlights = driver.highlights(0..100);
//!
//! assert!(!highlights.is_empty());
//! ```

use std::sync::Arc;

use {
    reovim_driver_syntax_treesitter::{Language, Query, TreeSitterDriver},
    reovim_driver_text_syntax::{
        CommentTokens, LanguageInfo, LanguageInfoStore, SyntaxDriver, SyntaxDriverFactory,
        SyntaxFactoryStore,
    },
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// C highlights query (embedded from queries/highlights.scm)
const C_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// C folds query (embedded from queries/folds.scm)
const C_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// C context query (embedded from queries/context.scm)
const C_CONTEXT_QUERY: &str = include_str!("queries/context.scm");

/// Factory for creating C syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// C syntax highlighting and fold detection using the tree-sitter-c grammar.
#[allow(clippy::struct_field_names)]
pub struct CSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
    /// Pre-compiled context query
    context_query: Arc<Query>,
}

impl CSyntaxFactory {
    /// Create a new C syntax factory.
    ///
    /// Pre-compiles the highlights and folds queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    /// This should never happen with correctly bundled queries.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_c::LANGUAGE.into();

        let highlight_query = Query::new(&language, C_HIGHLIGHTS_QUERY)
            .expect("Failed to compile C highlights query");

        let folds_query =
            Query::new(&language, C_FOLDS_QUERY).expect("Failed to compile C folds query");

        let context_query =
            Query::new(&language, C_CONTEXT_QUERY).expect("Failed to compile C context query");

        Self {
            highlight_query: Arc::new(highlight_query),
            folds_query: Arc::new(folds_query),
            context_query: Arc::new(context_query),
        }
    }

    /// Get the shared folds query.
    #[must_use]
    pub const fn folds_query(&self) -> &Arc<Query> {
        &self.folds_query
    }
}

impl Default for CSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for CSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "c" {
            return None;
        }

        let language: Language = tree_sitter_c::LANGUAGE.into();

        TreeSitterDriver::builder("c", &language, self.highlight_query.clone())
            .folds_query(self.folds_query.clone())
            .context_query(self.context_query.clone())
            .build()
            .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["c"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "c"
    }
}

// ============================================================================
// Module Implementation (Self-Registration Pattern)
// ============================================================================

/// Treesitter C syntax module.
///
/// Follows the self-registration pattern:
/// - Implements `Module` trait
/// - Registers `CSyntaxFactory` into `SyntaxFactoryStore` during `init()`
pub struct TreesitterCModule;

impl TreesitterCModule {
    /// Create a new Treesitter C module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterCModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterCModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-c")
    }

    fn name(&self) -> &'static str {
        "Treesitter C"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(CSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("c", "C")
                .with_extensions(["c", "h"])
                .with_mime_types(["text/x-c"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        tracing::info!("TreesitterCModule: registered C syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterCModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
