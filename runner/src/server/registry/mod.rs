//! Registries for modes, commands, keymaps, and session handlers.
//!
//! These registries provide the lookup tables that the event loop uses
//! to dispatch key events to commands. They are the "mechanism" layer -
//! they store and retrieve, but don't define policy.
//!
//! # Registry Types
//!
//! - [`ModeRegistry`]: Stores mode metadata and behavior traits
//! - [`CommandRegistry`]: Stores command handlers by ID
//! - [`KeymapRegistry`]: Maps (mode, key sequence) to command IDs
//! - [`EmptySessionHandlerRegistry`]: Handles empty session state

mod command;
mod empty_session;
mod keymap;
mod mode;

pub use {
    command::CommandRegistry,
    empty_session::EmptySessionHandlerRegistry,
    keymap::KeymapRegistry,
    mode::{ModeEntry, ModeRegistry},
};

// Re-export KeyLookupResult from driver (moved from keymap.rs in Epic #353)
pub use reovim_driver_input::KeyLookupResult;
