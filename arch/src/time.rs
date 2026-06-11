//! Clocks over `clock_gettime` — the LOG2 timestamp source.
//!
//! [`Instant`] reads `CLOCK_MONOTONIC`, which never steps backward, so it is
//! the source for LOG2 timeline ordering (9.5 §2): two log lines emitted in
//! program order carry non-decreasing monotonic timestamps. Wall-clock time
//! (for human-readable LOG2 fields) comes from [`realtime`] over
//! `CLOCK_REALTIME`. Boot-relative rebasing of the monotonic clock is a
//! kernel concern, deferred (rule of three).

use crate::sys::{CLOCK_MONOTONIC, CLOCK_REALTIME, Timespec, clock_gettime};

/// Nanoseconds per second.
const NANOS_PER_SEC: i64 = 1_000_000_000;

/// A monotonic time point from `CLOCK_MONOTONIC`.
///
/// Never decreases between two reads on the same thread, so the difference of
/// two `Instant`s is a non-negative duration. The LOG2 timestamp source.
///
/// Ordering is by total nanoseconds (see [`as_nanos`](Instant::as_nanos)) so
/// the comparison does not depend on `Timespec` deriving `Ord`.
///
/// ```rust
/// use reovim_arch::time::Instant;
///
/// let t0 = Instant::now();
/// let t1 = Instant::now();
/// // A monotonic clock never decreases.
/// assert!(t1.as_nanos() >= t0.as_nanos());
/// assert!(t1.elapsed_nanos(t0) == (t1.as_nanos() - t0.as_nanos()) as u64);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instant {
    /// The captured monotonic time.
    ts: Timespec,
}

impl PartialOrd for Instant {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Instant {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_nanos().cmp(&other.as_nanos())
    }
}

impl Instant {
    /// Wraps a raw [`Timespec`] as an `Instant`, the inverse of [`monotonic`].
    ///
    /// The caller asserts the `Timespec` came from `CLOCK_MONOTONIC` (or an
    /// equivalent monotone source); the type carries no other invariant, so
    /// the conversion is total. Used where a monotonic reading was captured as
    /// a raw `Timespec` and a comparable [`Instant`] is wanted back.
    ///
    /// ```rust
    /// use reovim_arch::time::Instant;
    /// use reovim_arch::sys::Timespec;
    ///
    /// let ts = Timespec { tv_sec: 10, tv_nsec: 500 };
    /// let t = Instant::from_timespec(ts);
    /// assert_eq!(t.as_nanos(), 10 * 1_000_000_000 + 500);
    /// ```
    #[must_use]
    pub const fn from_timespec(ts: Timespec) -> Self {
        Self { ts }
    }

    /// Reads the current monotonic time.
    ///
    /// `CLOCK_MONOTONIC` is always available on Linux, so this never fails in
    /// practice; on the impossible error it returns the zero instant rather
    /// than panicking (the floor has no panic budget for a clock read).
    ///
    /// ```rust
    /// use reovim_arch::time::Instant;
    /// // Two successive reads are non-decreasing.
    /// let t0 = Instant::now();
    /// let t1 = Instant::now();
    /// assert!(t1 >= t0);
    /// ```
    #[must_use]
    pub fn now() -> Self {
        let mut ts = Timespec::default();
        // CLOCK_MONOTONIC cannot fail with a valid `Timespec` pointer; on the
        // theoretical error the zero instant is returned (monotone-safe: it
        // only ever compares <= a real reading).
        let _ = clock_gettime(CLOCK_MONOTONIC, &mut ts);
        Self { ts }
    }

    /// Whole nanoseconds since the unspecified monotonic epoch.
    ///
    /// Saturates rather than overflowing; the monotonic clock's range is far
    /// beyond any realistic process lifetime, so saturation is unreachable in
    /// practice but keeps the conversion total.
    ///
    /// ```rust
    /// use reovim_arch::time::Instant;
    /// use reovim_arch::sys::Timespec;
    ///
    /// let t = Instant::from_timespec(Timespec { tv_sec: 1, tv_nsec: 1 });
    /// assert_eq!(t.as_nanos(), 1_000_000_001);
    /// ```
    #[must_use]
    pub const fn as_nanos(self) -> i64 {
        self.ts
            .tv_sec
            .saturating_mul(NANOS_PER_SEC)
            .saturating_add(self.ts.tv_nsec)
    }

    /// Nanoseconds elapsed from `earlier` to `self`.
    ///
    /// Returns `0` if `earlier` is not actually earlier (a clock that did not
    /// advance), keeping the result a non-negative duration.
    ///
    /// ```rust
    /// use reovim_arch::time::Instant;
    /// use reovim_arch::sys::Timespec;
    ///
    /// let t0 = Instant::from_timespec(Timespec { tv_sec: 1, tv_nsec: 0 });
    /// let t1 = Instant::from_timespec(Timespec { tv_sec: 2, tv_nsec: 0 });
    /// assert_eq!(t1.elapsed_nanos(t0), 1_000_000_000);
    /// // Non-advancing clock returns zero.
    /// assert_eq!(t0.elapsed_nanos(t1), 0);
    /// ```
    #[must_use]
    pub const fn elapsed_nanos(self, earlier: Self) -> u64 {
        let delta = self.as_nanos() - earlier.as_nanos();
        if delta < 0 {
            0
        } else {
            // `delta >= 0` here, so the cast to `u64` is value-preserving.
            #[allow(clippy::cast_sign_loss)]
            let out = delta as u64;
            out
        }
    }
}

/// Reads the current monotonic time as a raw [`Timespec`].
///
/// Thin alias for [`Instant::now`]'s underlying read, for callers that want
/// the raw `timespec` rather than the comparison-only [`Instant`].
///
/// ```rust
/// use reovim_arch::time::monotonic;
///
/// let ts = monotonic();
/// assert!(ts.tv_sec >= 0);
/// assert!(ts.tv_nsec >= 0 && ts.tv_nsec < 1_000_000_000);
/// ```
#[must_use]
pub fn monotonic() -> Timespec {
    Instant::now().ts
}

/// Reads the current wall-clock (real) time.
///
/// The human-readable LOG2 timestamp source. Unlike [`monotonic`] this can
/// step (NTP, manual set), so it is never used for ordering.
///
/// ```rust
/// use reovim_arch::time::realtime;
///
/// let ts = realtime();
/// // Real-time clock returns positive seconds after the UNIX epoch (past year 2000).
/// assert!(ts.tv_sec > 946_684_800); // 2000-01-01 00:00:00 UTC
/// assert!(ts.tv_nsec >= 0 && ts.tv_nsec < 1_000_000_000);
/// ```
#[must_use]
pub fn realtime() -> Timespec {
    let mut ts = Timespec::default();
    // CLOCK_REALTIME cannot fail with a valid pointer; on the theoretical
    // error the zero timespec is returned.
    let _ = clock_gettime(CLOCK_REALTIME, &mut ts);
    ts
}

// L12 layout (#785 Phase 5): tests live in the sibling file `time_tests.rs`,
// declared as a parent-declared sibling in `lib.rs` (not a `#[path]` child
// here) because they access only the public API and need no `super::` access.
// The arch-selftest runner bin links them in via the `selftest` feature.
