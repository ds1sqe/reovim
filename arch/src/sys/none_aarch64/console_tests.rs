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
    let mut console = Console::new(fb, font, FG, BG);
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
