//! Ex-command registry re-export.
//!
//! This module re-exports `ExCommandRegistry` from driver-command for
//! convenience. The server uses this to create and register the registry.

pub use reovim_driver_command::ExCommandRegistry;
