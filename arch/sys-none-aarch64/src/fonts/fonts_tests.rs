//! Tests for the [`super::Font`] descriptor's glyph selection, compiled into
//! the lib under `selftest` and run by the `arch-selftest` no_std runner (the
//! binary the freestanding coverage measurement is taken from).
//!
//! These exercise the pure selection path for both embedded fonts:
//! - in-range bytes index real coverage (both font arms),
//! - the below-`first` and above-`last` fallback arms both resolve to the
//!   blank space cell without indexing out of bounds.

use {
    super::{JETBRAINS_MONO, TERMINUS},
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(fonts_glyph_space_cell_is_blank, {
    // The first cell (space) is all-background, and the returned slice is
    // exactly one cell wide — for both fonts.
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        let cell = font.glyph(b' ');
        testrt::check_eq(cell.len(), (font.cell_w * font.cell_h) as usize);
        testrt::check(cell.iter().all(|&c| c == 0), "space cell is blank");
    }
});

arch_test!(fonts_glyph_in_range_has_ink, {
    // A letter inside `first..=last` renders some coverage (in-range arm),
    // proving the index lands on real glyph data for both fonts.
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        let cell = font.glyph(b'M');
        testrt::check(cell.iter().any(|&c| c != 0), "letter M has ink");
    }
});

arch_test!(fonts_glyph_out_of_range_falls_back_to_blank, {
    // Bytes below `first` (0x20) and above `last` (0x7F) both fall back to the
    // blank space cell — covering each side of the range decision — and never
    // index past the table.
    for font in [&JETBRAINS_MONO, &TERMINUS] {
        let blank = font.glyph(b' ');
        let below = font.glyph(0x01);
        let above = font.glyph(0xFF);
        testrt::check(below == blank, "below-range byte renders the blank cell");
        testrt::check(above == blank, "above-range byte renders the blank cell");
    }
});
