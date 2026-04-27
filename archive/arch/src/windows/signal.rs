//! Windows signal handler stub implementation.
//!
//! This is a placeholder for future Windows support.

use crate::traits::{SignalHandler, TerminalSize};

/// Windows signal handler stub.
///
/// This is a placeholder implementation that will panic if used.
/// Full Windows support will be implemented in a future phase.
pub struct WindowsSignalHandler;

impl WindowsSignalHandler {
    /// Create a new Windows signal handler.
    ///
    /// # Panics
    ///
    /// Panics because Windows support is not yet implemented.
    pub fn new() -> Self {
        todo!("Windows signal handler support not yet implemented")
    }
}

impl Default for WindowsSignalHandler {
    fn default() -> Self {
        todo!("Windows signal handler support not yet implemented")
    }
}

impl SignalHandler for WindowsSignalHandler {
    fn on_resize(&mut self, _handler: Box<dyn Fn(TerminalSize) + Send + Sync>) {
        todo!("Windows signal handler support not yet implemented")
    }

    fn on_interrupt(&mut self, _handler: Box<dyn Fn() + Send + Sync>) {
        todo!("Windows signal handler support not yet implemented")
    }

    fn on_suspend(&mut self, _handler: Box<dyn Fn() + Send + Sync>) {
        todo!("Windows signal handler support not yet implemented")
    }
}
