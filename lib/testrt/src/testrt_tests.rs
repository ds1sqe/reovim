//! Tests for `lib.rs`, compiled into the lib under `selftest` (#785 Phase 5
//! coverage).
//!
//! L12 layout: declared inside `lib.rs` as
//! `#[cfg(feature = "selftest")] #[path = "testrt_tests.rs"] mod tests;`
//! so `super::` reaches the private `CURRENT_TEST` and `CURRENT_TEST_LEN`.
//!
//! Coverage targets (from the worklist):
//! - lib.rs `current_test() -> None` (null pointer arm)
//! - lib.rs `check` w/ no current test (`<unknown test>` branch)
//!
//! The former `write_all` bad-fd coverage case moved to `reovim-arch`'s facade:
//! output is now injected (`run(sink)`), so this core-only leaf has no fd-write
//! arm of its own to exercise.

use {crate::arch_test, core::sync::atomic::Ordering};

// Access private items via `super::`.
use super::{CURRENT_TEST, CURRENT_TEST_LEN, current_test};

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
    crate::check(result.is_none(), "current_test returns None when pointer is null");
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
    crate::check(true, "unknown-label path reached");

    // Restore the saved test name.
    CURRENT_TEST_LEN.store(saved_len, Ordering::Release);
    CURRENT_TEST.store(saved_ptr, Ordering::Release);
});
