//! Python language support plugin (example)
//!
//! This is an example of a language-specific plugin that demonstrates
//! how to create custom plugins for reovim.

use std::any::TypeId;

use crate::plugin::{Plugin, PluginContext, PluginId};

use super::CorePlugin;

/// Python language support plugin
///
/// Example plugin demonstrating language-specific functionality.
/// This could be extended to include:
/// - Python-specific commands (run file, REPL, etc.)
/// - Python-specific keybindings
/// - Python-specific modifiers (e.g., virtual env indicator)
///
/// # Example
///
/// ```ignore
/// use reovim_core::plugin::{DefaultPlugins, PythonPlugin, PluginLoader};
///
/// let mut loader = PluginLoader::new();
/// loader.add_plugins((DefaultPlugins, PythonPlugin));
/// loader.load(&mut ctx).unwrap();
/// ```
pub struct PythonPlugin;

impl Plugin for PythonPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:python")
    }

    fn name(&self) -> &'static str {
        "Python"
    }

    fn description(&self) -> &'static str {
        "Python language support"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<CorePlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Example: Register Python-specific commands
        self.register_python_commands(ctx);

        // Example: Register Python-specific keybindings
        self.register_python_keybindings(ctx);
    }
}

#[allow(clippy::unused_self, clippy::missing_const_for_fn)]
impl PythonPlugin {
    fn register_python_commands(&self, _ctx: &PluginContext) {
        // Future: Register commands like:
        // - PythonRunFileCommand
        // - PythonStartReplCommand
        // - PythonFormatBufferCommand
        // - PythonLintCommand
        //
        // Example:
        // let _ = ctx.register_command(PythonRunFileCommand);
    }

    fn register_python_keybindings(&self, _ctx: &mut PluginContext) {
        // Future: Bind Python-specific keys
        // These would only be active when editing Python files
        // (requires filetype-aware keymaps, future enhancement)
        //
        // Example:
        // ctx.bind_key("normal", " pr", CommandRef::Registered(PYTHON_RUN_FILE));
        // ctx.bind_key("normal", " pi", CommandRef::Registered(PYTHON_START_REPL));
    }
}
