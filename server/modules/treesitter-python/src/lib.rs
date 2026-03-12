#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Python syntax highlighting module for reovim.
//!
//! Provides Python language support for syntax highlighting
//! using tree-sitter-python and the `reovim-driver-syntax-treesitter` driver.
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
//! reovim-module-treesitter-python  (THIS CRATE - Module + Python grammar)
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_python::PythonSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = PythonSyntaxFactory::new();
//! let mut driver = factory.create("python").expect("Python is supported");
//!
//! driver.parse("def hello():\n    print(\"hi\")");
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

/// Python highlights query (embedded from queries/highlights.scm)
const PYTHON_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Python folds query (embedded from queries/folds.scm)
const PYTHON_FOLDS_QUERY: &str = include_str!("queries/folds.scm");

/// Factory for creating Python syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Python syntax highlighting and fold detection using the tree-sitter-python grammar.
pub struct PythonSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled folds query
    folds_query: Arc<Query>,
}

impl PythonSyntaxFactory {
    /// Create a new Python syntax factory.
    ///
    /// Pre-compiles the highlights and folds queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    /// This should never happen with correctly bundled queries.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_python::LANGUAGE.into();

        let highlight_query = Query::new(&language, PYTHON_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Python highlights query");

        let folds_query = Query::new(&language, PYTHON_FOLDS_QUERY)
            .expect("Failed to compile Python folds query");

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

impl Default for PythonSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for PythonSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id != "python" {
            return None;
        }

        let language: Language = tree_sitter_python::LANGUAGE.into();

        TreeSitterDriver::with_queries(
            "python",
            &language,
            self.highlight_query.clone(),
            Some(self.folds_query.clone()),
            None, // No injections
            None, // No indents
        )
        .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["python"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "python"
    }
}

// ============================================================================
// Module Implementation (Self-Registration Pattern)
// ============================================================================

/// Treesitter Python syntax module.
///
/// Follows the self-registration pattern:
/// - Implements `Module` trait
/// - Registers `PythonSyntaxFactory` into `SyntaxFactoryStore` during `init()`
pub struct TreesitterPythonModule;

impl TreesitterPythonModule {
    /// Create a new Treesitter Python module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterPythonModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterPythonModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-python")
    }

    fn name(&self) -> &'static str {
        "Treesitter Python"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(PythonSyntaxFactory::new());

        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("python", "Python")
                .with_extensions(["py", "pyi", "pyw"])
                .with_mime_types(["text/x-python"])
                .with_comments(CommentTokens::line_only("#")),
        );

        tracing::info!("TreesitterPythonModule: registered Python syntax factory");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterPythonModule: exiting");
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
