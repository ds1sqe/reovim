//! Parent mode configuration for snippet's navigating mode.
//!
//! The adapter module (e.g., `vim-snippet`) resolves the parent mode
//! from the editor personality and registers it here before snippet
//! initializes. This decouples snippet from any specific editor
//! personality — it never hardcodes mode names like `"vim:insert"`.

use reovim_kernel::api::v1::{ModeId, Service};

/// Parent mode for snippet navigating mode inheritance.
///
/// Registered in [`ServiceRegistry`] by the adapter module before
/// snippet's `init()`. Snippet reads this to configure its navigating
/// mode's `inherits_from`, resolver parent, and command return mode.
pub struct SnippetParentMode(ModeId);

impl SnippetParentMode {
    /// Create a new parent mode configuration.
    #[must_use]
    pub const fn new(mode: ModeId) -> Self {
        Self(mode)
    }

    /// Get the parent mode ID.
    #[must_use]
    pub const fn mode(&self) -> &ModeId {
        &self.0
    }
}

impl Service for SnippetParentMode {}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
