//! UI Components Plugin
//!
//! Registers the built-in UI components (Editor, `CommandLine`)
//! and their focus input handlers.
//!
//! Plugin-specific components (Explorer, Telescope, Settings) are registered
//! by their respective plugins under plugins/features/.

use std::any::TypeId;

use crate::{
    interactor::{CommandLineInt, Editor},
    plugin::{Plugin, PluginContext, PluginId},
};

use super::CorePlugin;

/// Plugin that registers built-in UI components
///
/// This plugin registers the core UI components that handle user input:
/// - Editor: Main text editing area
/// - `CommandLine`: Command mode input
///
/// Plugin-specific components (Explorer, Telescope, Settings) are registered
/// by their respective plugins under plugins/features/.
pub struct UIComponentsPlugin;

impl Plugin for UIComponentsPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:ui_components")
    }

    fn name(&self) -> &'static str {
        "UI Components"
    }

    fn description(&self) -> &'static str {
        "Registers built-in UI components and their input handlers"
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register built-in UI components
        // These components route to FocusInput events for Runtime access
        ctx.register_component(Box::new(Editor));
        ctx.register_component(Box::new(CommandLineInt));
    }

    fn dependencies(&self) -> Vec<TypeId> {
        // Depends on CorePlugin for base command infrastructure
        vec![TypeId::of::<CorePlugin>()]
    }
}
