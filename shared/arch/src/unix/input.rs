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

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for UnixInputSource {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for UnixInputSource {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[test]
    fn test_input_source_default() {
        fn assert_default<T: Default>() {}
        assert_default::<UnixInputSource>();
    }

    #[test]
    fn test_input_source_drain_empty() {
        let mut source = UnixInputSource::new();
        // drain() is the default method on InputSource trait
        // In CI, there should be no pending events, so drain returns empty
        let events = source.drain();
        // We can only assert it doesn't panic; events may or may not be empty
        // depending on the test environment
        let _ = events;
    }
}
