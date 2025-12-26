//! Plugin configuration for reovim
//!
//! This module defines which plugins are loaded and their configuration.
//! Users can modify this to customize which features are available.

#![allow(dead_code)] // Functions will be used once runtime decoupling is complete

use reovim_core::plugin::{
    CorePlugin, DefaultPlugins, Plugin, PluginLoader, PluginTuple, WindowPlugin,
};

// External plugin crates
use {
    reovim_plugin_completion::CompletionPlugin, reovim_plugin_explorer::ExplorerPlugin,
    reovim_plugin_fold::FoldPlugin, reovim_plugin_leap::LeapPlugin, reovim_plugin_lsp::LspPlugin,
    reovim_plugin_microscope::MicroscopePlugin, reovim_plugin_pair::PairPlugin,
    reovim_plugin_pickers::PickersPlugin, reovim_plugin_settings_menu::SettingsMenuPlugin,
    reovim_plugin_treesitter::TreesitterPlugin,
};

// Language plugins
use {
    reovim_lang_bash::BashPlugin, reovim_lang_c::CPlugin, reovim_lang_javascript::JavaScriptPlugin,
    reovim_lang_json::JsonPlugin, reovim_lang_markdown::MarkdownPlugin,
    reovim_lang_python::PythonPlugin, reovim_lang_rust::RustPlugin, reovim_lang_toml::TomlPlugin,
};

/// All plugins including both built-in (`DefaultPlugins`) and external plugin crates
///
/// This is the complete set of plugins for a standard reovim experience.
pub struct AllPlugins;

impl PluginTuple for AllPlugins {
    fn add_to(self, loader: &mut PluginLoader) {
        // Add built-in plugins (CorePlugin, UIComponentsPlugin, WindowPlugin)
        loader.add_plugins(DefaultPlugins);

        // Add external plugin crates
        loader.add(LeapPlugin);
        loader.add(FoldPlugin::new());
        loader.add(PairPlugin::new());
        loader.add(SettingsMenuPlugin);
        loader.add(CompletionPlugin::new());
        loader.add(ExplorerPlugin);
        loader.add(MicroscopePlugin);
        loader.add(PickersPlugin); // Must come after MicroscopePlugin

        // Treesitter infrastructure (must come before language plugins)
        loader.add(TreesitterPlugin::new());

        // Language plugins
        loader.add(RustPlugin);
        loader.add(CPlugin);
        loader.add(JavaScriptPlugin);
        loader.add(PythonPlugin);
        loader.add(JsonPlugin);
        loader.add(TomlPlugin);
        loader.add(MarkdownPlugin);
        loader.add(BashPlugin);

        // LSP integration (after language plugins)
        loader.add(LspPlugin::new());
    }
}

/// Create the default set of plugins
///
/// This configures the standard reovim experience with all built-in features.
/// Modify this function to customize which plugins are loaded.
#[must_use]
pub fn default_plugins() -> impl IntoIterator<Item = Box<dyn Plugin>> {
    vec![
        // Core functionality (required)
        Box::new(CorePlugin) as Box<dyn Plugin>,
        // Editor features
        Box::new(WindowPlugin),
        Box::new(LeapPlugin),
        Box::new(MicroscopePlugin),
        Box::new(PickersPlugin), // Must come after MicroscopePlugin
        Box::new(CompletionPlugin::new()),
        Box::new(ExplorerPlugin),
        Box::new(FoldPlugin::new()),
        Box::new(PairPlugin::new()),
        Box::new(SettingsMenuPlugin),
        // Treesitter infrastructure (must come before language plugins)
        Box::new(TreesitterPlugin::new()),
        // Language support
        Box::new(RustPlugin),
        Box::new(CPlugin),
        Box::new(JavaScriptPlugin),
        Box::new(PythonPlugin),
        Box::new(JsonPlugin),
        Box::new(TomlPlugin),
        Box::new(MarkdownPlugin),
        Box::new(BashPlugin),
        // LSP integration
        Box::new(LspPlugin::new()),
    ]
}

/// Create a minimal set of plugins for testing
///
/// Only includes core functionality without extra features.
#[must_use]
#[allow(dead_code)]
pub fn minimal_plugins() -> impl IntoIterator<Item = Box<dyn Plugin>> {
    vec![Box::new(CorePlugin) as Box<dyn Plugin>]
}

/// Create a plugin loader with the default plugins
#[must_use]
pub fn create_plugin_loader() -> PluginLoader {
    let mut loader = PluginLoader::new();
    loader.add_plugins(DefaultPlugins);
    // Add external plugin crates
    loader.add(LeapPlugin);
    loader.add(FoldPlugin::new());
    loader.add(PairPlugin::new());
    loader.add(SettingsMenuPlugin);
    loader.add(CompletionPlugin::new());
    loader.add(ExplorerPlugin);
    loader.add(MicroscopePlugin);
    loader.add(PickersPlugin); // Must come after MicroscopePlugin
    // Treesitter infrastructure
    loader.add(TreesitterPlugin::new());
    // Language plugins
    loader.add(RustPlugin);
    loader.add(CPlugin);
    loader.add(JavaScriptPlugin);
    loader.add(PythonPlugin);
    loader.add(JsonPlugin);
    loader.add(TomlPlugin);
    loader.add(MarkdownPlugin);
    loader.add(BashPlugin);
    // LSP integration
    loader.add(LspPlugin::new());
    loader
}

// Note: Custom plugin loading will be implemented when we have
// the full plugin extraction complete. For now, use DefaultPlugins
// or create_plugin_loader() which uses the built-in plugin set.
