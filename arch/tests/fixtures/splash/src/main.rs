//! Bare-metal aarch64 boot-log payload (QEMU raspi4b / BCM2711).
//!
//! Boots through the arch `_start`, asks the `VideoCore` for a 1280x720x32
//! framebuffer over the mailbox property interface, wraps it in a
//! [`console::Console`] rendering a selectable embedded font (here
//! [`fonts::JETBRAINS_MONO`], anti-aliased) via coverage software-blend, and
//! renders the boot log onto the HDMI surface
//! while echoing the same lines to the PL011 UART. It then parks in a `wfe`
//! loop forever — it deliberately does NOT semihosting-exit, so the rendered
//! surface persists for a QEMU screendump / VNC capture. This is the floor's
//! first legible proof of life on screen, not only on the serial line.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` symbol declarations the
// `unsafe_code` lint flags, and the wfe park uses inline asm; a bare-metal
// payload is unsafe by nature. The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use {
    core::arch::asm,
    reovim_arch::sys::{color::Color, console, fonts, framebuffer, write},
};

/// Packs an RGB triple into a `0x00RRGGBB` pixel (matches the requested RGB
/// pixel-order tag).
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// The font this boot log renders through. Selecting another embedded font is
/// a one-line change here (e.g. [`fonts::TERMINUS`]).
const BOOT_FONT: &fonts::Font = &fonts::JETBRAINS_MONO;

/// A dual-sink boot log: every line goes to the PL011 UART (fd 1) and, once
/// the framebuffer is up, to the on-screen text console as well.
///
/// Before the framebuffer is acquired `console` is `None` and output is
/// UART-only — the natural degraded path when the mailbox alloc fails.
struct BootLog {
    console: Option<console::Console>,
}

impl BootLog {
    /// Writes `s` to the UART and, if present, the on-screen console. Polled
    /// MMIO never short-writes, so the UART result is ignored.
    fn str(&mut self, s: &str) {
        let _ = write(1, s.as_bytes());
        if let Some(console) = self.console.as_mut() {
            console.print(s);
        }
    }

    /// Sets the on-screen foreground pen for subsequent text. A no-op when the
    /// framebuffer is absent — the UART sink has no color.
    fn set_fg(&mut self, color: Color) {
        if let Some(console) = self.console.as_mut() {
            console.set_fg(color);
        }
    }

    /// Restores the on-screen pen to the boot foreground/background.
    fn reset_colors(&mut self) {
        if let Some(console) = self.console.as_mut() {
            console.reset_colors();
        }
    }

    /// Writes `n` as decimal through both sinks, no allocation.
    fn dec(&mut self, n: usize) {
        let mut buf = [0u8; 20];
        let mut i = buf.len();
        let mut v = n;
        loop {
            i -= 1;
            // `v % 10` is 0..=9, so the `u8` cast is value-preserving.
            #[allow(clippy::cast_possible_truncation)]
            let digit = (v % 10) as u8;
            buf[i] = b'0' + digit;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        // `buf[i..]` is all ASCII digits, so the UTF-8 decode cannot fail; the
        // fallback keeps this panic-free regardless.
        self.str(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
    }

    /// Writes `n` as a `0x`-prefixed 16-digit hex value through both sinks.
    fn hex(&mut self, n: usize) {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut buf = [0u8; 18];
        buf[0] = b'0';
        buf[1] = b'x';
        for i in 0..16 {
            // Most-significant nibble first.
            let shift = (15 - i) * 4;
            let nibble = (n >> shift) & 0xF;
            buf[2 + i] = DIGITS[nibble];
        }
        // The buffer is `0x` plus 16 hex digits — always valid ASCII/UTF-8.
        self.str(core::str::from_utf8(&buf).unwrap_or("?"));
    }
}

reovim_arch::entry!(|_argc, _argv, _envp| {
    // Acquire the framebuffer first; the geometry is captured before the
    // surface is moved into the console so it can be reported in the log.
    let fb = framebuffer::init();
    let geometry = fb
        .as_ref()
        .map(|fb| (fb.width(), fb.height(), fb.base(), fb.pitch()));

    let mut log = BootLog {
        console: fb.map(|fb| {
            // Light text on a dark slate background — legible on an HDMI
            // capture and unmistakably "ours". The font renders at its native
            // 8x16 cell via coverage software-blend.
            console::Console::new(
                fb,
                BOOT_FONT,
                Color::Rgb(rgb(0xC8, 0xE0, 0xFF)),
                Color::Rgb(rgb(0x0A, 0x14, 0x28)),
            )
        }),
    };

    log.str("reovim bare-metal boot log\n");
    log.str("uart ok\n");

    if let Some((width, height, base, pitch)) = geometry {
        log.str("mbox fb: ");
        log.dec(width as usize);
        log.str("x");
        log.dec(height as usize);
        log.str("x32 @ ");
        log.hex(base);
        log.str(" pitch ");
        log.dec(pitch as usize);
        log.str("\n");
        log.str("framebuffer console up; boot log on screen\n");
        log.str("font: ");
        log.str(BOOT_FONT.name());
        log.str("\n");
    } else {
        log.str("mbox fb: alloc failed; uart only\n");
    }
    // Color demo: render distinct truecolor and indexed-palette segments so a
    // screendump proves the pen resolves both color forms. The pen change is
    // console-only; the UART sink sees the words in its single color.
    log.str("color: ");
    log.set_fg(Color::Rgb(0x00FF_5C57));
    log.str("truecolor ");
    log.set_fg(Color::Indexed(46));
    log.str("green ");
    log.set_fg(Color::Indexed(33));
    log.str("blue ");
    log.set_fg(Color::Indexed(244));
    log.str("gray");
    log.reset_colors();
    log.str("\n");

    log.str("entering wfe loop\n");

    loop {
        // SAFETY: `wfe` is an unprivileged hint that parks the core until an
        // event; it touches no memory and has no architectural side effect
        // beyond the wait. The surface stays resident for screendump capture.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
});
