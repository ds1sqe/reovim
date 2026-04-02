//! Snippet expander trait for cross-module decoupling.
//!
//! Defines the [`SnippetExpander`] trait so the completion module can
//! trigger snippet expansion without depending on `reovim-module-snippet`.
//!
//! # Architecture (#542)
//!
//! This is mechanism: it defines WHAT operation is available (expanding a
//! snippet body into the buffer with tab-stop navigation). The
//! `reovim-module-snippet` provides the policy: HOW snippets are parsed,
//! expanded, and navigated.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{BufferId, Service},
    reovim_types_text::Position,
};

use crate::SessionRuntime;

/// Trait for expanding snippet bodies during completion confirm.
///
/// Implemented by `reovim-module-snippet`. The completion module invokes
/// this to insert snippet text and enter tab-stop navigation mode without
/// directly importing snippet module types.
pub trait SnippetExpander: Send + Sync {
    /// Expand a snippet body at the given position.
    ///
    /// Implementations should:
    /// 1. Parse the snippet body
    /// 2. Expand variables and compute tab stops
    /// 3. Insert the expanded text into the buffer
    /// 4. Store active snippet state in the session extension
    /// 5. Enter snippet navigation mode if tab stops exist
    /// 6. Position cursor at the first tab stop
    fn expand(
        &self,
        runtime: &mut SessionRuntime<'_>,
        buffer_id: BufferId,
        insert_pos: Position,
        snippet_body: &str,
    );
}

/// Registry for snippet expander implementations.
///
/// Allows modules to register a [`SnippetExpander`] implementation that
/// other modules can discover via `ServiceRegistry`.
pub struct SnippetExpanderRegistry {
    expander: parking_lot::RwLock<Option<Arc<dyn SnippetExpander>>>,
}

impl std::fmt::Debug for SnippetExpanderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnippetExpanderRegistry")
            .field("has_expander", &self.expander.read().is_some())
            .finish()
    }
}

impl SnippetExpanderRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            expander: parking_lot::RwLock::new(None),
        }
    }

    /// Register a snippet expander implementation.
    pub fn register(&self, expander: Arc<dyn SnippetExpander>) {
        *self.expander.write() = Some(expander);
    }

    /// Get the registered snippet expander implementation.
    #[must_use]
    pub fn get(&self) -> Option<Arc<dyn SnippetExpander>> {
        self.expander.read().clone()
    }
}

impl Default for SnippetExpanderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for SnippetExpanderRegistry {}
#[cfg(test)]
#[path = "snippet_expander_tests.rs"]
mod tests;
