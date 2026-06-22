//! The `no_std` server-runtime self-test runner (#797 Phase 3).
//!
//! Boots through the arch `_start` and runs the server-runtime test suite via
//! [`reovim_arch::testrt`]. Test bodies live in the server-runtime's sibling
//! `*_tests.rs` modules (L12.1), compiled in by `reovim-server-rt/selftest`
//! and registered via `arch_test!`; this bin owns the Phase 3 integration
//! smoke tests that require the text Domain (ext crate, cannot live in the
//! server-rt rlib — core/ext boundary).
//!
//! The `inject-failure` feature adds one deliberately-failing test so the
//! integration harness can assert a non-zero exit.
#![no_std]
#![no_main]
// `entry!` expands to `#[unsafe(...)]` symbol declarations; a test-runner bin
// requires unsafe by nature.
#![allow(unsafe_code)]

use reovim_arch::testrt;

// Force the server-runtime rlib (and its `arch_test!` registrations) to link.
use reovim_server_rt as _;

// Force the kernel rlib (and its `arch_test!` registrations) to link.
use reovim_kernel as _;

// Force the Domain contract tier rlib (and its `arch_test!` registrations).
use reovim_subsys_domain as _;

// Force the text Domain rlib (and its `arch_test!` registrations).
use reovim_domain_text as _;

// Phase 3 integration smoke: Hello → Attach → SendInput over a real UDS.
// Lives here (not in the server-rt rlib) because the server-rt has no dep on
// the text Domain ext crate (core/ext boundary).
mod runtime_smoke;

// One deliberately-failing test, present only under `inject-failure`.
#[cfg(feature = "inject-failure")]
reovim_arch::arch_test!(deliberately_fails, {
    reovim_arch::testrt::check_eq(1 + 1, 3);
});

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;

entry!(|_argc, _argv, _envp| {
    // Install the platform handle before anything reads it. The server-runtime
    // boot sequence accesses the handle; installing here, as the closure's
    // first statement, matches the no-read-before-install discipline the
    // `apps/*` composition roots follow. This fixture is host-only (native-only
    // pin per `fixtures_exec.rs`), so only the POSIX provider is needed — no
    // bare-metal scaffold branch.
    let _ = reovim_platform_linux_native::install_platform();
    let _ = reovim_system_kernel::mm::install_lib_ds_alloc_backend();
    let _ = reovim_system_kernel::sched::install_lib_ds_sync_backend();
    testrt::run()
});
