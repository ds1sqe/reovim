//! Registries for modes, commands, keymaps, session handlers, and providers.
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
//! - [`DefaultModeProviderRegistry`]: Collects default mode providers
//!
//! # Note
//!
//! VFS providers now use `ServiceRegistry` with typed keys (Epic #417).
//! See `reovim_driver_vfs::VfsProviderRegistry`.

mod command;
mod empty_session;
mod keymap;
mod mode;
mod overlay_content;
mod provider;
mod which_key;

pub use {
    command::{CommandQuerySnapshot, CommandRegistry},
    empty_session::EmptySessionHandlerRegistry,
    keymap::KeymapRegistry,
    mode::{ModeEntry, ModeRegistry},
    overlay_content::RunnerOverlayContent,
    provider::DefaultModeProviderRegistry,
    which_key::WhichKeySaturatorSender,
};

// Re-export KeyLookupResult from driver (moved from keymap.rs in Epic #353)
pub use reovim_driver_input::KeyLookupResult;
