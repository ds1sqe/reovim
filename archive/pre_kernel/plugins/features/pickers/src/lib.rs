//! Built-in pickers plugin for reovim
//!
//! This plugin provides all built-in pickers for the Microscope fuzzy finder:
//! - Files picker (find project files)
//! - Buffers picker (switch between open buffers)
//! - Grep picker (live grep with ripgrep)
//! - Commands picker (command palette)
//! - Recent picker (recently opened files)
//! - Themes picker (colorscheme selector)
//! - Help picker (help tags)
//! - Keymaps picker (view keybindings)
//! - Profiles picker (configuration profiles)
//!
//! These pickers are registered in the `PickerRegistry` when the plugin is loaded.

mod buffers;
mod commands;
mod files;
mod grep;
mod help;
mod keymaps;
mod profiles;
mod recent;
mod syntax_helper;
mod themes;

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    reovim_plugin_microscope::PickerRegistry,
};

// Re-export picker types
pub use {
    buffers::BuffersPicker,
    commands::{CommandInfo, CommandsPicker},
    files::FilesPicker,
    grep::GrepPicker,
    help::{HelpEntry, HelpPicker},
    keymaps::{KeymapEntry, KeymapsPicker},
    profiles::ProfilesPicker,
    recent::RecentPicker,
    themes::ThemesPicker,
};

/// Plugin that registers all built-in pickers
pub struct PickersPlugin;

impl Plugin for PickersPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:pickers")
    }

    fn name(&self) -> &'static str {
        "Built-in Pickers"
    }

    fn description(&self) -> &'static str {
        "Files, buffers, grep, commands, and more pickers"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        // Depends on MicroscopePlugin for PickerRegistry
        vec![TypeId::of::<reovim_plugin_microscope::MicroscopePlugin>()]
    }

    fn build(&self, _ctx: &mut PluginContext) {
        // No commands to register - pickers are data providers
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Ensure PickerRegistry exists (created by MicroscopePlugin, but register if not)
        if !registry.contains::<PickerRegistry>() {
            registry.register(PickerRegistry::new());
        }

        // Register all built-in pickers
        registry.with_mut::<PickerRegistry, _, _>(|picker_registry| {
            picker_registry.register(Arc::new(FilesPicker::new()));
            picker_registry.register(Arc::new(BuffersPicker::new()));
            picker_registry.register(Arc::new(GrepPicker::new()));
            picker_registry.register(Arc::new(CommandsPicker::new()));
            picker_registry.register(Arc::new(RecentPicker::new()));
            picker_registry.register(Arc::new(ThemesPicker::new()));
            picker_registry.register(Arc::new(HelpPicker::new()));
            picker_registry.register(Arc::new(KeymapsPicker::new()));
            picker_registry.register(Arc::new(ProfilesPicker::new()));
        });
    }
}
