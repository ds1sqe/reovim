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

// The pen tests below blit through Terminus deliberately: its coverage is pure
// `0x00`/`0xFF`, so a full-coverage ink pixel resolves to *exactly* the pen's
// color and a background pixel to *exactly* the bg pen — equality is precise,
// with no anti-aliased edge values to reason about.

arch_test!(console_pen_indexed_blits_palette_rgb, {
    // An indexed pen resolves through the palette and feeds the blend: index
    // 196 is the cube's pure red (`0x00FF0000`).
    let mut buf = [0u32; CELL_PIXELS];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 8, 16, 8 * 4);
    let mut console = Console::new(fb, &TERMINUS, Color::Rgb(FG), Color::Rgb(BG));
    console.set_fg(Color::Indexed(196));
    console.print("M");
    testrt::check(buf.iter().any(|&p| p == 0x00FF_0000), "indexed pen paints palette-red ink");
    testrt::check(buf.iter().any(|&p| p == BG), "background stays the bg pen");
});

arch_test!(console_pen_rgb_blits_truecolor, {
    // A truecolor pen reaches the surface unchanged at full coverage.
    let mut buf = [0u32; CELL_PIXELS];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 8, 16, 8 * 4);
    let mut console = Console::new(fb, &TERMINUS, Color::Rgb(FG), Color::Rgb(BG));
    console.set_fg(Color::Rgb(0x00AB_CDEF));
    console.print("M");
    testrt::check(buf.iter().any(|&p| p == 0x00AB_CDEF), "rgb pen paints truecolor ink");
});

arch_test!(console_pen_change_affects_only_subsequent_cells, {
    // Two cells side by side: cell A under a red pen, then the pen switches to
    // green for cell B. Immediate-mode means cell A is never repainted, so its
    // ink must stay red while only cell B is green. The 16-wide surface holds
    // two 8-px cells (cols 0 and 1) on one row.
    let mut buf = [0u32; 16 * 16];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 16, 16, 16 * 4);
    let mut console = Console::new(fb, &TERMINUS, Color::Rgb(FG), Color::Rgb(BG));
    console.set_fg(Color::Rgb(0x00FF_0000));
    console.print("M"); // cell A, col 0
    console.set_fg(Color::Rgb(0x0000_FF00));
    console.print("M"); // cell B, col 1

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

arch_test!(console_reset_colors_restores_default, {
    // After a `set_fg`, `reset_colors` returns the pen to the `new`-time fg.
    let mut buf = [0u32; CELL_PIXELS];
    let fb = Framebuffer::over_region(buf.as_mut_ptr() as usize, 8, 16, 8 * 4);
    let mut console = Console::new(fb, &TERMINUS, Color::Rgb(FG), Color::Rgb(BG));
    console.set_fg(Color::Rgb(0x0000_FF00));
    console.reset_colors();
    console.print("M");
    testrt::check(buf.iter().any(|&p| p == FG), "reset restores the default fg ink");
    testrt::check(!buf.iter().any(|&p| p == 0x0000_FF00), "no green ink after reset");
});
