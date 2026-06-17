//! Clock Math wrappers over the kabi platform handle's time primitives.
//!
//! [`monotonic`] and [`realtime`] reach the boot-installed `kabi` handle's
//! `clock`/`realtime` slots — never `arch` directly. The two clocks are
//! distinct: monotonic never steps and carries an unspecified epoch (use it for
//! ordering and durations); realtime is wall-clock nanos since the Unix epoch
//! and can step under NTP or a manual clock set (use it only as a
//! human-readable timestamp anchor). Same Math-over-handle pattern as
//! [`crate::net`]: the algorithm is portable, the backend is injected at boot
//! (master invariant 2: `lib/ds ⊄ arch`).
//!
//! ## Bootstrap prerequisite
//!
//! Both functions read the `kabi` handle, so they must run after the boot path
//! installs it — the same no-DS-before-install prerequisite as every other
//! `lib/ds` op.

use reovim_kabi_platform::handle;

/// Reads the monotonic clock, returning whole nanoseconds since an unspecified
/// epoch.
///
/// The reading never steps backward, so the difference of two readings is a
/// non-negative duration. Use it for ordering and durations, never as a
/// human-readable timestamp — for that use [`realtime`].
///
/// ```no_run
/// // no_run: reads the boot-installed handle.
/// let _nanos = reovim_lib_ds::time::monotonic();
/// ```
#[must_use]
pub fn monotonic() -> i64 {
    handle().clock()
}

/// Reads the wall clock, returning whole nanoseconds since the Unix epoch.
///
/// The reading can step (NTP, a manual clock set), so it is a human-readable
/// timestamp anchor, never a basis for ordering — use [`monotonic`] for
/// durations.
///
/// ```no_run
/// // no_run: reads the boot-installed handle.
/// let _nanos = reovim_lib_ds::time::realtime();
/// ```
#[must_use]
pub fn realtime() -> i64 {
    handle().realtime()
}
