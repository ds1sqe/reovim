//! Windows platform implementation.
//!
//! Provides Windows Console API integration.
//!
//! **Note:** This is a stub implementation. Full Windows support will be
//! implemented in a future phase.

mod input;
mod signal;
mod terminal;

pub use {input::WindowsInputSource, signal::WindowsSignalHandler, terminal::WindowsTerminal};
