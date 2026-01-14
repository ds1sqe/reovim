//! Registries for modes, commands, and keymaps.
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

mod command;
mod keymap;
mod mode;

pub use {
    command::CommandRegistry,
    keymap::{KeyLookupResult, KeymapRegistry},
    mode::{ModeEntry, ModeRegistry},
};
