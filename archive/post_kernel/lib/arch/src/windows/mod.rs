//! Windows platform implementation.
//!
//! Provides Windows Console API integration.
//!
//! **Note:** This is a stub implementation. Full Windows support will be
//! implemented in a future phase.

mod input;
pub mod local;
mod signal;
mod terminal;

pub use {
    input::WindowsInputSource,
    local::{WindowsLocalListener, WindowsLocalStream, process_exists},
    signal::WindowsSignalHandler,
    terminal::WindowsTerminal,
};
