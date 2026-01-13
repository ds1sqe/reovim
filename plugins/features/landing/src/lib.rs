//! Landing page plugin with animated ASCII lion mascot
//!
//! Shows a responsive animated lion when no files are open:
//! - **Large** (50x24+): Roar animation - dramatic mouth opening with sound waves
//! - **Medium** (35x16+): Sleep animation - eyelids closing with zzZ
//! - **Small** (fallback): Breathing animation - subtle mane expansion
//!
//! # Usage
//!
//! The landing page appears automatically when the editor starts with no files.
//! Once a file is opened, the landing page is hidden.
//!
//! ```ignore
//! use reovim_plugin_landing::LandingPlugin;
//!
//! // Register the plugin
//! plugin_manager.register(Box::new(LandingPlugin::new()));
//! ```

pub mod frames;
pub mod sprite;
pub mod state;
pub mod window;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    event_bus::EventBus,
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
};

use {state::LandingState, window::LandingPluginWindow};

// Re-export key types
pub use {
    frames::LogoSize,
    sprite::{AnimationMode, AsciiSprite},
    state::{LandingState as State, generate},
};

/// Landing page plugin
///
/// Provides an animated ASCII lion mascot that displays when no files are open.
/// The animation adapts to terminal size with three responsive variants.
pub struct LandingPlugin;

impl Default for LandingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl LandingPlugin {
    /// Create a new landing plugin
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Plugin for LandingPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:landing")
    }

    fn name(&self) -> &'static str {
        "Landing"
    }

    fn description(&self) -> &'static str {
        "Animated ASCII lion landing page"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, _ctx: &mut PluginContext) {
        tracing::debug!("LandingPlugin: build complete");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Initialize with default dimensions (will be resized on first render)
        let landing = LandingState::new(80, 24);
        registry.register(landing);

        // Register the plugin window
        registry.register_plugin_window(Arc::new(LandingPluginWindow::new()));

        tracing::debug!("LandingPlugin: registered window and state");
    }

    fn subscribe(&self, _bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // No event subscriptions needed - landing renders when no files open
        tracing::debug!("LandingPlugin: subscribe complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let plugin = LandingPlugin::new();
        assert_eq!(plugin.name(), "Landing");
        assert_eq!(plugin.description(), "Animated ASCII lion landing page");
    }

    #[test]
    fn test_plugin_id() {
        let plugin = LandingPlugin::new();
        assert_eq!(plugin.id().as_str(), "reovim:landing");
    }

    #[test]
    fn test_plugin_default() {
        let plugin = LandingPlugin;
        assert_eq!(plugin.name(), "Landing");
    }
}
