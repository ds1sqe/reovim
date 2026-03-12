#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! TypeScript syntax highlighting module for reovim.
//!
//! Provides TypeScript and TSX language support for syntax highlighting using
//! tree-sitter-typescript and the `reovim-driver-syntax-treesitter` driver.
//!
//! This module supports both `"typescript"` and `"typescriptreact"` language
//! IDs, using `LANGUAGE_TYPESCRIPT` and `LANGUAGE_TSX` grammars respectively.
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_typescript::TypeScriptSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = TypeScriptSyntaxFactory::new();
//! let mut driver = factory.create("typescript").expect("TypeScript is supported");
//!
//! driver.parse("function hello(): string { return \"hi\"; }");
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

/// TypeScript highlights query (embedded from queries/highlights.scm)
const TS_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// TypeScript folds query (embedded from queries/folds.scm)
const TS_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// Factory for creating TypeScript and TSX syntax drivers.
///
/// Supports both `"typescript"` (`.ts`) and `"typescriptreact"` (`.tsx`)
/// language IDs, using their respective tree-sitter grammars.
// Dual-grammar factory needs `_ts`/`_tsx` suffixes to distinguish fields
#[allow(clippy::struct_field_names)]
pub struct TypeScriptSyntaxFactory {
    /// Pre-compiled highlights query for TypeScript
    highlight_query_ts: Arc<Query>,
    /// Pre-compiled folds query for TypeScript
    folds_query_ts: Arc<Query>,
    /// Pre-compiled highlights query for TSX
    highlight_query_tsx: Arc<Query>,
    /// Pre-compiled folds query for TSX
    folds_query_tsx: Arc<Query>,
}

impl TypeScriptSyntaxFactory {
    /// Create a new TypeScript syntax factory.
    ///
    /// Pre-compiles queries for both TypeScript and TSX grammars.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    #[must_use]
    pub fn new() -> Self {
        let lang_typescript: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        let lang_tsx: Language = tree_sitter_typescript::LANGUAGE_TSX.into();

        let highlight_query_ts = Query::new(&lang_typescript, TS_HIGHLIGHTS_QUERY)
            .expect("Failed to compile TypeScript highlights query");
        let folds_query_ts = Query::new(&lang_typescript, TS_FOLDS_QUERY)
            .expect("Failed to compile TypeScript folds query");

        let highlight_query_tsx = Query::new(&lang_tsx, TS_HIGHLIGHTS_QUERY)
            .expect("Failed to compile TSX highlights query");
        let folds_query_tsx =
            Query::new(&lang_tsx, TS_FOLDS_QUERY).expect("Failed to compile TSX folds query");

        Self {
            highlight_query_ts: Arc::new(highlight_query_ts),
            folds_query_ts: Arc::new(folds_query_ts),
            highlight_query_tsx: Arc::new(highlight_query_tsx),
            folds_query_tsx: Arc::new(folds_query_tsx),
        }
    }

    /// Get the shared folds query for TypeScript.
    #[must_use]
    pub const fn folds_query_ts(&self) -> &Arc<Query> {
        &self.folds_query_ts
    }

    /// Get the shared folds query for TSX.
    #[must_use]
    pub const fn folds_query_tsx(&self) -> &Arc<Query> {
        &self.folds_query_tsx
    }
}

impl Default for TypeScriptSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for TypeScriptSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        match language_id {
            "typescript" => {
                let language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
                TreeSitterDriver::with_queries(
                    "typescript",
                    &language,
                    self.highlight_query_ts.clone(),
                    Some(self.folds_query_ts.clone()),
                    None, // No injections
                    None, // No indents
                )
                .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
            }
            "typescriptreact" => {
                let language: Language = tree_sitter_typescript::LANGUAGE_TSX.into();
                TreeSitterDriver::with_queries(
                    "typescriptreact",
                    &language,
                    self.highlight_query_tsx.clone(),
                    Some(self.folds_query_tsx.clone()),
                    None, // No injections
                    None, // No indents
                )
                .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
            }
            _ => None,
        }
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["typescript", "typescriptreact"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "typescript" || language_id == "typescriptreact"
    }
}

// ============================================================================
// Module Implementation
// ============================================================================

/// Treesitter TypeScript syntax module.
///
/// Registers support for both TypeScript (`.ts`) and TSX (`.tsx`) files.
pub struct TreesitterTypeScriptModule;

impl TreesitterTypeScriptModule {
    /// Create a new Treesitter TypeScript module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterTypeScriptModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterTypeScriptModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-typescript")
    }

    fn name(&self) -> &'static str {
        "Treesitter TypeScript"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(TypeScriptSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("typescript", "TypeScript")
                .with_extensions(["ts"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );
        lang_store.add(
            LanguageInfo::new("typescriptreact", "TypeScript React")
                .with_extensions(["tsx"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        tracing::info!("TreesitterTypeScriptModule: registered TypeScript/TSX syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterTypeScriptModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
