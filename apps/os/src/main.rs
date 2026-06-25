//! `reovim-os` — RTOS image composition root (#800).
//!
//! The binary wires provider/floor startup to the shared RTOS boot flow in
//! `boot.rs`. It owns app-profile selection and per-profile payload wiring; the
//! root daemon and splash path are shared implementation.
#![no_std]
#![no_main]
// entry! expands to `#[unsafe(no_mangle)]`, so keep one explicit allowance.
#![allow(unsafe_code)]

#[cfg(feature = "launch-profile")]
use reovim_os::boot::launch_profile;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_os::boot::exec_bundle_profile;
use reovim_os::boot::{run_shell_profile, shell_only_profile, BootProfile};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use reovim_arch_floor_none_x86_64::entry;

fn shell_or_exec_bundle_profile() -> BootProfile<'static> {
    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    {
        let requested = option_env!("REOVIM_OS_PROFILE").unwrap_or("shell-only");
        if requested == "exec-bundle" {
            return exec_bundle_profile();
        }
    }
    shell_only_profile()
}

#[cfg(not(feature = "launch-profile"))]
fn resolve_profile() -> BootProfile<'static> {
    shell_or_exec_bundle_profile()
}

#[cfg(feature = "launch-profile")]
fn resolve_profile() -> BootProfile<'static> {
    match option_env!("REOVIM_OS_PROFILE").unwrap_or("shell-only") {
        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        "exec-bundle" => exec_bundle_profile(),
        "launch" => launch_profile(),
        "shell-only" => shell_or_exec_bundle_profile(),
        _ => shell_or_exec_bundle_profile(),
    }
}

entry!(|_argc, _argv, _envp| {
    run_shell_profile(resolve_profile())
});
