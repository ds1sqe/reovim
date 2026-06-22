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

use core::{alloc::Layout, ptr::NonNull};

use {
    reovim_arch::testrt,
    reovim_uapi_mm::{AllocControl, AllocError},
};

// Force the system-kernel rlib (and its `arch_test!` registrations for the
// lifted console/fonts/escape/color/fdt/boot-info/inventory selftests) to link,
// so `testrt::run()` walks them — they register into the shared link section
// (SP04 04a: coverage relocated with the code, no regression).
use reovim_system_kernel as _;

// One deliberately-failing test, present only under `inject-failure`, to
// demonstrate the runner's non-zero exit on a failing test (fail-fast: the
// panic handler exits the halt disposition code, failing the bin).
#[cfg(feature = "inject-failure")]
reovim_arch::arch_test!(deliberately_fails, {
    reovim_arch::testrt::check_eq(1 + 1, 3);
});

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_floor_none_x86_64::entry;

fn arch_test_alloc(layout: Layout) -> Result<NonNull<u8>, AllocError> {
    reovim_arch::alloc::alloc(layout).map_err(|_| AllocError)
}

fn arch_test_dealloc(ptr: NonNull<u8>, layout: Layout) {
    // SAFETY: lib/ds calls this only with pointers/layouts returned by the
    // paired `arch_test_alloc` function and not yet freed.
    unsafe { reovim_arch::alloc::dealloc(ptr, layout) }
}

fn install_arch_test_alloc_backend()
-> Result<(), reovim_lib_ds::alloc_backend::AllocBackendInstallError> {
    reovim_lib_ds::alloc_backend::install(AllocControl::new(arch_test_alloc, arch_test_dealloc))
}

entry!(|_argc, _argv, _envp| {
    // Install the platform handle as the closure's first statement, before any
    // test reads it (`lib_ds_tests` allocates + parks through installed DS
    // backends). The installer is cfg-split: the real POSIX provider on hosted
    // Linux, the bare-metal scaffold on `*-unknown-none` — exactly one links
    // per target, matching the no-read-before-install discipline the `apps/*`
    // roots follow.
    #[cfg(not(target_os = "linux"))]
    let _ = reovim_platform_stub_none::install_platform();
    #[cfg(target_os = "linux")]
    let _ = reovim_platform_linux_native::install_platform();
    let _ = install_arch_test_alloc_backend();
    let _ = reovim_system_kernel::sched::install_lib_ds_sync_backend();
    testrt::run()
});
