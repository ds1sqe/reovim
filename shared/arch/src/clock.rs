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
mod tests {
    use {super::*, std::sync::Arc};

    #[test]
    fn system_clock_advances() {
        let clock = SystemClock;
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t2 >= t1);
    }

    #[test]
    fn test_clock_stable_without_advance() {
        let clock = TestClock::new();
        let t1 = clock.now();
        let t2 = clock.now();
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_clock_advances_by_duration() {
        let clock = TestClock::new();
        let t1 = clock.now();
        clock.advance(Duration::from_millis(500));
        let t2 = clock.now();
        assert_eq!(t2 - t1, Duration::from_millis(500));
    }

    #[test]
    fn test_clock_cumulative_advance() {
        let clock = TestClock::new();
        let t1 = clock.now();
        clock.advance(Duration::from_millis(100));
        clock.advance(Duration::from_millis(200));
        let t2 = clock.now();
        assert_eq!(t2 - t1, Duration::from_millis(300));
    }

    #[test]
    fn test_clock_is_send_sync() {
        let clock = Arc::new(TestClock::new());
        let clone = Arc::clone(&clock);
        let handle = std::thread::spawn(move || {
            clone.advance(Duration::from_millis(100));
            clone.now()
        });
        let t_thread = handle.join().unwrap();
        let t_main = clock.now();
        assert_eq!(t_thread, t_main);
    }
}
