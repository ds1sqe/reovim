//! Shared state for the treesitter plugin
//!
//! Provides thread-safe access to the treesitter manager.

use std::sync::RwLock;

use reovim_core::textobject::{SemanticTextObjectSource, SemanticTextObjectSpec, TextObjectBounds};

use crate::{manager::TreesitterManager, queries::QueryType, text_objects::TextObjectResolver};

/// Thread-safe wrapper for TreesitterManager
///
/// Registered in PluginStateRegistry for cross-plugin access.
pub struct SharedTreesitterManager {
    inner: RwLock<TreesitterManager>,
}

impl SharedTreesitterManager {
    /// Create a new shared treesitter manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(TreesitterManager::new()),
        }
    }

    /// Access the manager immutably
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&TreesitterManager) -> R,
    {
        let guard = self.inner.read().unwrap();
        f(&guard)
    }

    /// Access the manager mutably
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut TreesitterManager) -> R,
    {
        let mut guard = self.inner.write().unwrap();
        f(&mut guard)
    }
}

impl Default for SharedTreesitterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticTextObjectSource for SharedTreesitterManager {
    fn find_bounds(
        &self,
        buffer_id: usize,
        content: &str,
        cursor_row: u32,
        cursor_col: u32,
        spec: &SemanticTextObjectSpec,
    ) -> Option<TextObjectBounds> {
        // Use immutable access since queries are pre-compiled
        self.with(|manager| {
            // Get the language ID for this buffer
            let language_id = manager.buffer_language(buffer_id)?;

            // Get the parse tree
            let tree = manager.get_tree(buffer_id)?;

            // Get the pre-compiled text objects query
            let query = manager.get_cached_query(&language_id, QueryType::TextObjects)?;

            // Resolve the text object bounds
            TextObjectResolver::find_bounds(tree, query, content, cursor_row, cursor_col, spec)
        })
    }
}
