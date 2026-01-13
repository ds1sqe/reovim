//! Lightweight profiling support.
//!
//! Linux equivalent: `kernel/trace/ring_buffer.c`
//!
//! Provides RAII-based profiling for measuring hot paths.

use std::time::Instant;

use super::metrics;

/// RAII guard for timing a scope.
///
/// Records the elapsed time to a histogram when dropped.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::debug::ProfileGuard;
///
/// fn expensive_operation() {
///     let _guard = ProfileGuard::new("expensive_operation");
///     // ... do work ...
/// } // time recorded to histogram when guard drops
/// ```
pub struct ProfileGuard {
    name: &'static str,
    start: Instant,
}

impl ProfileGuard {
    /// Create a new profile guard that will record to the named histogram.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for ProfileGuard {
    #[allow(clippy::cast_possible_truncation)] // Microsecond truncation acceptable
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let histogram = metrics().histogram(self.name);
        histogram.record(elapsed.as_micros() as u64);
    }
}

/// Profile a scope and record timing to a histogram.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::profile;
///
/// fn process_buffer() {
///     profile!("buffer_processing");
///     // ... processing code ...
/// }
/// ```
#[macro_export]
macro_rules! profile {
    ($name:expr) => {
        let _guard = $crate::debug::ProfileGuard::new($name);
    };
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{thread, time::Duration},
    };

    #[test]
    fn test_profile_guard_basic() {
        // Create guard and let it drop
        {
            let _guard = ProfileGuard::new("test_profile");
            // Simulate some work
            thread::sleep(Duration::from_millis(1));
        }

        // Check that histogram was updated
        let snapshot = metrics().snapshot();
        assert!(snapshot.histograms.contains_key("test_profile"));

        let (count, _mean) = snapshot.histograms.get("test_profile").unwrap();
        assert_eq!(*count, 1);
    }

    #[test]
    fn test_profile_guard_multiple() {
        let name = "test_profile_multi";

        for _ in 0..5 {
            let _guard = ProfileGuard::new(name);
        }

        let snapshot = metrics().snapshot();
        let (count, _mean) = snapshot.histograms.get(name).unwrap();
        assert_eq!(*count, 5);
    }
}
