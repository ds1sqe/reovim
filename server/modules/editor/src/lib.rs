//! Core Editor Module for Reovim
//!
//! This module provides basic editor commands (cursor movement, text operations)
//! that form the foundation of vim-style editing.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (kernel/drivers): Mode/Command traits, registries
//! - **Policy** (this module): Command implementations
//!
//! # Components
//!
//! - [`command`]: Cursor movement, delete, yank, paste, and other text operations
//! - [`ResolverRegistry`]: Motion and text object resolvers (re-exported from driver)
//!
//! # Note
//!
//! Mode definitions and mode-specific commands (enter insert, exit to normal,
//! etc.) have been moved to `reovim_module_vim`. This module provides
//! policy-agnostic editor commands; mode identities and transitions are in vim.
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_editor::command::{CursorDown, DeleteLine};
//!
//! // Register editor commands in your application
//! for cmd in reovim_module_editor::command::all_commands() {
//!     command_registry.register(cmd);
//! }
//! ```

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub mod command;
pub mod display_lines;
pub mod ids;
pub mod resolver;

pub use resolver::ResolverRegistry;

/// Module identifier for editor module.
pub const EDITOR_MODULE: ModuleId = ids::MODULE;

/// Editor module instance.
///
/// Provides core editor commands: cursor movement, text operations,
/// undo/redo, yank/paste, etc. These commands are policy-agnostic;
/// mode-specific behavior (vim modes) is in the vim module.
pub struct EditorModule;

impl EditorModule {
    /// Create a new editor module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EditorModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EditorModule {
    fn id(&self) -> ModuleId {
        EDITOR_MODULE
    }

    fn name(&self) -> &'static str {
        "Editor"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Epic #417 Part 3: Self-register commands
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl CommandProvider for EditorModule {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        command::all_commands()
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(EditorModule);
