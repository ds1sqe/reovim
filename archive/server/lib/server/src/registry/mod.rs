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
//! - [`KeymapRegistry`]: Maps (mode, input sequence) to command IDs

mod command;
pub(crate) mod keymap;
mod mode;

pub use {
    command::{CommandQuerySnapshot, CommandRegistry},
    keymap::KeymapRegistry,
    mode::{ModeEntry, ModeRegistry},
};

// Re-export generic lookup primitives from subsys-input.
pub use reovim_subsys_input::{BindingInfo, BindingLayer, KeymapQuery, LookupResult, LookupState};
