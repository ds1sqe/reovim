//! Bare-metal aarch64 boot-splash demo (QEMU raspi4b / BCM2711).
//!
//! A thin harness around the system kernel's standing boot path: it boots
//! through the arch `_start`, installs the freestanding system-kernel platform
//! vtable, stands up the system kernel's framebuffer console as the floor's
//! fd 1/2 write sink (SP04 04b), and lets the system kernel's own boot-splash
//! module (SP04 04c) paint the branded splash through that standing console. It
//! then parks in a `wfe` loop forever — it deliberately does NOT semihosting-
//! exit, so the rendered surface persists for a QEMU screendump / VNC capture.
//!
//! The splash *content* (wordmark, accent rule, identity line, boot panel, SGR
//! diagnostics) is policy and lives in `reovim_system_kernel::splash`; the floor
//! exposes only mechanism (`framebuffer`, the standing `console`). This fixture
//! is the dedicated visual proof: a minimal composition root whose only job is
//! to stand up the boot path and let the standing splash render, so the
//! screendump has a self-contained bin.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` symbol declarations the
// `unsafe_code` lint flags, and the wfe park uses inline asm; a bare-metal
// payload is unsafe by nature. The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use {
    core::arch::asm,
    // The framebuffer (VideoCore MMIO) STAYS below; the platform vtable, the
    // standing console, and the splash content all live in the system kernel.
    reovim_arch::sys::framebuffer,
    reovim_system_kernel::{color::Color, console, fonts, platform, splash},
};

/// Packs an RGB triple into a `0x00RRGGBB` pixel (matches the requested RGB
/// pixel-order tag).
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// The font the standing console renders through. The 1280x720 surface at this
/// font's 8-px cell yields a 160-column console — the width the splash module's
/// centring offsets assume.
const BOOT_FONT: &fonts::Font = &fonts::JETBRAINS_MONO;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;

entry!(|_argc, _argv, _envp| {
    // Boot ordering (no read before install): install the platform vtable, then
    // the standing console, then render the splash. The splash writes through
    // fd 1, which the vtable's `file_write` slot and the floor `write` route to
    // the UART plus the console — so both the vtable and the console must stand
    // before the first splash byte.
    let _ = platform::install_platform();

    // Acquire the framebuffer and capture its geometry before the surface is
    // moved into the standing console (the console consumes it by value). The
    // geometry feeds the splash boot panel; `None` is the degraded UART-only
    // path when the mailbox alloc or the one-shot screen-grid hand-out fails.
    let fb = framebuffer::init();
    let geometry = fb.as_ref().map(|fb| (fb.width(), fb.height()));

    if let Some((fb, grid)) = fb.zip(console::screen_grid()) {
        // Light text on a dark slate background — legible on an HDMI capture and
        // unmistakably "ours". The standing console clears the surface to the
        // background pen and registers its trampoline into the floor's write
        // sink, so every fd 1/2 byte fans to the framebuffer from here on.
        console::install_console(
            fb,
            BOOT_FONT,
            Color::Rgb(rgb(0xC8, 0xE0, 0xFF)),
            Color::Rgb(rgb(0x0A, 0x14, 0x28)),
            grid,
        );
    }

    // Render the branded splash through the standing console (UART-only when no
    // framebuffer came up).
    splash::render(geometry);

    loop {
        // SAFETY: `wfe` is an unprivileged hint that parks the core until an
        // event; it touches no memory and has no architectural side effect
        // beyond the wait. The surface stays resident for screendump capture.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
});
