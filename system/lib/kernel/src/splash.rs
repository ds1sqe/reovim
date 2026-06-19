//! The reovim boot splash content (aarch64 bare metal).
//!
//! The branded splash — the centred `r e o v i m` wordmark, the accent rule,
//! the identity line, the compact boot panel reporting the proven floor facts,
//! and the one-line SGR diagnostics — lifted out of the splash fixture (SP04
//! 04c). Splash *content* (text, colours, layout) is policy, so it lives here in
//! the system kernel, not in the raw-mechanism floor below.
//!
//! It renders through the **standing console** (04b): every byte is written to
//! fd 1 via the floor's `write`, which fans it to the PL011 UART *and* — once
//! [`install_console`](crate::console::install_console) has registered the
//! framebuffer console trampoline — to the HDMI surface. The accent rule is a
//! reverse-video SGR span, not direct pixel writes, so every splash pixel stays
//! inside the console's blit path with no new floor primitive. There is no
//! second `Console` and no `framebuffer::init` here: the standing console owns
//! the surface, and the splash reaches it the same way the kernel's boot-stage
//! log does — through fd 1.

use reovim_arch_sys_none_aarch64::write;

/// Truecolor SGR selecting the brand accent pen (sky blue) — wordmark + rule.
const ACCENT: &str = "\x1b[38;2;90;210;255m";
/// SGR reverse-video on: a space row painted under reverse fills with the pen.
const REVERSE: &str = "\x1b[7m";
/// SGR full reset: pens back to the console defaults, all attributes cleared.
const RESET: &str = "\x1b[0m";

/// Console column count on the fixed 1280-px splash surface (`1280 / CELL_W` at
/// the boot font's 8-px cell), the basis for horizontal centring. The console
/// clamps to this internally, so an over-estimate only shifts a line right; it
/// never writes out of bounds.
const COLS: usize = 160;

/// The splash writer: emits every byte to fd 1, which the floor's `write` fans
/// to the UART plus the standing framebuffer console (when one is installed).
///
/// Zero-sized — the sink is the process-wide fd, not owned state — so the splash
/// holds no framebuffer or console; the standing console (04b) does. All helpers
/// are allocation-free (fixed stack buffers + slices of a static blank run), so
/// the splash needs no heap and no `alloc` crate.
struct Splash;

impl Splash {
    /// Writes `s` to fd 1. Polled MMIO never short-writes and the console sink
    /// is total, so the result is ignored.
    fn str(&mut self, s: &str) {
        let _ = write(1, s.as_bytes());
    }

    /// Writes `n` as decimal through fd 1, no allocation.
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

    /// Emits `n` spaces through fd 1 in bounded chunks (no allocation). On the
    /// console these advance the cursor; under [`REVERSE`] they paint a solid
    /// pen-filled run.
    fn spaces(&mut self, n: usize) {
        const BLANK: &str =
            "                                                                                ";
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

/// Renders the branded boot splash through the standing console.
///
/// `geometry` is the framebuffer's `(width, height)` captured by the caller
/// *before* it handed the surface to
/// [`install_console`](crate::console::install_console) — `Some` when a display
/// came up, `None` on the degraded UART-only path (no mailbox framebuffer). The
/// dimensions only feed the boot panel's reported facts; the splash text routes
/// through fd 1 either way, so the same call renders to the serial line whether
/// or not a console is installed.
///
/// Call once, after the standing console is installed and after the kernel's
/// boot-stage log has streamed (so the splash sits below it on screen). The
/// fixed indents below are derived from the 160-column / 36-row surface; a
/// surface the console clamps smaller only shifts a line, never writes out of
/// bounds.
pub fn render(geometry: Option<(u32, u32)>) {
    let mut splash = Splash;

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
}
