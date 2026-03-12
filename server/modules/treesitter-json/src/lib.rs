#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! JSON syntax highlighting module for reovim.
//!
//! Provides JSON language support for syntax highlighting using
//! tree-sitter-json and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_json::JsonSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = JsonSyntaxFactory::new();
//! let mut driver = factory.create("json").expect("JSON is supported");
//!
//! driver.parse(r#"{"key": "value", "number": 42}"#);
//! let highlights = driver.highlights(0..100);
//!
//! assert!(!highlights.is_empty());
//! ```

use std::sync::Arc;

use {
    reovim_driver_syntax::{
        LanguageInfo, LanguageInfoStore, SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore,
    },
    reovim_driver_syntax_treesitter::{Language, Query, TreeSitterDriver},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// JSON highlights query (embedded from queries/highlights.scm)
const JSON_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Factory for creating JSON syntax drivers.
pub struct JsonSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
}

impl JsonSyntaxFactory {
    /// Create a new JSON syntax factory.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_json::LANGUAGE.into();

        let highlight_query = Query::new(&language, JSON_HIGHLIGHTS_QUERY)
            .expect("Failed to compile JSON highlights query");

        Self {
            highlight_query: Arc::new(highlight_query),
        }
    }
}

impl Default for JsonSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for JsonSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "json" {
            return None;
        }

        let language: Language = tree_sitter_json::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "json",
            &language,
            self.highlight_query.clone(),
            None, // No folds
            None, // No injections
            None, // No indents
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["json"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "json"
    }
}

// ============================================================================
// Module Implementation
// ============================================================================

/// Treesitter JSON syntax module.
pub struct TreesitterJsonModule;

impl TreesitterJsonModule {
    /// Create a new Treesitter JSON module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterJsonModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterJsonModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-json")
    }

    fn name(&self) -> &'static str {
        "Treesitter JSON"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(JsonSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(LanguageInfo::new("json", "JSON").with_extensions(["json"]));

        tracing::info!("TreesitterJsonModule: registered JSON syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterJsonModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
