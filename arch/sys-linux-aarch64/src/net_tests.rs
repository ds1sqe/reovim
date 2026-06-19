//! Tests for `net.rs`, compiled into the lib under `selftest` (#797 Phase 1).
//!
//! L12 layout: sibling file declared in `net.rs` via
//! `#[cfg(feature = "selftest")] #[path = "net_tests.rs"] mod tests;`. Tests
//! access only public items from `crate::net`, so `super::` crate-private
//! access is not needed.

use {
    crate::net::{AF_UNIX, SOCK_STREAM, SockaddrUn, UNIX_PATH_MAX},
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(sockaddr_un_zeroed_is_all_zero, {
    let sa = SockaddrUn::zeroed();
    testrt::check_eq(sa.sun_family, 0u16);
    testrt::check(sa.sun_path.iter().all(|&b| b == 0), "zeroed sun_path is all zero");
});

arch_test!(sockaddr_un_addrlen_short_path, {
    let mut sa = SockaddrUn::zeroed();
    sa.sun_family = AF_UNIX;
    // b"/tmp/ab\0" is 8 bytes; non-NUL length 7, NUL at index 7.
    sa.sun_path[..8].copy_from_slice(b"/tmp/ab\0");
    // addrlen = offsetof(sun_path) + 7 + 1 = 2 + 8 = 10.
    testrt::check_eq(sa.addrlen(), 10);
});

arch_test!(sockaddr_un_addrlen_empty_path_is_two, {
    // sun_path[0] == 0 → path_len = 1 (the NUL itself) → addrlen = 2 + 1 = 3.
    // (An empty abstract socket, but this exercises the zero-position branch.)
    let sa = SockaddrUn::zeroed();
    // The NUL is at index 0; addrlen = 2 + 1 = 3.
    testrt::check_eq(sa.addrlen(), 3);
});

arch_test!(sockaddr_un_addrlen_max_path, {
    // Fill all bytes with non-NUL to reach the fallback (UNIX_PATH_MAX).
    let mut sa = SockaddrUn::zeroed();
    for b in &mut sa.sun_path {
        *b = b'x';
    }
    // No NUL → addrlen = 2 + UNIX_PATH_MAX.
    testrt::check_eq(sa.addrlen(), 2 + UNIX_PATH_MAX);
});

arch_test!(sockaddr_un_constants_match_kernel_values, {
    // AF_UNIX = 1, SOCK_STREAM = 1 per kernel headers.
    testrt::check_eq(AF_UNIX, 1u16);
    testrt::check_eq(SOCK_STREAM, 1usize);
    testrt::check_eq(UNIX_PATH_MAX, 108usize);
});
