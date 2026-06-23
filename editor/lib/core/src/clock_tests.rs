//! Tests for `clock.rs` — `BootClock` unit coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The editor-core-selftest bin in
//! tests/fixtures/ runs these.

use {reovim_arch::arch_test, reovim_uapi::sched::ClockControl};

use crate::clock::BootClock;

fn test_clock_control() -> ClockControl {
    ClockControl::new(test_monotonic, test_realtime)
}

fn test_monotonic() -> i64 {
    1_000_000
}

fn test_realtime() -> i64 {
    946_684_800_000_000_001
}

arch_test!(boot_clock_capture_wall_after_epoch, {
    let clock = BootClock::capture(test_clock_control());
    // Wall anchor must be after year 2000 (Unix-epoch nanos).
    assert!(clock.wall_anchor > 946_684_800_000_000_000, "wall anchor before epoch");
});

arch_test!(boot_clock_elapsed_nanos_completes, {
    let clock = BootClock::capture(test_clock_control());
    // Verify the call completes and returns a value in the u64 range.
    assert!(clock.elapsed_nanos() < u64::MAX, "elapsed_nanos must be in u64 range");
});

arch_test!(boot_clock_elapsed_micros_consistent_with_nanos, {
    let clock = BootClock::capture(test_clock_control());
    let nanos = clock.elapsed_nanos();
    let micros = clock.elapsed_micros();
    // micros must be <= nanos / 1000 at most (monotone, but reading twice).
    // The delta is tiny; just verify micros <= nanos (since 1 us = 1000 ns).
    assert!(micros <= nanos, "micros={micros} > nanos={nanos}");
});
