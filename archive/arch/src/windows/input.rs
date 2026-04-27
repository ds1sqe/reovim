//! Windows input source stub implementation.
//!
//! This is a placeholder for future Windows support.

use std::{io, time::Duration};

use crate::traits::{InputSource, PlatformEvent};

/// Windows input source stub.
///
/// This is a placeholder implementation that will panic if used.
/// Full Windows support will be implemented in a future phase.
pub struct WindowsInputSource;

impl WindowsInputSource {
    /// Create a new Windows input source.
    ///
    /// # Panics
    ///
    /// Panics because Windows support is not yet implemented.
    pub fn new() -> io::Result<Self> {
        todo!("Windows input source support not yet implemented")
    }
}

impl Default for WindowsInputSource {
    fn default() -> Self {
        todo!("Windows input source support not yet implemented")
    }
}

impl InputSource for WindowsInputSource {
    fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
        todo!("Windows input source support not yet implemented")
    }

    fn read_event(&mut self) -> io::Result<PlatformEvent> {
        todo!("Windows input source support not yet implemented")
    }
}
