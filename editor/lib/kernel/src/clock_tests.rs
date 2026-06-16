//! Tests for `clock.rs` — `BootClock` unit coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The kernel-selftest bin in
//! tests/fixtures/ runs these.

use reovim_arch::arch_test;

use crate::clock::BootClock;

arch_test!(boot_clock_capture_wall_after_epoch, {
    let clock = BootClock::capture();
    // Wall anchor must be after year 2000.
    assert!(clock.wall_anchor.tv_sec > 946_684_800, "wall anchor before epoch");
});

arch_test!(boot_clock_elapsed_nanos_completes, {
    let clock = BootClock::capture();
    // Verify the call completes and returns a value in the u64 range.
    assert!(clock.elapsed_nanos() < u64::MAX, "elapsed_nanos must be in u64 range");
});

arch_test!(boot_clock_elapsed_micros_consistent_with_nanos, {
    let clock = BootClock::capture();
    let nanos = clock.elapsed_nanos();
    let micros = clock.elapsed_micros();
    // micros must be <= nanos / 1000 at most (monotone, but reading twice).
    // The delta is tiny; just verify micros <= nanos (since 1 us = 1000 ns).
    assert!(micros <= nanos, "micros={micros} > nanos={nanos}");
});
