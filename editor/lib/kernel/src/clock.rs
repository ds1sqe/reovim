//! `BootClock` — the per-boot time anchor (7.5 §4).
//!
//! Captures the monotonic zero and the CLOCK\_REALTIME wall-clock reading at
//! the instant `Init::new` is called (boot stage 0). All LOG2 timestamps are
//! re-based to this anchor so they are human-readable relative to boot rather
//! than the unspecified monotonic epoch.

use reovim_uapi::sched::ClockControl;

/// Boot-time clock anchor (7.5 §4).
///
/// Created once in `Init::new` (boot stage 0) and transferred into `Kernel`
/// unchanged at the handoff so timestamps and ring content are continuous
/// across the boot/steady-state boundary.
///
/// Both fields are whole nanoseconds read through the injected up-face clock
/// control table, so `BootClock` names no lower scheduler type:
///
/// - `mono_zero`: the monotonic reading at `Init::new`. Subtract it from a
///   later monotonic reading to obtain a boot-relative duration.
/// - `wall_anchor`: the wall-clock reading (Unix-epoch nanos) at the same
///   instant, used to derive an absolute wall-clock time for the boot origin
///   (LOG2 §2 first line, human audit use).
///
/// # Example
///
/// ```rust,no_run
/// use reovim_kernel::BootClock;
///
/// let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
/// // Elapsed nanoseconds since boot is always non-negative.
/// let elapsed = clock.elapsed_nanos();
/// assert!(elapsed < u64::MAX);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BootClock {
    /// Up-face clock control used for elapsed-time reads after capture.
    clock: ClockControl,
    /// Monotonic nanos read at boot-stage-0 — the re-base zero for LOG2
    /// timestamps.
    pub mono_zero: i64,
    /// Wall-clock nanos (Unix epoch) read at boot-stage-0 — the wall-clock
    /// anchor (7.5 §4).
    pub wall_anchor: i64,
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
    /// ```rust,no_run
    /// use reovim_kernel::BootClock;
    /// use reovim_uapi::sched::ClockControl;
    ///
    /// let clock = BootClock::capture(ClockControl::default());
    /// assert_eq!(clock.wall_anchor, 0);
    /// ```
    #[must_use]
    pub fn capture(clock: ClockControl) -> Self {
        Self {
            clock,
            mono_zero: clock.monotonic(),
            wall_anchor: clock.realtime(),
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
    /// ```rust,no_run
    /// use reovim_kernel::BootClock;
    ///
    /// let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    /// let elapsed = clock.elapsed_nanos();
    /// // Non-negative; cannot overflow a u64 in any realistic uptime.
    /// assert!(elapsed < u64::MAX);
    /// ```
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub fn elapsed_nanos(&self) -> u64 {
        // Clamp at 0: the monotonic clock never steps backward, so a current
        // reading at or before `mono_zero` (clock did not advance, or
        // `capture()` ran after the query) saturates to a zero elapsed rather
        // than wrapping. `.max(0)` keeps the value non-negative, so the cast to
        // u64 cannot lose sign.
        (self.clock.monotonic() - self.mono_zero).max(0) as u64
    }

    /// Microseconds elapsed since the boot anchor (truncated, not rounded).
    ///
    /// Convenience wrapper for LOG2 timestamp formatting (9.5 §2 requires
    /// microseconds, zero-padded to width 6).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use reovim_kernel::BootClock;
    ///
    /// let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    /// let us = clock.elapsed_micros();
    /// assert!(us < u64::MAX);
    /// ```
    #[must_use]
    pub fn elapsed_micros(&self) -> u64 {
        self.elapsed_nanos() / 1_000
    }
}

// L12 layout: tests in sibling clock_tests.rs, declared in lib.rs.
