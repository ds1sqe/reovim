//! Tests for [`Color`] palette resolution, compiled into the lib under
//! `selftest` and run by the `arch-selftest` no_std runner.
//!
//! Every branch of the palette is pinned to an exact `0x00RRGGBB` at a named
//! index: the ANSI-16 table endpoints, both ends and the per-channel split of
//! the 6×6×6 cube's level ramp, and the grayscale ramp at its ends and
//! midpoint. The truecolor arm asserts the top byte is masked off.

use {
    super::Color,
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(color_indexed_ansi16_endpoints, {
    // First and last named entries bracket the `i < 16` table lookup.
    testrt::check_eq(Color::Indexed(0).resolve(), 0x0000_0000u32);
    testrt::check_eq(Color::Indexed(15).resolve(), 0x00FF_FFFFu32);
});

arch_test!(color_indexed_cube_branches, {
    // n = 0: every channel takes the `c == 0` arm → pure black.
    testrt::check_eq(Color::Indexed(16).resolve(), 0x0000_0000u32);
    // n = 215 → (5, 5, 5): every channel takes the `c != 0` arm, level 255.
    testrt::check_eq(Color::Indexed(231).resolve(), 0x00FF_FFFFu32);
    // n = 180 → (5, 0, 0): r takes `c != 0` while g and b take `c == 0`,
    // proving the per-channel branch resolves both ways within one entry.
    testrt::check_eq(Color::Indexed(196).resolve(), 0x00FF_0000u32);
});

arch_test!(color_indexed_grayscale_ramp, {
    // Endpoints and midpoint of the linear `8 + (i - 232) * 10` ramp.
    testrt::check_eq(Color::Indexed(232).resolve(), 0x0008_0808u32);
    testrt::check_eq(Color::Indexed(243).resolve(), 0x0076_7676u32);
    testrt::check_eq(Color::Indexed(255).resolve(), 0x00EE_EEEEu32);
});

arch_test!(color_rgb_masks_top_byte, {
    // The byte above the low 24 bits is stripped — no channel leak even when
    // its high bit is set.
    testrt::check_eq(Color::Rgb(0xFF12_3456).resolve(), 0x0012_3456u32);
});
