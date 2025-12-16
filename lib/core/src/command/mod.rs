pub mod terminal {
    pub use reovim_sys::terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType,
    };
}

mod context;

pub use context::CommandContext;

// Trait-based command system
pub mod builtin;
pub mod id;
pub mod registry;
pub mod traits;

pub use id::CommandId;
pub use registry::{CommandRegistry, RegistryError};
pub use traits::{
    CommandLineAction, CommandResult, CommandTrait, DeferredAction, ExecutionContext,
};
