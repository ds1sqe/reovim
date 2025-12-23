//! Built-in plugins that provide core editor functionality
//!
//! These plugins are loaded by default and provide the essential
//! editor features.
//!
//! Note: `FoldPlugin` has been moved to the `reovim-plugin-fold` crate.
//! Note: `SettingsPlugin` has been moved to the `reovim-plugin-settings-menu` crate.
//! Note: `CompletionPlugin` has been moved to the `reovim-plugin-completion` crate.
//! Note: `ExplorerPlugin` has been moved to the `reovim-plugin-explorer` crate.
//! Note: `TelescopePlugin` has been moved to the `reovim-plugin-telescope` crate.
//! Note: `LeapPlugin` has been moved to the `reovim-plugin-leap` crate.

mod core;
mod python;
mod window;

pub use {self::core::CorePlugin, python::PythonPlugin, window::WindowPlugin};

use super::{PluginLoader, PluginTuple};

/// Default plugin set - equivalent to current functionality
///
/// This loads all built-in plugins in the correct dependency order.
/// Note: Feature plugins are now in separate crates.
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

        // Feature plugins
        loader.add(WindowPlugin);
        // Note: LeapPlugin is now in reovim-plugin-leap crate
        // Note: CompletionPlugin is now in reovim-plugin-completion crate
        // Note: SettingsPlugin is now in reovim-plugin-settings-menu crate
        // Note: ExplorerPlugin is now in reovim-plugin-explorer crate
        // Note: TelescopePlugin is now in reovim-plugin-telescope crate
    }
}
