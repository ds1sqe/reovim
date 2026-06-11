//! Tests for `arch::term`, compiled into the lib under `selftest` (#797 Phase 1).
//!
//! L12 layout: sibling file, declared inside `arch/src/term/mod.rs` as
//! `#[cfg(feature = "selftest")] #[path = "term_tests.rs"] mod tests;`.
//!
//! These tests exercise `RawMode::enter` on non-tty fds (the ENOTTY arm), since
//! the selftest runner stdin is typically not a real tty. The tty round-trip
//! (TCGETS before/after) is covered by the integration smoke fixture.

use crate::{
    arch_test,
    sys::{
        AT_FDCWD, EBADF, ENOTTY, O_CLOEXEC, O_CREAT, O_RDONLY, O_WRONLY, close, openat, unlinkat,
    },
    term::RawMode,
    testrt,
};

arch_test!(rawmode_enter_on_dev_null_returns_enotty, {
    // /dev/null is not a tty; TCGETS returns ENOTTY.
    let fd_raw =
        openat(AT_FDCWD, b"/dev/null\0", O_RDONLY | O_CLOEXEC, 0).expect("/dev/null opens");
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let fd = fd_raw as i32;
    let result = RawMode::enter(fd);
    close(fd).unwrap();
    testrt::check_eq(result.err(), Some(ENOTTY));
});

arch_test!(rawmode_enter_on_bad_fd_returns_ebadf, {
    // fd -1 is never valid; ioctl(TCGETS) returns EBADF.
    testrt::check_eq(RawMode::enter(-1).err(), Some(EBADF));
});

arch_test!(rawmode_enter_on_regular_file_returns_enotty, {
    // A regular file is not a tty; TCGETS returns ENOTTY.
    let path: &[u8] = b"/tmp/reovim-arch-term-regular-file\0";
    let fd_raw =
        openat(AT_FDCWD, path, O_CREAT | O_WRONLY | O_CLOEXEC, 0o600).expect("create regular file");
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let fd = fd_raw as i32;
    let result = RawMode::enter(fd);
    close(fd).unwrap();
    let _ = unlinkat(AT_FDCWD, path, 0);
    testrt::check_eq(result.err(), Some(ENOTTY));
});
