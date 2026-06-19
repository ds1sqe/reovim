//! The `no_std` arch self-test runner (#785 Phase 5).
//!
//! Boots through the arch `_start` and runs the full arch test suite via
//! [`reovim_arch::testrt`]. All `arch_test!` registrations come from the lib
//! (compiled in via the `selftest` feature — every `*_tests.rs` sibling module
//! is gated on that feature and declared inside its source file or in `lib.rs`).
//! This bin owns no tests of its own; it is purely a runner entrypoint.
//!
//! The `inject-failure` feature adds one deliberately-failing test so an
//! integration test can exec the failing variant and assert the runner exits
//! non-zero — the charter smoke's "one passing + one failing, correct exit
//! codes" requirement.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(...)]` symbol declarations the
// `unsafe_code` lint flags; a test-runner bin is unsafe by nature.
#![allow(unsafe_code)]

use reovim_arch::testrt;

// One deliberately-failing test, present only under `inject-failure`, to
// demonstrate the runner's non-zero exit on a failing test (fail-fast: the
// panic handler exits the halt disposition code, failing the bin).
#[cfg(feature = "inject-failure")]
reovim_arch::arch_test!(deliberately_fails, {
    reovim_arch::testrt::check_eq(1 + 1, 3);
});

reovim_arch::entry!(|_argc, _argv, _envp| {
    // Install the platform handle as the closure's first statement, before any
    // test reads it (`lib_ds_tests` allocates + parks through the handle). The
    // installer is cfg-split: the real POSIX provider on hosted Linux, the
    // bare-metal scaffold on `*-unknown-none` — exactly one links per target,
    // matching the no-read-before-install discipline the `apps/*` roots follow.
    #[cfg(not(target_os = "linux"))]
    let _ = reovim_platform_stub_none::install_platform();
    #[cfg(target_os = "linux")]
    let _ = reovim_platform_linux_native::install_platform();
    testrt::run()
});
