//! `BootClock` — the per-boot time anchor (7.5 §4).
//!
//! Captures the monotonic zero and the CLOCK\_REALTIME wall-clock reading at
//! the instant `Init::new` is called (boot stage 0). All LOG2 timestamps are
//! re-based to this anchor so they are human-readable relative to boot rather
//! than the unspecified monotonic epoch.

use reovim_arch::{
    sys::Timespec,
    time::{Instant, realtime},
};

/// Boot-time clock anchor (7.5 §4).
///
/// Created once in `Init::new` (boot stage 0) and transferred into `Kernel`
/// unchanged at the handoff so timestamps and ring content are continuous
/// across the boot/steady-state boundary.
///
/// Two fields:
///
/// - `mono_zero`: the `CLOCK_MONOTONIC` reading at `Init::new`. Subtract this
///   from a later `Instant` to obtain a boot-relative monotonic duration.
/// - `wall_anchor`: the `CLOCK_REALTIME` reading at the same instant, used to
///   derive an absolute wall-clock time for the boot origin (LOG2 §2 first
///   line, human audit use).
///
/// # Example
///
/// ```rust
/// use reovim_kernel::BootClock;
///
/// let clock = BootClock::capture();
/// // Elapsed nanoseconds since boot is always non-negative.
/// let elapsed = clock.elapsed_nanos();
/// assert!(elapsed < u64::MAX);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BootClock {
    /// Monotonic reading at boot-stage-0 — the re-base zero for LOG2 timestamps.
    pub mono_zero: Instant,
    /// CLOCK\_REALTIME reading at boot-stage-0 — the wall-clock anchor (7.5 §4).
    pub wall_anchor: Timespec,
}

impl BootClock {
    /// Captures the current monotonic and wall-clock readings as the boot
    /// anchor.
    ///
    /// Called once in `Init::new`. The two clocks are read back-to-back with
    /// no synchronization between them; a sub-millisecond skew is acceptable
    /// for the LOG2 audit use case.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::BootClock;
    ///
    /// let clock = BootClock::capture();
    /// // Wall anchor is after the Unix epoch (past year 2000).
    /// assert!(clock.wall_anchor.tv_sec > 946_684_800);
    /// ```
    #[must_use]
    pub fn capture() -> Self {
        Self {
            mono_zero: Instant::now(),
            wall_anchor: realtime(),
        }
    }

    /// Nanoseconds elapsed on `CLOCK_MONOTONIC` since the boot anchor.
    ///
    /// Returns `0` when the current monotonic time is at or before
    /// `mono_zero` (clock did not advance, or `capture()` was called after
    /// the query — unreachable in practice).
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::BootClock;
    ///
    /// let clock = BootClock::capture();
    /// let elapsed = clock.elapsed_nanos();
    /// // Non-negative; cannot overflow a u64 in any realistic uptime.
    /// assert!(elapsed < u64::MAX);
    /// ```
    #[must_use]
    pub fn elapsed_nanos(&self) -> u64 {
        Instant::now().elapsed_nanos(self.mono_zero)
    }

    /// Microseconds elapsed since the boot anchor (truncated, not rounded).
    ///
    /// Convenience wrapper for LOG2 timestamp formatting (9.5 §2 requires
    /// microseconds, zero-padded to width 6).
    ///
    /// # Example
    ///
    /// ```rust
    /// use reovim_kernel::BootClock;
    ///
    /// let clock = BootClock::capture();
    /// let us = clock.elapsed_micros();
    /// assert!(us < u64::MAX);
    /// ```
    #[must_use]
    pub fn elapsed_micros(&self) -> u64 {
        self.elapsed_nanos() / 1_000
    }
}

// L12 layout: tests in sibling clock_tests.rs, declared in lib.rs.
