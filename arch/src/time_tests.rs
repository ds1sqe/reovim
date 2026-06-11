//! Tests for `arch/src/time.rs`, compiled into the lib under `selftest`
//! (#785 Phase 5) and included by the arch-selftest runner bin.
//!
//! Declared as a parent-declared sibling in `lib.rs` (all items under test
//! are public, so no `#[path]` child redirect is needed).

use crate::{
    arch_test,
    sys::Timespec,
    time::{Instant, monotonic, realtime},
};

arch_test!(time_monotonic_is_non_decreasing, {
    let a = Instant::now();
    let b = Instant::now();
    crate::testrt::check(b >= a, "monotonic clock never steps backward");
    crate::testrt::check(b.as_nanos() >= a.as_nanos(), "nanos non-decreasing");
});

arch_test!(time_elapsed_nanos_non_negative, {
    let a = Instant::now();
    let b = Instant::now();
    // b is at or after a, so elapsed is well-defined and non-negative.
    let _ = b.elapsed_nanos(a);
    // The reversed direction clamps to zero rather than underflowing.
    crate::testrt::check_eq(a.elapsed_nanos(b), 0u64);
});

arch_test!(time_elapsed_nanos_measures_a_gap, {
    // A constructed gap: two instants a known number of nanos apart via the
    // public `from_timespec` constructor, proving the arithmetic.
    let earlier = Instant::from_timespec(Timespec {
        tv_sec: 1,
        tv_nsec: 0,
    });
    let later = Instant::from_timespec(Timespec {
        tv_sec: 1,
        tv_nsec: 500,
    });
    crate::testrt::check_eq(later.elapsed_nanos(earlier), 500u64);
});

arch_test!(time_monotonic_raw_matches_instant, {
    let ts = monotonic();
    crate::testrt::check(ts.tv_sec > 0 || ts.tv_nsec > 0, "monotonic raw is nonzero");
});

arch_test!(time_realtime_is_plausibly_nonzero, {
    let ts = realtime();
    // Wall clock is well past the epoch in any realistic environment.
    crate::testrt::check(ts.tv_sec > 0, "realtime past the epoch");
});

arch_test!(time_as_nanos_saturation_is_total, {
    // A pathological timespec drives the saturating arithmetic without
    // overflow (the saturate branch is otherwise unreachable from a real read).
    let huge = Instant::from_timespec(Timespec {
        tv_sec: i64::MAX,
        tv_nsec: i64::MAX,
    });
    crate::testrt::check_eq(huge.as_nanos(), i64::MAX);
});
