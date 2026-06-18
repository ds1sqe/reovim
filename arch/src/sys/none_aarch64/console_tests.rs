//! Tests for the coverage blit and color blend, compiled into the lib under
//! `selftest` and run by the `arch-selftest` no_std runner.
//!
//! The blend math is asserted directly at named `(fg, bg, coverage)` triples.
//! The blit is exercised by rendering into a caller-owned memory region via
//! [`Framebuffer::over_region`] — a "stub surface" the test reads back — so
//! `draw_cell` runs end-to-end for both embedded fonts without the MMIO
//! framebuffer.

use {
    super::{
        super::{
            color::Color,
            fonts::{Font, JETBRAINS_MONO, TERMINUS},
            framebuffer::Framebuffer,
        },
        Console, blend,
    },
    crate::{arch_test, testrt},
};

/// Opaque-white foreground over a distinct dark background, so a rendered
/// pixel is unambiguously distinguishable from the cleared cell.
const FG: u32 = 0x00FF_FFFF;
const BG: u32 = 0x0010_2030;

/// One 8x16 cell worth of `u32` pixels — the surface every blit test renders
/// into. Matches both fonts' cell geometry.
const CELL_PIXELS: usize = 8 * 16;

/// Resolved palette pixels the SGR tests assert against: `Indexed(1)`/`SGR 31`
/// normal red, `Indexed(2)`/`SGR 42` normal green, and `Indexed(9)`/`SGR 91`
/// bright red (also cube index 196, via `SGR 38;5;196`).
const PALETTE_RED: u32 = 0x0080_0000;
const PALETTE_GREEN: u32 = 0x0000_8000;
const BRIGHT_RED: u32 = 0x00FF_0000;

/// Renders `s` into a fresh one-cell stub surface through `font` and returns
/// the backing pixels (row-major, `8 * 16`).
fn render(font: &'static Font, s: &str) -> [u32; CELL_PIXELS] {
    let mut buf = [0u32; CELL_PIXELS];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 8, 16, 8 * 4);
    let mut console = Console::new(fb, font, Color::Rgb(FG), Color::Rgb(BG));
    console.print(s);
    buf
}

arch_test!(console_blend_named_triples, {
    // Endpoints: zero coverage is pure background, full coverage is pure
    // foreground (the `+127` rounding still lands exactly on each endpoint).
    testrt::check_eq(blend(0x00FF_FFFF, 0x0000_0000, 0), 0x0000_0000u32);
    testrt::check_eq(blend(0x00FF_FFFF, 0x0000_0000, 255), 0x00FF_FFFFu32);
    // Mid coverage exercises the rounding boundary in both directions:
    // white-over-black at cov=128 rounds up to 0x80; black-over-white at
    // cov=128 rounds to 0x7F.
    testrt::check_eq(blend(0x00FF_FFFF, 0x0000_0000, 128), 0x0080_8080u32);
    testrt::check_eq(blend(0x0000_0000, 0x00FF_FFFF, 128), 0x007F_7F7Fu32);
});

arch_test!(console_blit_renders_blank_and_ink, {
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        // A space glyph is all-background: the whole cell stays `BG`.
        let space = render(font, " ");
        testrt::check(space.iter().all(|&p| p == BG), "space cell is all background");
        // A letter renders ink: at least one pixel differs from the background.
        let letter = render(font, "M");
        testrt::check(letter.iter().any(|&p| p != BG), "letter M paints some ink");
    }
});

arch_test!(console_blit_out_of_range_is_blank, {
    // A byte outside the font range resolves to the blank cell, so the surface
    // stays all background — no panic, no out-of-bounds, no stray ink.
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        let cell = render(font, "\u{01}");
        testrt::check(cell.iter().all(|&p| p == BG), "out-of-range byte renders blank");
    }
});

// The SGR tests below drive the console the way a terminal does — escape
// sequences embedded in the printed stream, not imperative pen calls — and
// blit through Terminus deliberately: its coverage is pure `0x00`/`0xFF`, so a
// full-coverage ink pixel resolves to *exactly* the pen's color and a
// background pixel to *exactly* the bg pen, with no anti-aliased edge values.

arch_test!(console_sgr_indexed_fg_blits_palette, {
    // Both the named form (`31`) and the 256-index form (`38;5;196`) resolve
    // through the palette to the blitted ink.
    let named = render(&TERMINUS, "\x1b[31mM");
    testrt::check(named.iter().any(|&p| p == PALETTE_RED), "SGR 31 paints palette red");
    let indexed = render(&TERMINUS, "\x1b[38;5;196mM");
    testrt::check(indexed.iter().any(|&p| p == BRIGHT_RED), "SGR 38;5;196 paints cube red");
});

arch_test!(console_sgr_bright_fg_blits_palette, {
    // The bright range (`90`–`97`) maps to palette indices 8–15.
    let buf = render(&TERMINUS, "\x1b[91mM");
    testrt::check(buf.iter().any(|&p| p == BRIGHT_RED), "SGR 91 paints bright red (index 9)");
});

arch_test!(console_sgr_truecolor_fg, {
    // `38;2;r;g;b` reaches the surface as the exact 24-bit color.
    let buf = render(&TERMINUS, "\x1b[38;2;171;205;239mM");
    testrt::check(buf.iter().any(|&p| p == 0x00AB_CDEF), "SGR 38;2 paints the exact truecolor");
});

arch_test!(console_sgr_background_fills_cell, {
    // A bg code repaints the cell's background pixels (coverage 0) while the
    // default fg still inks the glyph — exercises the reinstated `set_bg`.
    let named = render(&TERMINUS, "\x1b[42mM");
    testrt::check(named.iter().any(|&p| p == PALETTE_GREEN), "SGR 42 fills the cell background");
    testrt::check(named.iter().any(|&p| p == FG), "the default fg still inks the glyph");
    let truecolor = render(&TERMINUS, "\x1b[48;2;171;205;239mM");
    testrt::check(
        truecolor.iter().any(|&p| p == 0x00AB_CDEF),
        "SGR 48;2 fills a truecolor background",
    );
});

arch_test!(console_sgr_default_and_reset_restore_pens, {
    // `39` restores the default fg; `0` restores both pens (the reinstated
    // `default_bg`).
    let dflt_fg = render(&TERMINUS, "\x1b[31m\x1b[39mM");
    testrt::check(dflt_fg.iter().any(|&p| p == FG), "SGR 39 restores the default fg");
    testrt::check(!dflt_fg.iter().any(|&p| p == PALETTE_RED), "no red remains after SGR 39");
    let reset = render(&TERMINUS, "\x1b[31;42m\x1b[0mM");
    testrt::check(reset.iter().any(|&p| p == FG), "SGR 0 restores the default fg");
    testrt::check(reset.iter().any(|&p| p == BG), "SGR 0 restores the default bg");
    testrt::check(!reset.iter().any(|&p| p == PALETTE_GREEN), "no custom bg remains after SGR 0");
});

arch_test!(console_sgr_ignores_noncolor_attribute, {
    // `1` (bold) is a non-color attribute the console does not render yet: it
    // is consumed without moving the pen and without panicking.
    let buf = render(&TERMINUS, "\x1b[1mM");
    testrt::check(buf.iter().any(|&p| p == FG), "non-color SGR leaves the fg pen at default");
    testrt::check(buf.iter().any(|&p| p == BG), "non-color SGR leaves the bg pen at default");
});

arch_test!(console_sgr_malformed_extended_leaves_pen, {
    // A `38` with no mode, a `38;5` with no index, and a short `38;2` triple
    // are all malformed extended-color introducers: the pen must not move.
    for seq in ["\x1b[38mM", "\x1b[38;5mM", "\x1b[38;2;1;2mM"] {
        let buf = render(&TERMINUS, seq);
        testrt::check(
            buf.iter().any(|&p| p == FG),
            "malformed extended color leaves the fg pen at default",
        );
    }
});

arch_test!(console_sgr_pen_change_affects_only_subsequent_cells, {
    // Two cells: cell A printed under a red pen, then SGR switches to green for
    // cell B. Immediate-mode means cell A is never repainted. The 16-wide
    // surface holds two 8-px cells (cols 0 and 1) on one row.
    let mut buf = [0u32; 16 * 16];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 16, 16, 16 * 4);
    let mut console = Console::new(fb, &TERMINUS, Color::Rgb(FG), Color::Rgb(BG));
    console.print("\x1b[38;2;255;0;0mM"); // cell A, col 0 (red)
    console.print("\x1b[38;2;0;255;0mM"); // cell B, col 1 (green)

    let (mut a_red, mut a_green, mut b_green) = (false, false, false);
    for y in 0..16 {
        for x in 0..16 {
            let p = buf[y * 16 + x];
            if x < 8 {
                a_red |= p == 0x00FF_0000;
                a_green |= p == 0x0000_FF00;
            } else {
                b_green |= p == 0x0000_FF00;
            }
        }
    }
    testrt::check(a_red, "cell A keeps the original red pen");
    testrt::check(!a_green, "cell A was not repainted by the later green pen");
    testrt::check(b_green, "cell B uses the new green pen");
});

arch_test!(console_sgr_integration_colored_then_reset, {
    // Integration smoke: a colored glyph then a reset, end to end — parser,
    // SGR dispatch, pen, and blit composing across two cells.
    let mut buf = [0u32; 16 * 16];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 16, 16, 16 * 4);
    let mut console = Console::new(fb, &TERMINUS, Color::Rgb(FG), Color::Rgb(BG));
    console.print("\x1b[31mA\x1b[0mB");

    let (mut a_red, mut b_default) = (false, false);
    for y in 0..16 {
        for x in 0..16 {
            let p = buf[y * 16 + x];
            if x < 8 {
                a_red |= p == PALETTE_RED;
            } else {
                b_default |= p == FG;
            }
        }
    }
    testrt::check(a_red, "the colored cell A blits palette red");
    testrt::check(b_default, "after SGR 0, cell B blits the default fg");
});
