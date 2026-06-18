//! Bare-metal aarch64 boot-core payload (QEMU raspi4b / BCM2711).
//!
//! Where the splash payload proves the framebuffer pipeline with a demo, this
//! payload boots the REAL reovim kernel on the freestanding floor: it installs
//! the framebuffer as a console sink, then calls [`Init::boot`]. The kernel's
//! boot-stage log streams to the UART and the framebuffer live, through the
//! kernel's own stderr echo and the floor's `write` fan-out — no manual ring
//! drain. It parks in a `wfe` loop afterward so the rendered surface persists
//! for a QEMU screendump. This is the `Init` -> `Kernel` handoff running on
//! bare metal instead of in a hosted test harness — the first real-runtime
//! proof of life on the floor.
//!
//! Boot-core needs the platform slots that are real on aarch64-none — clock,
//! alloc, park, and now `file_write` (fd 1/2 -> the floor's UART+console
//! sink); the socket/thread stubs are untouched.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` symbols the
// `unsafe_code` lint flags, and the wfe park uses inline asm; a bare-metal
// payload is unsafe by nature. The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use {
    core::arch::asm,
    reovim_arch::sys::{console, framebuffer, write},
    reovim_kernel::{Init, LauncherArgs},
};

/// Packs an RGB triple into a `0x00RRGGBB` pixel (RGB pixel-order tag).
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Parks the core forever so the rendered surface persists for capture.
fn park() -> ! {
    loop {
        // SAFETY: `wfe` is an unprivileged hint that parks the core until an
        // event; it touches no memory and has no architectural side effect.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

reovim_arch::entry!(|_argc, _argv, _envp| {
    // Install the framebuffer as the floor's console sink so everything written
    // to fd 1/2 — the kernel's own boot-stage stderr echo included — appears on
    // the HDMI surface as well as the UART. 2x glyph scale: 16px cells, legible
    // on the 1280x720 surface. If the mailbox alloc fails the floor stays
    // UART-only.
    if let Some(fb) = framebuffer::init() {
        console::install(console::Console::new(
            fb,
            rgb(0xC8, 0xE0, 0xFF),
            rgb(0x0A, 0x14, 0x28),
            2,
        ));
    }

    let _ = write(1, b"\nreovim kernel boot on bare metal\n");

    // Boot the REAL reovim kernel on the freestanding floor. Its boot-stage log
    // streams to both sinks live via the kernel's own stderr echo (fd 2 ->
    // file_write -> the floor's write fan-out) — no manual ring drain. `_kernel`
    // is held to boot and then parked.
    let Ok(_kernel) = Init::new(LauncherArgs::default()).boot() else {
        let _ = write(1, b"kernel boot FAILED\n");
        park();
    };

    let _ = write(1, b"kernel booted; runtime parked\n");
    park();
});
