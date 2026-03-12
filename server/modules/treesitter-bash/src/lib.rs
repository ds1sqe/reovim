#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Bash syntax highlighting module for reovim.
//!
//! Provides Bash language support for syntax highlighting
//! using tree-sitter-bash and the `reovim-driver-syntax-treesitter` driver.
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
//! reovim-module-treesitter-bash    (THIS CRATE - Module + Bash grammar)
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_bash::BashSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = BashSyntaxFactory::new();
//! let mut driver = factory.create("bash").expect("Bash is supported");
//!
//! driver.parse("#!/bin/bash\necho \"hello\"");
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

/// Bash highlights query (embedded from queries/highlights.scm)
const BASH_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Bash folds query (embedded from queries/folds.scm)
const BASH_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// Factory for creating Bash syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Bash syntax highlighting and fold detection using the tree-sitter-bash grammar.
pub struct BashSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
}

impl BashSyntaxFactory {
    /// Create a new Bash syntax factory.
    ///
    /// Pre-compiles the highlights and folds queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    /// This should never happen with correctly bundled queries.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_bash::LANGUAGE.into();

        let highlight_query = Query::new(&language, BASH_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Bash highlights query");

        let folds_query =
            Query::new(&language, BASH_FOLDS_QUERY).expect("Failed to compile Bash folds query");

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

impl Default for BashSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for BashSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "bash" {
            return None;
        }

        let language: Language = tree_sitter_bash::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "bash",
            &language,
            self.highlight_query.clone(),
            Some(self.folds_query.clone()),
            None, // No injections
            None, // No indents
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["bash"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "bash"
    }
}

// ============================================================================
// Module Implementation (Self-Registration Pattern)
// ============================================================================

/// Treesitter Bash syntax module.
///
/// Follows the self-registration pattern:
/// - Implements `Module` trait
/// - Registers `BashSyntaxFactory` into `SyntaxFactoryStore` during `init()`
pub struct TreesitterBashModule;

impl TreesitterBashModule {
    /// Create a new Treesitter Bash module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterBashModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterBashModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-bash")
    }

    fn name(&self) -> &'static str {
        "Treesitter Bash"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(BashSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("bash", "Bash")
                .with_extensions(["sh", "bash", "zsh"])
                .with_comments(CommentTokens::line_only("#")),
        );

        tracing::info!("TreesitterBashModule: registered Bash syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterBashModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
