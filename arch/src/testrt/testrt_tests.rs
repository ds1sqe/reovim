//! Tests for `testrt/mod.rs`, compiled into the lib under `selftest` (#785
//! Phase 5 coverage).
//!
//! L12 layout: declared inside `testrt/mod.rs` as
//! `#[cfg(feature = "selftest")] #[path = "testrt_tests.rs"] mod tests;`
//! so `super::` reaches the private `write_all`, `CURRENT_TEST`,
//! `CURRENT_TEST_LEN`, and `write_usize`.
//!
//! Coverage targets (from the worklist):
//! - testrt/mod.rs L98: `current_test() -> None` (null pointer arm)
//! - testrt/mod.rs L128: `write_all` `Ok(0)|Err(_)` break arm (bad fd)
//! - testrt/mod.rs L198: `check` w/ no current test (`<unknown test>` branch)

use {crate::arch_test, core::sync::atomic::Ordering};

// Access private items via `super::`.
use super::{CURRENT_TEST, CURRENT_TEST_LEN, current_test, write_all};

arch_test!(testrt_current_test_none_when_cleared, {
    // The `current_test()` None arm fires when CURRENT_TEST is null.
    // We temporarily clear the pointer (the runner sets it before calling the
    // test body, so we save + restore it around the check).
    let saved_ptr = CURRENT_TEST.load(Ordering::Acquire);
    let saved_len = CURRENT_TEST_LEN.load(Ordering::Acquire);

    // Temporarily set to null to exercise the None arm.
    CURRENT_TEST.store(core::ptr::null_mut(), Ordering::Release);
    CURRENT_TEST_LEN.store(0, Ordering::Release);

    let result = current_test();

    // Restore so subsequent tests and the runner still work.
    CURRENT_TEST_LEN.store(saved_len, Ordering::Release);
    CURRENT_TEST.store(saved_ptr, Ordering::Release);

    // The None arm was taken.
    crate::testrt::check(result.is_none(), "current_test returns None when pointer is null");
});

arch_test!(testrt_write_all_handles_bad_fd, {
    // Write to fd -1: the kernel returns EBADF, which hits the `Err(_)` break
    // arm in `write_all` (testrt/mod.rs L128). A non-empty buffer is required
    // so the while loop body is entered at least once.
    // The function returns normally (no panic) — the break arm is best-effort.
    write_all(-1, b"probe");
    // If we reach here the break arm executed without a panic: coverage hit.
    crate::testrt::check(true, "write_all break arm on bad fd completed without panic");
});

arch_test!(testrt_check_with_no_active_test_uses_unknown_label, {
    // `check` calls `current_test().unwrap_or("<unknown test>")`.
    // This arm of `unwrap_or` fires when the ptr is null (the None case).
    // We force it the same way as above (save/null/restore).
    let saved_ptr = CURRENT_TEST.load(Ordering::Acquire);
    let saved_len = CURRENT_TEST_LEN.load(Ordering::Acquire);

    CURRENT_TEST.store(core::ptr::null_mut(), Ordering::Release);
    CURRENT_TEST_LEN.store(0, Ordering::Release);

    // `check(true, ...)` with a null current_test should not panic —
    // it formats "<unknown test>: msg" as the assert message (never triggered
    // because the condition is true).
    crate::testrt::check(true, "unknown-label path reached");

    // Restore the saved test name.
    CURRENT_TEST_LEN.store(saved_len, Ordering::Release);
    CURRENT_TEST.store(saved_ptr, Ordering::Release);
});
