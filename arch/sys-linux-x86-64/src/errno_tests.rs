//! Tests for `errno.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: sibling file declared in `errno.rs` via
//! `#[cfg(feature = "selftest")] #[path = "errno_tests.rs"] mod tests;`. The
//! tests access only public items (`Errno`, `from_ret`, named constants — the
//! crate-root re-exports), so crate-private access through `super::` is not
//! needed.

use {
    crate::{EAGAIN, EBADF, ENOMEM, EWOULDBLOCK, Errno, from_ret},
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(code_returns_positive_errno, {
    testrt::check_eq(EBADF.code(), 9);
    testrt::check_eq(ENOMEM.code(), 12);
    testrt::check_eq(Errno(4095).code(), 4095);
    testrt::check_eq(EWOULDBLOCK.code(), EAGAIN.code());
});

arch_test!(from_ret_range_boundaries, {
    testrt::check_eq(from_ret(-1), Err(Errno(1)));
    testrt::check_eq(from_ret(-4095), Err(Errno(4095)));
    testrt::check_eq(from_ret(0), Ok(0));
    // One below the floor is an address-shaped success, not an error.
    testrt::check_eq(from_ret(-4096), Ok((-4096_isize).cast_unsigned()));
});
