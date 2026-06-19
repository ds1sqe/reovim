//! Tests for `term.rs`, compiled into the lib under `selftest` (#797 Phase 1).
//!
//! L12 layout: sibling file declared in `term.rs` via
//! `#[cfg(feature = "selftest")] #[path = "term_tests.rs"] mod tests;`.

use {
    crate::term::{
        BRKINT, ECHO, ECHOE, ECHOK, ICANON, ICRNL, INPCK, ISIG, ISTRIP, IXON, NCCS, OPOST, TCGETS,
        TCSETS, Termios, VMIN, VTIME,
    },
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(termios_zeroed_is_all_zero, {
    let t = Termios::zeroed();
    testrt::check_eq(t.c_iflag, 0u32);
    testrt::check_eq(t.c_oflag, 0u32);
    testrt::check_eq(t.c_cflag, 0u32);
    testrt::check_eq(t.c_lflag, 0u32);
    testrt::check_eq(t.c_line, 0u8);
    testrt::check(t.c_cc.iter().all(|&b| b == 0), "c_cc all zero");
});

arch_test!(termios_cc_array_length_is_nccs, {
    let t = Termios::zeroed();
    testrt::check_eq(t.c_cc.len(), NCCS);
    testrt::check_eq(NCCS, 19usize);
});

arch_test!(termios_flag_constants_match_kernel_values, {
    // Verified against include/uapi/asm-generic/termbits.h.
    testrt::check_eq(ICANON, 0x0000_0002u32);
    testrt::check_eq(ECHO, 0x0000_0008u32);
    testrt::check_eq(ECHOE, 0x0000_0010u32);
    testrt::check_eq(ECHOK, 0x0000_0020u32);
    testrt::check_eq(ISIG, 0x0000_0001u32);
    testrt::check_eq(OPOST, 0x0000_0001u32);
    testrt::check_eq(BRKINT, 0x0000_0002u32);
    testrt::check_eq(ICRNL, 0x0000_0100u32);
    testrt::check_eq(IXON, 0x0000_0400u32);
    testrt::check_eq(INPCK, 0x0000_0010u32);
    testrt::check_eq(ISTRIP, 0x0000_0020u32);
});

arch_test!(termios_ioctl_request_constants_match_kernel_values, {
    // Verified against include/uapi/asm-generic/ioctls.h.
    testrt::check_eq(TCGETS, 0x5401usize);
    testrt::check_eq(TCSETS, 0x5402usize);
});

arch_test!(vmin_vtime_indices_match_kernel_values, {
    // Verified against include/uapi/asm-generic/termbits.h.
    testrt::check_eq(VMIN, 6usize);
    testrt::check_eq(VTIME, 5usize);
});

arch_test!(termios_clone_copy_is_byte_equal, {
    let mut orig = Termios::zeroed();
    orig.c_iflag = 0xABCD_1234;
    orig.c_lflag = 0x1234_5678;
    orig.c_cc[VMIN] = 1;
    orig.c_cc[VTIME] = 0;
    let copy = orig;
    testrt::check_eq(orig, copy);
});
