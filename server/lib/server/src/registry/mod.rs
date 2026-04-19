//! Registries for modes, commands, and keymaps.
//!
//! These registries provide the lookup tables that the server uses
//! to dispatch key events to commands. They are the "mechanism" layer -
//! they store and retrieve, but don't define policy.
//!
//! # Registry Types
//!
//! - [`ModeRegistry`]: Stores mode metadata and behavior traits
//! - [`CommandRegistry`]: Stores command handlers by ID
//! - [`KeymapRegistry`]: Maps (mode, key sequence) to command IDs
//!
//! # Note
//!
//! These registries are ported from the runner's registry module for
//! the gRPC v2 server migration (Phase 9).

mod command;
mod keymap;
mod mode;

pub use {
    command::{CommandQuerySnapshot, CommandRegistry},
    keymap::KeymapRegistry,
    mode::{ModeEntry, ModeRegistry},
};

// Re-export KeyLookupResult from the shared input contracts.
pub use reovim_subsys_input_contracts::KeyLookupResult;
