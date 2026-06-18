//! ANSI/VT terminal colors for the framebuffer console.
//!
//! [`Color`] names a color the way a terminal stream does: either a literal
//! 24-bit truecolor or an index into the 256-entry xterm palette. Both resolve
//! to the framebuffer's `0x00RRGGBB` pixel through [`Color::resolve`], so the
//! console blends through one path regardless of how a color was named.
//!
//! This module lives under `none_aarch64/` because the framebuffer console is
//! its only consumer — nothing else in `arch` deals in cell colors. The
//! palette is platform-neutral spec data (the xterm 256-color layout) and
//! carries no platform assumptions, so promoting this module if a second
//! framebuffer target ever appears is a move, not a rewrite — the same call as
//! the sibling [`super::fonts`] module.

/// A console color: a literal 24-bit truecolor, or an index into the xterm
/// 256-color palette.
///
/// Both forms [`resolve`](Color::resolve) to a `0x00RRGGBB` framebuffer pixel,
/// so the blit treats them uniformly. The two variants are the two ways an
/// ANSI/VT stream names a color (SGR `38;2;r;g;b` truecolor and `38;5;n`
/// indexed), which is why both exist from the first flight that has a color at
/// all.
#[derive(Clone, Copy)]
pub enum Color {
    /// Packed 24-bit truecolor. The byte above the low 24 bits is ignored on
    /// [`resolve`](Color::resolve), so a stray alpha byte never leaks into a
    /// channel.
    Rgb(u32),
    /// Index into the 256-color palette: `0..=15` the named ANSI colors,
    /// `16..=231` the 6×6×6 color cube, `232..=255` the grayscale ramp.
    Indexed(u8),
}

impl Color {
    /// Resolves to the framebuffer's `0x00RRGGBB` pixel. `Rgb` masks off the
    /// top byte; `Indexed` looks the entry up in the 256-color palette. Every
    /// `u8` index resolves, so there is no failure path.
    #[must_use]
    pub const fn resolve(self) -> u32 {
        match self {
            Self::Rgb(v) => v & 0x00FF_FFFF,
            Self::Indexed(i) => palette(i),
        }
    }
}

/// The 16 named ANSI colors (xterm defaults) as `0x00RRGGBB`. Indices `0..=7`
/// are the normal colors, `8..=15` their bright variants.
const ANSI_16: [u32; 16] = [
    0x0000_0000, // 0  black
    0x0080_0000, // 1  red
    0x0000_8000, // 2  green
    0x0080_8000, // 3  yellow
    0x0000_0080, // 4  blue
    0x0080_0080, // 5  magenta
    0x0000_8080, // 6  cyan
    0x00C0_C0C0, // 7  white
    0x0080_8080, // 8  bright black (gray)
    0x00FF_0000, // 9  bright red
    0x0000_FF00, // 10 bright green
    0x00FF_FF00, // 11 bright yellow
    0x0000_00FF, // 12 bright blue
    0x00FF_00FF, // 13 bright magenta
    0x0000_FFFF, // 14 bright cyan
    0x00FF_FFFF, // 15 bright white
];

/// Resolves a 256-color palette index to its `0x00RRGGBB` pixel over the three
/// standard xterm ranges: the named table, the 6×6×6 cube, and the grayscale
/// ramp. Pure integer arithmetic — every index is total, no panic path.
const fn palette(index: u8) -> u32 {
    if index < 16 {
        ANSI_16[index as usize]
    } else if index < 232 {
        // 6×6×6 cube: the offset into the cube decomposes into base-6
        // (r, g, b) components, each mapped through the nonlinear level ramp.
        let offset = (index as u32) - 16;
        let r = cube_level(offset / 36);
        let g = cube_level((offset / 6) % 6);
        let b = cube_level(offset % 6);
        (r << 16) | (g << 8) | b
    } else {
        // Grayscale ramp: 24 steps from 8 to 238 in increments of 10, the same
        // level on all three channels.
        let level = 8 + ((index as u32) - 232) * 10;
        (level << 16) | (level << 8) | level
    }
}

/// One channel level of the 6×6×6 color cube: a zero component is pure `0`,
/// every other component `c` is `55 + 40·c` (xterm's nonlinear ramp, so the
/// cube's darkest non-zero step is `0x5F`, not `0x33`).
const fn cube_level(c: u32) -> u32 {
    if c == 0 { 0 } else { 55 + 40 * c }
}

#[cfg(feature = "selftest")]
#[path = "color_tests.rs"]
mod color_tests;
