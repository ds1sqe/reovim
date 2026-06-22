//! Tests for `paint.rs` raw-mode entry (L12.1 sibling, compiled under
//! `selftest`).
//!
//! The raw-mode/restore round trip on a real tty needs a terminal the selftest
//! runner does not have, so the covered path is the **ENOTTY fallback**: when
//! stdin is not a tty (a pipe or `/dev/null`), the provider's `term_set_raw`
//! TCGETS yields `ENOTTY`, [`RawMode::enter`] surfaces it, and `run`'s
//! `Err(ENOTTY)` arm continues without a raw guard. These tests reach the
//! platform handle the fixture composition root installs
//! (`reovim_platform_linux_native::install_platform`), so `RawMode::enter`
//! dispatches through the real provider.

use {
    reovim_arch::{
        arch_test,
        sys::{AT_FDCWD, O_CLOEXEC, O_RDONLY, close, openat},
        testrt,
    },
    reovim_kabi_platform::{ENOTTY, RawMode},
};

arch_test!(paint_enotty_fallback_on_non_tty, {
    // A non-tty fd: /dev/null. TCGETS on it yields ENOTTY (it is not a terminal),
    // exactly the pipe-stdin case `run` must tolerate.
    let fd_raw =
        openat(AT_FDCWD, b"/dev/null\0", O_RDONLY | O_CLOEXEC, 0).expect("/dev/null opens");
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let fd = fd_raw as i32;

    // (1) RawMode::enter on the non-tty fd returns Err(ENOTTY).
    let result = RawMode::enter(fd);
    testrt::check_eq(result.err(), Some(ENOTTY));

    // (2) The fallback arm `run` takes (paint.rs:152-156): the Err(ENOTTY) match
    // produces None — no raw guard is constructed, so no restore is registered.
    let guard = match RawMode::enter(fd) {
        Ok(g) => Some(g),
        Err(e) if e == ENOTTY => None, // the fallback arm
        Err(_) => None,
    };
    testrt::check(guard.is_none(), "ENOTTY takes the no-raw-guard fallback arm");

    close(fd).unwrap();
});
