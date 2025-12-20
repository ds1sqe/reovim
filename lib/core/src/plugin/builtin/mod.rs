//! Built-in plugins that provide core editor functionality
//!
//! These plugins are loaded by default and provide the essential
//! editor features.

mod completion;
mod core;
mod explorer;
mod fold;
mod leap;
mod python;
mod settings;
mod telescope;
mod window;

pub use {
    self::core::CorePlugin, completion::CompletionPlugin, explorer::ExplorerPlugin,
    fold::FoldPlugin, leap::LeapPlugin, python::PythonPlugin, settings::SettingsPlugin,
    telescope::TelescopePlugin, window::WindowPlugin,
};

use super::{PluginLoader, PluginTuple};

/// Default plugin set - equivalent to current functionality
///
/// This loads all built-in plugins in the correct dependency order.
///
/// # Example
///
/// ```ignore
/// let mut loader = PluginLoader::new();
/// loader.add_plugins(DefaultPlugins);
/// loader.load(&mut ctx).unwrap();
/// ```
pub struct DefaultPlugins;

impl PluginTuple for DefaultPlugins {
    fn add_to(self, loader: &mut PluginLoader) {
        // Core must be first - all others depend on it
        loader.add(CorePlugin);

        // Feature plugins (order doesn't matter due to dependency resolution)
        loader.add(CompletionPlugin);
        loader.add(TelescopePlugin);
        loader.add(ExplorerPlugin);
        loader.add(LeapPlugin);
        loader.add(FoldPlugin);
        loader.add(WindowPlugin);
        loader.add(SettingsPlugin);
    }
}
