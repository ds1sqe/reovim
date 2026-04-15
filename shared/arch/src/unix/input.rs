//! Unix input source implementation using crossterm.

use std::{io, time::Duration};

use crossterm::event;

use {
    super::convert::convert_event,
    crate::traits::{PlatformEvent, InputSource},
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
    fn read_event(&mut self) -> io::Result<PlatformEvent> {
        let ct_event = event::read()?;
        Ok(convert_event(ct_event))
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
