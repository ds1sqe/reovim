#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TOML syntax highlighting module for reovim.
//!
//! Provides TOML language support for syntax highlighting using
//! tree-sitter-toml and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_toml::TomlSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = TomlSyntaxFactory::new();
//! let mut driver = factory.create("toml").expect("TOML is supported");
//!
//! driver.parse("[package]\nname = \"test\"");
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

/// TOML highlights query (embedded from queries/highlights.scm)
const TOML_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// TOML context query (embedded from queries/context.scm)
const TOML_CONTEXT_QUERY: &str = include_str!("queries/context.scm");

/// Factory for creating TOML syntax drivers.
#[allow(clippy::struct_field_names)]
pub struct TomlSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled context query
    context_query: Arc<Query>,
}

impl TomlSyntaxFactory {
    /// Create a new TOML syntax factory.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_toml_ng::LANGUAGE.into();

        let highlight_query = Query::new(&language, TOML_HIGHLIGHTS_QUERY)
            .expect("Failed to compile TOML highlights query");

        let context_query = Query::new(&language, TOML_CONTEXT_QUERY)
            .expect("Failed to compile TOML context query");

        Self {
            highlight_query: Arc::new(highlight_query),
            context_query: Arc::new(context_query),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for TomlSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for TomlSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "toml" {
            return None;
        }

        let language: Language = tree_sitter_toml_ng::LANGUAGE.into();

        TreeSitterDriver::builder("toml", &language, self.highlight_query.clone())
            .context_query(self.context_query.clone())
            .build()
            .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["toml"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "toml"
    }
}

// ============================================================================
// Module Implementation
// ============================================================================

/// Treesitter TOML syntax module.
pub struct TreesitterTomlModule;

impl TreesitterTomlModule {
    /// Create a new Treesitter TOML module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for TreesitterTomlModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterTomlModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-toml")
    }

    fn name(&self) -> &'static str {
        "Treesitter TOML"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(TomlSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("toml", "TOML")
                .with_extensions(["toml"])
                .with_comments(CommentTokens::line_only("#")),
        );

        tracing::info!("TreesitterTomlModule: registered TOML syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterTomlModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
