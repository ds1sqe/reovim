#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Go syntax highlighting module for reovim.
//!
//! Provides Go language support for syntax highlighting using
//! tree-sitter-go and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_go::GoSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = GoSyntaxFactory::new();
//! let mut driver = factory.create("go").expect("Go is supported");
//!
//! driver.parse("package main\n\nfunc main() {}");
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
    reovim_driver_syntax_treesitter::{Language, Query, TreeSitterDriver},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Go highlights query (embedded from queries/highlights.scm)
const GO_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Go folds query (embedded from queries/folds.scm)
const GO_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// Factory for creating Go syntax drivers.
pub struct GoSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
}

impl GoSyntaxFactory {
    /// Create a new Go syntax factory.
    ///
    /// Pre-compiles the highlights and folds queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_go::LANGUAGE.into();

        let highlight_query = Query::new(&language, GO_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Go highlights query");

        let folds_query =
            Query::new(&language, GO_FOLDS_QUERY).expect("Failed to compile Go folds query");

        Self {
            highlight_query: Arc::new(highlight_query),
            folds_query: Arc::new(folds_query),
        }
    }

    /// Get the shared folds query.
    #[must_use]
    pub const fn folds_query(&self) -> &Arc<Query> {
        &self.folds_query
    }
}

impl Default for GoSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for GoSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "go" {
            return None;
        }

        let language: Language = tree_sitter_go::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "go",
            &language,
            self.highlight_query.clone(),
            Some(self.folds_query.clone()),
            None, // No injections
            None, // No indents
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["go"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "go"
    }
}

// ============================================================================
// Module Implementation
// ============================================================================

/// Treesitter Go syntax module.
pub struct TreesitterGoModule;

impl TreesitterGoModule {
    /// Create a new Treesitter Go module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterGoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterGoModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-go")
    }

    fn name(&self) -> &'static str {
        "Treesitter Go"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(GoSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("go", "Go")
                .with_extensions(["go"])
                .with_mime_types(["text/x-go"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        tracing::info!("TreesitterGoModule: registered Go syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterGoModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
