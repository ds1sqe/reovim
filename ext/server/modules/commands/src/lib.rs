#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Ex-commands module - POLICY.
//!
//! This module implements the standard ex-commands (colon commands):
//! - `:q` / `:quit` - Quit the editor
//! - `:w` / `:write` - Write the buffer to disk
//! - `:wq` - Write and quit
//! - `:e` / `:edit` - Open a file
//! - `:set` - Set editor options
//! - `:colorscheme` - Switch color theme
//! - `:detach` - Detach from server
//! - `:servers` - List server instances
//! - `:kill-server` - Kill the server

mod buffer;
mod colorscheme;
mod edit;
mod quit;
mod session;
mod set;
mod substitute;
mod write;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub use {
    buffer::{BdeleteCommand, BnextCommand, BpreviousCommand, QuitAllCommand},
    colorscheme::ColorschemeCommand,
    edit::EditCommand,
    quit::QuitCommand,
    session::{DetachCommand, KillServerCommand, ServersCommand},
    set::SetCommand,
    substitute::SubstituteCommand,
    write::{WriteCommand, WriteQuitCommand},
};

/// Returns all command handlers provided by this module.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(EditCommand),
        Box::new(QuitCommand),
        Box::new(WriteCommand),
        Box::new(WriteQuitCommand),
        Box::new(ColorschemeCommand),
        Box::new(SetCommand),
        Box::new(SubstituteCommand),
    ];
    cmds.extend(session::command_handlers());
    cmds.extend(buffer::command_handlers());
    cmds
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Commands module instance.
pub struct CommandsModule;

impl CommandsModule {
    /// Create a new commands module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CommandsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CommandsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("commands")
    }

    fn name(&self) -> &'static str {
        "Ex-Commands"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register command handlers (#547)
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in command_handlers() {
            store.add(handler);
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_subsys_command::capabilities::COMMAND_DISPATCH]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CommandsModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
