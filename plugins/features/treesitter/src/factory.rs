//! Syntax factory for tree-sitter based syntax highlighting
//!
//! This module provides the `TreesitterSyntaxFactory` which implements
//! the core `SyntaxFactory` trait to create syntax providers for files.

use std::sync::Arc;

use reovim_core::syntax::{SyntaxFactory, SyntaxProvider};

use crate::{SharedTreesitterManager, queries::QueryType, syntax::TreeSitterSyntax};

/// Factory for creating tree-sitter based syntax providers
///
/// Uses the treesitter manager's language registry and query cache
/// to create syntax providers for files.
pub struct TreesitterSyntaxFactory {
    manager: Arc<SharedTreesitterManager>,
}

impl TreesitterSyntaxFactory {
    /// Create a new syntax factory with access to the treesitter manager
    pub fn new(manager: Arc<SharedTreesitterManager>) -> Self {
        Self { manager }
    }
}

impl SyntaxFactory for TreesitterSyntaxFactory {
    fn create_syntax(
        &self,
        buffer_id: usize,
        file_path: &str,
        _content: &str,
    ) -> Option<Box<dyn SyntaxProvider>> {
        self.manager.with(|manager| {
            // Detect language from file extension
            let language_id = manager.registry().detect_language(file_path)?;

            // Get the registered language
            let registered = manager.registry().get(&language_id)?;

            // Get or compile highlights query (lazy compilation)
            let query = manager.get_cached_query(&language_id, QueryType::Highlights)?;

            // Check for injection query (lazy compilation)
            let injection_query = manager.get_cached_query(&language_id, QueryType::Injections);

            // Create syntax provider - use injection-aware version if available
            // Always pass manager for tree syncing to context provider
            let syntax = TreeSitterSyntax::with_injections(
                buffer_id,
                registered.language(),
                &language_id,
                query,
                injection_query,
                Arc::clone(&self.manager),
            )?;

            Some(Box::new(syntax) as Box<dyn SyntaxProvider>)
        })
    }

    fn supports_file(&self, file_path: &str) -> bool {
        self.manager
            .with(|manager| manager.registry().detect_language(file_path).is_some())
    }
}
