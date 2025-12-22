//! Plugin window provider system
//!
//! Allows plugins to register windows that should be rendered by the core

use std::sync::Arc;

use crate::screen::window::Window;

use super::PluginStateRegistry;

/// Trait for plugins that provide windows to be rendered
pub trait WindowProvider: Send + Sync {
    /// Get windows that should be rendered
    ///
    /// Returns a vector of windows. Return empty vector if no windows should be shown.
    fn get_windows(&self, state: &Arc<PluginStateRegistry>) -> Vec<Window>;
}
