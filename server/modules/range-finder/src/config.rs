//! Parent mode configuration for range-finder's jump-input mode.
//!
//! The adapter module (e.g., `vim-range-finder`) resolves the parent mode
//! from the editor personality and registers it here before range-finder
//! initializes. This decouples range-finder from any specific editor
//! personality — it never hardcodes mode names like `"vim:normal"`.

use reovim_kernel::api::v1::{ModeId, Service};

/// Parent mode for jump-input mode inheritance.
///
/// Registered in [`ServiceRegistry`] by the adapter module before
/// range-finder's `init()`. Range-finder reads this to configure
/// its jump-input mode's `inherits_from` and resolver parent.
pub struct JumpParentMode(ModeId);

impl JumpParentMode {
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

impl Service for JumpParentMode {}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
