//! Bare-metal aarch64 boot splash (QEMU raspi4b / BCM2711).
//!
//! Boots through the arch `_start`, asks the `VideoCore` for the fixed
//! 1280x720x32 framebuffer over the mailbox property interface, wraps it in a
//! [`console::Console`] rendering [`fonts::JETBRAINS_MONO`] (anti-aliased) via
//! coverage software-blend, and paints a deliberate reovim boot splash —
//! centred wordmark, accent rule, identity line, and a compact boot panel
//! reporting the proven floor facts — onto the HDMI surface while echoing the
//! same bytes to the PL011 UART. It then parks in a `wfe` loop forever — it
//! deliberately does NOT semihosting-exit, so the rendered surface persists for
//! a QEMU screendump / VNC capture. This is the floor's branded proof of life
//! on screen, not only on the serial line.
//!
//! Splash *content* (text, colours, layout) is policy and lives here in the
//! payload; the floor exposes only mechanism (`framebuffer`, `console`,
//! `fonts`, `color`). The accent rule is a reverse-video SGR span, not direct
//! pixel writes: the console consumes the framebuffer by value, so every splash
//! pixel stays inside the console's blit path with no new floor primitive. The
//! console parser/scroll paths are proven independently by the `console_tests`
//! `arch_test!` cases under `arch-selftest`; this fixture is the visual proof.
#![no_std]
#![no_main]
// The `entry!` macro expands to `#[unsafe(no_mangle)]` symbol declarations the
// `unsafe_code` lint flags, and the wfe park uses inline asm; a bare-metal
// payload is unsafe by nature. The allow is scoped to this fixture bin.
#![allow(unsafe_code)]

use {
    core::arch::asm,
    // The framebuffer (VideoCore MMIO) + the raw `write` floor STAY below; the
    // console / fonts / color the splash renders through lifted into the system
    // kernel with the device-neutral library (SP04 04a).
    reovim_arch::sys::{framebuffer, write},
    reovim_system_kernel::{color::Color, console, fonts},
};

/// Packs an RGB triple into a `0x00RRGGBB` pixel (matches the requested RGB
/// pixel-order tag).
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// The font the splash renders through. The 1280x720 surface at this font's
/// 8-px cell yields a 160-column console; the centring offsets below are
/// derived from that width.
const BOOT_FONT: &fonts::Font = &fonts::JETBRAINS_MONO;

/// Console column count on the fixed splash surface (`1280 / CELL_W`), the
/// basis for horizontal centring. The console clamps to this internally, so an
/// over-estimate only shifts a line right; it never writes out of bounds.
const COLS: usize = 160;

/// Truecolor SGR selecting the brand accent pen (sky blue) — wordmark + rule.
const ACCENT: &str = "\x1b[38;2;90;210;255m";
/// SGR reverse-video on: a space row painted under reverse fills with the pen.
const REVERSE: &str = "\x1b[7m";
/// SGR full reset: pens back to the console defaults, all attributes cleared.
const RESET: &str = "\x1b[0m";

/// A dual-sink splash writer: every byte goes to the PL011 UART (fd 1) and,
/// once the framebuffer is up, to the on-screen text console as well.
///
/// Before the framebuffer is acquired `console` is `None` and output is
/// UART-only — the natural degraded path when the mailbox alloc fails.
struct Splash {
    console: Option<console::Console<'static>>,
}

impl Splash {
    /// Writes `s` to the UART and, if present, the on-screen console. Polled
    /// MMIO never short-writes, so the UART result is ignored.
    fn str(&mut self, s: &str) {
        let _ = write(1, s.as_bytes());
        if let Some(console) = self.console.as_mut() {
            console.print(s);
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

    /// Emits `n` spaces through both sinks in bounded chunks (no allocation).
    /// On the console these advance the cursor; under [`REVERSE`] they paint a
    /// solid pen-filled run.
    fn spaces(&mut self, n: usize) {
        const BLANK: &str = "                                                                                ";
        let mut left = n;
        while left > 0 {
            let take = left.min(BLANK.len());
            self.str(&BLANK[..take]);
            left -= take;
        }
    }

    /// Centres `text` (known visible length, no escapes) on the 160-column
    /// surface and terminates the line.
    fn center(&mut self, text: &str) {
        self.spaces(COLS.saturating_sub(text.len()) / 2);
        self.str(text);
        self.str("\n");
    }

    /// Emits `n` blank lines.
    fn blank(&mut self, n: usize) {
        for _ in 0..n {
            self.str("\n");
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
use reovim_arch_floor_none_aarch64::entry;

entry!(|_argc, _argv, _envp| {
    // Acquire the framebuffer first; the geometry is captured before the
    // surface is moved into the console so it can be reported in the panel.
    let fb = framebuffer::init();
    let geometry = fb.as_ref().map(|fb| (fb.width(), fb.height()));

    let mut splash = Splash {
        // Light text on a dark slate background — legible on an HDMI capture
        // and unmistakably "ours". `Console::new` consumes the framebuffer and
        // clears it to the background pen, backed by the console's retained
        // grid (taken once from the lib's static store).
        console: fb.zip(console::screen_grid()).map(|(fb, grid)| {
            console::Console::new(
                fb,
                BOOT_FONT,
                Color::Rgb(rgb(0xC8, 0xE0, 0xFF)),
                Color::Rgb(rgb(0x0A, 0x14, 0x28)),
                grid,
            )
        }),
    };

    if let Some((width, height)) = geometry {
        // Push the splash block toward the vertical centre of the 36-row
        // surface.
        splash.blank(11);

        // Wordmark: letter-spaced, in the brand accent pen. Centred by its
        // visible width; the SGR bytes do not advance the cursor. Rendered
        // without bold: the console's synthetic bold thickening saturates the
        // dense `m` glyph to a solid block in the 8-px cell, so the accent pen
        // alone carries the emphasis.
        splash.spaces(COLS.saturating_sub(11) / 2);
        splash.str(ACCENT);
        splash.str("r e o v i m");
        splash.str(RESET);
        splash.str("\n");

        splash.blank(1);

        // Accent rule: a centred 30-cell reverse-video span in the accent pen.
        // The leading pad is normal background; the span itself is solid accent.
        splash.spaces((COLS - 30) / 2);
        splash.str(ACCENT);
        splash.str(REVERSE);
        splash.spaces(30);
        splash.str(RESET);
        splash.str("\n");

        splash.blank(1);
        splash.center("v0.16.0    RTOS-itself / appliance");
        splash.blank(2);

        // Boot panel: the proven floor facts, centred. Built piecewise, so the
        // indent is a fixed approximate centre rather than width-exact.
        splash.spaces(54);
        splash.str("framebuffer ");
        splash.dec(width as usize);
        splash.str("x");
        splash.dec(height as usize);
        splash.str("x32    uart ok    aarch64 BCM2711\n");

        splash.blank(1);

        // Diagnostics line: keeps one compact real-machine exercise of the SGR
        // parser+blit (truecolor + indexed colour + bold + reverse) on screen.
        // The escape bytes break visible-width centring, so this sits at a
        // fixed indent under the panel.
        splash.spaces(60);
        splash.str(
            "render: \x1b[38;2;255;92;87mtruecolor \x1b[38;5;46mindexed \
             \x1b[0m\x1b[1mbold \x1b[22m\x1b[7mreverse\x1b[0m\n",
        );
    } else {
        // Degraded path: no framebuffer, so the splash is a UART-only banner.
        splash.str("reovim v0.16.0 boot splash\n");
        splash.str("mbox fb: alloc failed; uart only\n");
    }

    loop {
        // SAFETY: `wfe` is an unprivileged hint that parks the core until an
        // event; it touches no memory and has no architectural side effect
        // beyond the wait. The surface stays resident for screendump capture.
        unsafe {
            asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
});
