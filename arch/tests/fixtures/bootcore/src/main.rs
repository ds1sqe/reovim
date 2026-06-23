//! Bare-metal boot-core payload wrapper for the RTOS image composition root.
//!
//! This fixture remains the `arch/tests/fixtures` integration surface for CI system-image
//! coverage. The actual kernel-owned boot sequence now lives in `apps/os`:
//! platform/runtime install, console registration, boot-fact assembly, splash, and
//! root daemon.
#![no_std]
#![no_main]
// entry! expands to a platform ABI entry shim with unsafe linkage attrs.
#![allow(unsafe_code)]

use reovim_os::boot::{run_shell_profile, shell_only_profile};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_floor_none_x86_64::entry;

entry!(|_argc, _argv, _envp| {
    run_shell_profile(shell_only_profile())
});
