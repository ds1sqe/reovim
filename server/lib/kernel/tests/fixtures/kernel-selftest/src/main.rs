//! The `no_std` kernel self-test runner (#796, extended in #797).
//!
//! Boots through the arch `_start` and runs the kernel test suite via
//! [`reovim_arch::testrt`]. Test bodies live in the kernel's sibling
//! `*_tests.rs` modules (L12.1), compiled in by `reovim-kernel/selftest`
//! and registered via `arch_test!`; this bin owns the Phase 2 integration
//! smoke tests that require the text Domain (ext crate, cannot live in the
//! kernel rlib — core/ext boundary).
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

// Force the Domain contract tier rlib (and its `arch_test!` registrations) to
// link. Its sibling test modules compile in under `selftest`.
use reovim_subsys_domain as _;

// Force the text Domain rlib (and its `arch_test!` registrations) to link.
// The domain's selftest modules are compiled in when the `selftest` feature is
// enabled (see `reovim-domain-text/Cargo.toml`).
use reovim_domain_text as _;

// Phase 2 integration smoke: kernel + text Domain dispatch end-to-end.
// Lives here (not in the kernel rlib) because the kernel has no dep on ext.
mod domain_smoke;

// One deliberately-failing test, present only under `inject-failure`.
#[cfg(feature = "inject-failure")]
reovim_arch::arch_test!(deliberately_fails, {
    reovim_arch::testrt::check_eq(1 + 1, 3);
});

reovim_arch::entry!(|_argc, _argv, _envp| {
    // Pin the panic disposition to Halt before running any test. The runner is
    // fail-fast: a failing selftest is non-recoverable, so it must exit the
    // halt code (70), deterministically — independent of test order. Without
    // this, a kernel-boot test that registers the kernel's runtime
    // `Disposition::Recover` (write-once, process-global) leaks into a later
    // test's failure exit, which would otherwise depend on link-section
    // ordering. The runner owns the process exit policy, so it registers first;
    // later kernel-boot registrations see `AlreadySet` (tolerated). The result
    // is ignored: this is the first writer by construction.
    let _ = reovim_arch::panic::set_disposition(reovim_arch::panic::Disposition::Halt);
    testrt::run()
});
