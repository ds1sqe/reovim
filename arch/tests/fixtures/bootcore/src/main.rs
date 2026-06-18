//! Bare-metal aarch64 boot-core payload (QEMU raspi4b / BCM2711).
//!
//! Where the splash payload proves the framebuffer pipeline with a demo, this
//! payload boots the REAL reovim kernel on the freestanding floor: it calls
//! [`Init::boot`], then walks the kernel's boot-stage log ring and renders
//! every pre-rendered line to both the PL011 UART and the framebuffer console.
//! It parks in a `wfe` loop afterward so the rendered surface persists for a
//! QEMU screendump. This is the `Init` -> `Kernel` handoff running on bare
//! metal instead of in a hosted test harness — the first real-runtime proof of
//! life on the floor.
//!
//! Boot-core needs only the three platform slots that are already real on
//! aarch64-none (clock, alloc, park); the socket/thread/fd stubs are untouched.
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
    let _ = write(1, b"\nreovim kernel boot on bare metal\n");

    // Bring up the framebuffer console for on-screen output (UART is the
    // always-present fallback when the mailbox alloc fails).
    // 2x glyph scale: 16px cells, legible on the 1280x720 surface.
    let mut con = framebuffer::init()
        .map(|fb| console::Console::new(fb, rgb(0xC8, 0xE0, 0xFF), rgb(0x0A, 0x14, 0x28), 2));
    if let Some(con) = con.as_mut() {
        con.print("reovim kernel boot on bare metal\n");
    }

    // Boot the REAL reovim kernel on the freestanding floor.
    let Ok(kernel) = Init::new(LauncherArgs::default()).boot() else {
        let _ = write(1, b"kernel boot FAILED\n");
        if let Some(con) = con.as_mut() {
            con.print("kernel boot FAILED\n");
        }
        park();
    };

    let _ = write(1, b"kernel booted; boot-stage log ring:\n");
    if let Some(con) = con.as_mut() {
        con.print("kernel booted; boot-stage log ring:\n");
    }

    // Render every pre-rendered boot-stage line to both sinks. `entry.line`
    // already carries the timestamp/subsystem/message and a trailing newline.
    kernel.log_ring.for_each(|entry| {
        let line: &[u8] = &entry.line;
        let _ = write(1, line);
        if let Some(con) = con.as_mut() {
            // The rendered line is ASCII/UTF-8; skip the cell on the off chance
            // a byte slice is not valid UTF-8 rather than panicking on the floor.
            if let Ok(text) = core::str::from_utf8(line) {
                con.print(text);
            }
        }
    });

    park();
});
