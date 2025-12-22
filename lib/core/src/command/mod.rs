pub mod terminal {
    pub use reovim_sys::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode};
}

mod context;

pub use context::CommandContext;

// Trait-based command system
pub mod builtin;
pub mod id;
pub mod registry;
pub mod traits;

pub use {
    id::CommandId,
    registry::{CommandRegistry, RegistryError},
    traits::{
        CommandLineAction, CommandResult, CommandTrait, DeferredAction, DeferredActionHandler,
        ExecutionContext, FileAction,
    },
};
