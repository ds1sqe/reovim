//! Tests for `arena.rs`, compiled into the lib under `selftest`.
//!
//! L12 layout: declared in `arena.rs` via
//! `#[cfg(feature = "selftest")] #[path = "arena_tests.rs"] mod tests;`, so
//! `super::` reaches the arena constants and `alloc_pages`.
//!
//! The arena is a process-global bump allocator shared with the live test
//! suite, so these tests only exercise the rejection arms — which return
//! `ENOMEM` *without* advancing the bump offset — and never consume pool
//! space that later tests rely on.

use {
    super::{ARENA_BYTES, PAGE_SIZE, alloc_pages},
    crate::{arch_test, sys::ENOMEM, testrt},
};

arch_test!(arena_rejects_request_larger_than_pool, {
    // A request exceeding the whole pool fails cleanly with ENOMEM; the bump
    // offset is left untouched (fetch_update keeps the old value on None).
    testrt::check_eq(alloc_pages(ARENA_BYTES + PAGE_SIZE), Err(ENOMEM));
});

arch_test!(arena_rejects_overflowing_length, {
    // A length whose page round-up overflows usize is ENOMEM, not a panic or
    // wraparound — the checked_next_multiple_of arm.
    testrt::check_eq(alloc_pages(usize::MAX), Err(ENOMEM));
});
