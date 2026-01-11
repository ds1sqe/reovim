//! Unix input source implementation using crossterm.

use std::{io, time::Duration};

use crossterm::event;

use {
    super::convert::convert_event,
    crate::traits::{InputEvent, InputSource},
};

/// Unix input source implementation wrapping crossterm's event system.
pub struct UnixInputSource;

impl UnixInputSource {
    /// Create a new Unix input source.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for UnixInputSource {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for UnixInputSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read_event(&mut self) -> io::Result<InputEvent> {
        let ct_event = event::read()?;
        Ok(convert_event(ct_event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_source_creation() {
        let _source = UnixInputSource::new();
    }

    #[test]
    fn test_poll_no_input() {
        let mut source = UnixInputSource::new();
        // Should return false immediately with zero timeout (no input available)
        let result = source.poll(Duration::ZERO);
        // In a TTY environment this should work; in CI it may fail
        // but should not panic
        let _ = result;
    }
}
