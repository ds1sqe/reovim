#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! JavaScript syntax highlighting module for reovim.
//!
//! Provides JavaScript language support for syntax highlighting using
//! tree-sitter-javascript and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_javascript::JavaScriptSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = JavaScriptSyntaxFactory::new();
//! let mut driver = factory.create("javascript").expect("JavaScript is supported");
//!
//! driver.parse("function hello() { console.log('hi'); }");
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

/// JavaScript highlights query (embedded from queries/highlights.scm)
const JS_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// JavaScript folds query (embedded from queries/folds.scm)
const JS_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// Factory for creating JavaScript syntax drivers.
pub struct JavaScriptSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
}

impl JavaScriptSyntaxFactory {
    /// Create a new JavaScript syntax factory.
    ///
    /// Pre-compiles the highlights and folds queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_javascript::LANGUAGE.into();

        let highlight_query = Query::new(&language, JS_HIGHLIGHTS_QUERY)
            .expect("Failed to compile JavaScript highlights query");

        let folds_query = Query::new(&language, JS_FOLDS_QUERY)
            .expect("Failed to compile JavaScript folds query");

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

impl Default for JavaScriptSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for JavaScriptSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "javascript" {
            return None;
        }

        let language: Language = tree_sitter_javascript::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "javascript",
            &language,
            self.highlight_query.clone(),
            Some(self.folds_query.clone()),
            None, // No injections
            None, // No indents
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["javascript"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "javascript"
    }
}

// ============================================================================
// Module Implementation
// ============================================================================

/// Treesitter JavaScript syntax module.
pub struct TreesitterJavaScriptModule;

impl TreesitterJavaScriptModule {
    /// Create a new Treesitter JavaScript module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterJavaScriptModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterJavaScriptModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-javascript")
    }

    fn name(&self) -> &'static str {
        "Treesitter JavaScript"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(JavaScriptSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("javascript", "JavaScript")
                .with_extensions(["js", "mjs", "cjs", "jsx"])
                .with_mime_types(["text/javascript"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        tracing::info!("TreesitterJavaScriptModule: registered JavaScript syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterJavaScriptModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
