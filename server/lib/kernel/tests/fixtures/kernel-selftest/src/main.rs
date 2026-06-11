//! The `no_std` kernel self-test runner (#796).
//!
//! Boots through the arch `_start` and runs the kernel test suite via
//! [`reovim_arch::testrt`]. Test bodies live in the kernel's sibling
//! `*_tests.rs` modules (L12.1), compiled in by `reovim-kernel/selftest`
//! and registered via `arch_test!`; this bin owns no tests of its own
//! beyond the inject-failure variant.
//!
//! The `inject-failure` feature adds one deliberately-failing test so the
//! integration harness can assert a non-zero exit.
#![no_std]
#![no_main]
// `entry!` expands to `#[unsafe(...)]` symbol declarations; a test-runner bin
// requires unsafe by nature.
#![allow(unsafe_code)]

use reovim_arch::testrt;

// Force the kernel rlib (and its `arch_test!` registrations) to link.
use reovim_kernel as _;

// One deliberately-failing test, present only under `inject-failure`.
#[cfg(feature = "inject-failure")]
reovim_arch::arch_test!(deliberately_fails, {
    reovim_arch::testrt::check_eq(1 + 1, 3);
});

reovim_arch::entry!(|_argc, _argv, _envp| { testrt::run() });
