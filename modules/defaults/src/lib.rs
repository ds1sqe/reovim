//! Default modules bundle - POLICY aggregator.
//!
//! This module bundles the standard vim-like behavior modules:
//! - `vim` - Vim keybindings (normal, insert, visual, operator-pending)
//! - `keymap` - Keymap utilities (mechanism)
//! - `operators` - Vim operators (d, y, c)
//! - `commands` - Ex-commands (:w, :q, :wq)
//!
//! # Purpose
//!
//! Provides a single entry point to load all default modules.
//! The runner can load this bundle to get a complete vim-like editor.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_defaults::DefaultsModule;
//!
//! let defaults = DefaultsModule::new();
//! let modules = defaults.create_modules();
//! for module in modules {
//!     registry.load(module)?;
//! }
//! ```

use reovim_kernel::api::v1::{
    EmptySessionHandlerRegistration, KeybindingRegistration, Module, ModuleContext, ModuleError,
    ModuleId, ProbeResult, Version,
};

// Import traits from their respective modules (mechanism vs policy)
use {reovim_module_commands::CommandHandler, reovim_module_operators::Operator};

// Re-export sub-modules for direct access
pub use {
    reovim_module_commands as commands, reovim_module_keymap as keymap,
    reovim_module_operators as operators, reovim_module_scratch_buffer as scratch_buffer,
    reovim_module_vim as vim,
};

// Re-export driver trait for handler type
pub use reovim_driver_session::EmptySessionHandler;

/// Default modules bundle.
///
/// This module aggregates all default modules and provides their
/// keybindings, operators, and commands.
pub struct DefaultsModule;

impl DefaultsModule {
    /// Create a new defaults module bundle.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Create instances of all default modules.
    ///
    /// Returns a vector of boxed module instances that can be loaded
    /// into the module registry.
    #[must_use]
    pub fn create_modules() -> Vec<Box<dyn Module>> {
        vec![
            Box::new(keymap::KeymapModule),
            Box::new(operators::OperatorsModule),
            Box::new(commands::CommandsModule),
        ]
    }
}

impl Default for DefaultsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for DefaultsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("defaults")
    }

    fn name(&self) -> &'static str {
        "Default Modules Bundle"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        // This module depends on its sub-modules
        vec![
            ModuleId::new("keymap"),
            ModuleId::new("operators"),
            ModuleId::new("commands"),
            ModuleId::new("scratch-buffer"),
        ]
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        // Aggregate keybindings from vim module
        vim::VimModule::new().keybindings()
    }

    fn empty_session_handlers(&self) -> Vec<EmptySessionHandlerRegistration> {
        // Expose empty session handler registration from scratch-buffer
        // The runner will use empty_session_handler() to get the actual handler
        vec![
            EmptySessionHandlerRegistration::new("defaults:scratch-buffer")
                .with_description("Create empty scratch buffer on startup")
                .with_priority(100),
        ]
    }
}

/// Get all default operators.
#[must_use]
pub fn operators() -> Vec<Box<dyn Operator>> {
    operators::operators()
}

/// Get all default commands.
#[must_use]
pub fn commands() -> Vec<Box<dyn CommandHandler>> {
    commands::commands()
}

/// Get all default keybindings.
#[must_use]
pub fn keybindings() -> Vec<KeybindingRegistration> {
    vim::bindings::all()
}

/// Get the default empty session handler.
///
/// Returns `ScratchBufferHandler` which creates an empty buffer
/// when a session starts with no files. This centralizes the
/// default policy in the `defaults` module.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use reovim_module_defaults::empty_session_handler;
///
/// let handler = Arc::new(empty_session_handler());
/// registry.register(handler);
/// ```
#[must_use]
pub const fn empty_session_handler() -> scratch_buffer::ScratchBufferHandler {
    scratch_buffer::ScratchBufferHandler
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_module_id() {
        let module = DefaultsModule::new();
        assert_eq!(module.id().as_str(), "defaults");
    }

    #[test]
    fn test_defaults_module_name() {
        let module = DefaultsModule::new();
        assert_eq!(module.name(), "Default Modules Bundle");
    }

    #[test]
    fn test_defaults_has_dependencies() {
        let module = DefaultsModule::new();
        let deps = module.dependencies();
        // keymap, operators, commands, scratch-buffer
        assert_eq!(deps.len(), 4);
    }

    #[test]
    fn test_create_modules() {
        let modules = DefaultsModule::create_modules();
        assert_eq!(modules.len(), 3);
    }

    #[test]
    fn test_operators_not_empty() {
        let ops = operators();
        assert!(!ops.is_empty());
    }

    #[test]
    fn test_commands_not_empty() {
        let cmds = commands();
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_keybindings_not_empty() {
        let bindings = keybindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_defaults_has_empty_session_handlers() {
        let module = DefaultsModule::new();
        let handlers = module.empty_session_handlers();
        assert_eq!(handlers.len(), 1);
    }

    #[test]
    fn test_empty_session_handler_registration_id() {
        let module = DefaultsModule::new();
        let handlers = module.empty_session_handlers();
        assert_eq!(handlers[0].id, "defaults:scratch-buffer");
    }

    #[test]
    fn test_empty_session_handler_registration_priority() {
        let module = DefaultsModule::new();
        let handlers = module.empty_session_handlers();
        assert_eq!(handlers[0].priority, 100);
    }

    #[test]
    fn test_empty_session_handler_factory() {
        use {
            reovim_driver_session::{EmptySessionAction, EmptySessionContext, EmptySessionHandler},
            std::path::Path,
        };

        let handler = empty_session_handler();

        // Handler should create buffer when no files specified
        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &[],
            cwd: Path::new("/home/user"),
        };
        let action = handler.handle(&ctx);
        assert!(matches!(action, EmptySessionAction::CreateBuffer { .. }));
    }

    #[test]
    fn test_empty_session_handler_defers_when_files_specified() {
        use {
            reovim_driver_session::{EmptySessionAction, EmptySessionContext, EmptySessionHandler},
            std::path::Path,
        };

        let handler = empty_session_handler();
        let files = vec!["file.txt".to_string()];

        let ctx = EmptySessionContext {
            session_id: 1,
            file_args: &files,
            cwd: Path::new("/home/user"),
        };
        let action = handler.handle(&ctx);
        assert!(matches!(action, EmptySessionAction::None));
    }
}
