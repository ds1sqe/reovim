//! Clock abstraction for deterministic time testing.
//!
//! Provides a `Clock` trait so components that depend on elapsed time
//! can be tested without real sleeps.
//!
//! - [`SystemClock`]: Production implementation using `Instant::now()`.
//! - [`TestClock`]: Manual time control via `advance()` for deterministic tests.

use std::time::{Duration, Instant};

use crate::sync::Mutex;

/// A monotonic clock source.
pub trait Clock: Send + Sync {
    /// Returns the current instant.
    fn now(&self) -> Instant;
}

/// Production clock using `std::time::Instant::now()`.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Manually-controlled clock for deterministic testing.
///
/// Time only advances when `advance()` is called explicitly.
/// This allows tests to verify timeout behavior without real sleeps.
pub struct TestClock {
    current: Mutex<Instant>,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::new()
    }
}

impl TestClock {
    /// Creates a new `TestClock` anchored at `Instant::now()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Mutex::new(Instant::now()),
        }
    }

    /// Advances the clock by the given duration.
    pub fn advance(&self, duration: Duration) {
        let mut current = self.current.lock();
        *current += duration;
    }
}

impl Clock for TestClock {
    fn now(&self) -> Instant {
        *self.current.lock()
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
