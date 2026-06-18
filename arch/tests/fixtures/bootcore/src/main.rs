//! Bare-metal boot-core payload: boots the REAL reovim kernel on the floor.
//!
//! Where the splash payload proves the framebuffer pipeline with a demo, this
//! payload boots the real kernel on the freestanding floor: it discovers the
//! machine's hardware facts, pushes them into [`Init::boot`] through
//! [`LauncherArgs`], and lets the kernel's boot-stage log stream to the console
//! live (UART, plus the framebuffer on aarch64). This is the `Init` -> `Kernel`
//! handoff running on bare metal instead of a hosted harness — the real-runtime
//! proof of life on the floor.
//!
//! Cross-arch (rule of three: two real arches now exist):
//!
//! - **aarch64** (QEMU raspi4b / BCM2711): installs the framebuffer as a console
//!   sink, then parks in `wfe` afterward so the rendered surface persists for a
//!   QEMU screendump (a manual proof).
//! - **x86_64** (QEMU q35, Multiboot1): UART-only — the boot banner rides
//!   fd 1/2 -> COM1. It exits through the `isa-debug-exit` channel after a
//!   successful boot, so the run is an automatable exit-code pilot.
//!
//! Boot-core needs the platform slots that are real on the freestanding floor —
//! clock, alloc, park, and `file_write` (fd 1/2 -> the floor's UART/console
//! sink); the socket/thread stubs are untouched.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` symbols the
// `unsafe_code` lint flags, and the aarch64 park uses inline asm; a bare-metal
// payload is unsafe by nature. The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use {
    reovim_arch::sys::{collect_boot_info, write},
    reovim_kernel::{Init, LauncherArgs},
};

#[cfg(target_arch = "aarch64")]
use {
    core::arch::asm,
    reovim_arch::sys::{console, framebuffer},
};

#[cfg(target_arch = "x86_64")]
use reovim_arch::sys::exit_group;

/// Packs an RGB triple into a `0x00RRGGBB` pixel (RGB pixel-order tag). Only the
/// aarch64 framebuffer path uses it.
#[cfg(target_arch = "aarch64")]
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Ends the run after the kernel has booted.
///
/// aarch64 parks the core in a `wfe` loop so the rendered framebuffer surface
/// persists for a QEMU screendump. x86 is UART-only — nothing to persist — so it
/// exits through the `isa-debug-exit` channel, making the boot an automatable
/// exit-code pilot. `code` is the exit status on the exiting arch; aarch64
/// ignores it (a parked image reports success by rendering, not by status).
#[cfg(target_arch = "aarch64")]
fn finish(_code: i32) -> ! {
    loop {
        // SAFETY: `wfe` is an unprivileged hint that parks the core until an
        // event; it touches no memory and has no architectural side effect.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn finish(code: i32) -> ! {
    exit_group(code)
}

reovim_arch::entry!(|_argc, _argv, _envp| {
    // aarch64: install the framebuffer as the floor's console sink so the
    // kernel's boot-stage log (its own fd-2 stderr echo included) renders on the
    // HDMI surface as well as the UART. 2x glyph scale, legible on 1280x720. If
    // the mailbox alloc fails the floor stays UART-only.
    #[cfg(target_arch = "aarch64")]
    if let Some(fb) = framebuffer::init() {
        console::install(console::Console::new(
            fb,
            rgb(0xC8, 0xE0, 0xFF),
            rgb(0x0A, 0x14, 0x28),
            2,
        ));
    }

    let _ = write(1, b"\nreovim kernel boot on bare metal\n");

    // Discover the machine's real hardware facts (RAM / CPU) and push them into
    // the kernel at entry through `LauncherArgs.boot_info`. The boot-tail
    // diagnostics banner prints them on the live console.
    let boot_info = collect_boot_info();
    let args = LauncherArgs {
        boot_info,
        ..LauncherArgs::default()
    };

    // Boot the REAL reovim kernel on the freestanding floor. Its boot-stage log
    // streams to the console live via the kernel's own stderr echo (fd 2 ->
    // file_write -> the floor's write fan-out) — no manual ring drain.
    let Ok(_kernel) = Init::new(args).boot() else {
        let _ = write(1, b"kernel boot FAILED\n");
        finish(70);
    };

    let _ = write(1, b"kernel booted\n");
    finish(0);
});
