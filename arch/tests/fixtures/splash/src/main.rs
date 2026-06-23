//! Bare-metal framebuffer splash proof wrapper (#800).
//!
//! This fixture now delegates the boot-system ordering to `reovim-os`, keeping
//! splash composition in the shared RTOS composition root and avoiding duplicated
//! boot/prompt logic above platform/boot layers.
#![no_std]
#![no_main]
// entry! expands to `#[unsafe(no_mangle)]`, so the global lint is scoped to this
// fixture.
#![allow(unsafe_code)]

use reovim_os::boot::run_splash_profile;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_floor_none_x86_64::entry;

entry!(|_argc, _argv, _envp| {
    run_splash_profile()
});
