//! Plugin system for modular, composable initialization
//!
//! This module provides a Bevy-inspired plugin architecture that allows
//! functionality to be organized into self-contained, reusable plugins.
//!
//! # Example
//!
//! ```ignore
//! use reovim_core::plugin::{Plugin, PluginContext, PluginId, PluginLoader, DefaultPlugins};
//!
//! // Create a custom plugin
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn id(&self) -> PluginId {
//!         PluginId::new("my:plugin")
//!     }
//!
//!     fn build(&self, ctx: &mut PluginContext) {
//!         ctx.register_command(MyCommand);
//!     }
//! }
//!
//! // Load plugins
//! let mut ctx = PluginContext::new();
//! let mut loader = PluginLoader::new();
//! loader.add_plugins((DefaultPlugins, MyPlugin));
//! loader.load(&mut ctx).unwrap();
//! ```

mod context;
mod editor_context;
mod loader;
pub mod macros;
mod runtime_context;
mod state;
mod statusline;
mod traits;
mod window;

pub mod builtin;

pub use {
    builtin::{CorePlugin, DefaultPlugins, PythonPlugin, WindowPlugin},
    context::{PluginContext, PluginOptionBuilder},
    editor_context::{EditorContext, PanelPosition},
    loader::{PluginError, PluginLoader, PluginTuple},
    runtime_context::{RuntimeContext, RuntimeContextBuilder},
    state::PluginStateRegistry,
    statusline::{
        RenderedSection, SectionAlignment, SharedStatuslineSectionProvider,
        StatuslineRenderContext, StatuslineSectionProvider,
    },
    traits::{Plugin, PluginId},
    window::{PluginWindow, Rect, WindowConfig},
};
