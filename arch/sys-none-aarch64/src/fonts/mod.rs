//! Embedded console fonts: platform-neutral glyph-coverage data plus the
//! [`Font`] descriptor the console blits through.
//!
//! This module lives under `none_aarch64/` because the framebuffer console is
//! its only consumer — nothing else in `arch` rasterizes glyphs. The generated
//! tables ([`jetbrains_mono`], [`terminus`]) are pure data and carry no
//! platform assumptions, so promoting this module if a second framebuffer
//! target ever appears is a move, not a rewrite.
//!
//! The tables are produced offline by `arch/tests/glyph-rasterizer/` from the
//! vendored OFL fonts; see each generated file's header for provenance. Both
//! target the same `CELL_W` x `CELL_H` monospace cell over the same printable
//! range, so the console blits either through one coverage path — selecting a
//! font is choosing a descriptor.

mod jetbrains_mono;
mod terminus;

// Compile-time integrity guards on the generated tables: nonzero cell metrics
// (so the console's `cols = width / cell_w` derivation can never divide by
// zero) and an exact glyph-count-to-byte-length match (so `Font::glyph`'s
// slice indexing is always in bounds). These hold by construction of the
// generator; asserting them here turns a malformed regeneration into a build
// error rather than a runtime fault — and, being `const`, they add no runtime
// branch to cover.
const _: () = assert!(jetbrains_mono::CELL_W > 0 && jetbrains_mono::CELL_H > 0);
const _: () = assert!(terminus::CELL_W > 0 && terminus::CELL_H > 0);
const _: () = assert!(
    jetbrains_mono::GLYPHS.len()
        == (jetbrains_mono::CELL_W * jetbrains_mono::CELL_H) as usize
            * (jetbrains_mono::LAST as usize - jetbrains_mono::FIRST as usize + 1)
);
const _: () = assert!(
    terminus::GLYPHS.len()
        == (terminus::CELL_W * terminus::CELL_H) as usize
            * (terminus::LAST as usize - terminus::FIRST as usize + 1)
);

/// A monospace coverage font: a row-major grayscale-coverage bitmap per glyph
/// over a fixed cell, plus the metrics the console needs to lay it out.
///
/// `glyphs` holds `cell_w * cell_h` coverage bytes for each codepoint in
/// `first..=last`, ascending. A coverage byte is `0x00` for background and
/// `0xFF` for full foreground; intermediate values are anti-aliased edges the
/// console blends against the cell background.
///
/// Fields are crate-private so the only `Font` values are the const-guarded
/// statics below — an external caller cannot construct one with a zero cell
/// dimension, which keeps the console's cell-count division total without a
/// runtime check.
pub struct Font {
    /// Row-major coverage for `first..=last`, `cell_w * cell_h` bytes/glyph.
    pub(crate) glyphs: &'static [u8],
    /// Cell width in pixels (nonzero by const guard).
    pub(crate) cell_w: u32,
    /// Cell height in pixels (nonzero by const guard).
    pub(crate) cell_h: u32,
    /// First codepoint present in `glyphs`.
    pub(crate) first: u8,
    /// Last codepoint present in `glyphs`.
    pub(crate) last: u8,
    /// Human-facing font name.
    pub(crate) name: &'static str,
}

impl Font {
    /// The font's human-facing name (e.g. for a selection log line).
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the coverage cell for `byte`, or the first cell (space) when
    /// `byte` is outside `first..=last`. The space cell is blank, so
    /// out-of-range bytes render as a gap and the slice index stays in bounds.
    pub(crate) fn glyph(&self, byte: u8) -> &'static [u8] {
        let cell = (self.cell_w * self.cell_h) as usize;
        let index = if byte >= self.first && byte <= self.last {
            (byte - self.first) as usize
        } else {
            0
        };
        &self.glyphs[index * cell..index * cell + cell]
    }
}

/// `JetBrains` Mono — anti-aliased (SIL OFL 1.1). Coverage table and
/// provenance in [`jetbrains_mono`].
pub static JETBRAINS_MONO: Font = Font {
    glyphs: &jetbrains_mono::GLYPHS,
    cell_w: jetbrains_mono::CELL_W,
    cell_h: jetbrains_mono::CELL_H,
    first: jetbrains_mono::FIRST,
    last: jetbrains_mono::LAST,
    name: "JetBrains Mono",
};

/// Terminus — crisp bitmap (SIL OFL 1.1). Coverage table and provenance in
/// [`terminus`].
pub static TERMINUS: Font = Font {
    glyphs: &terminus::GLYPHS,
    cell_w: terminus::CELL_W,
    cell_h: terminus::CELL_H,
    first: terminus::FIRST,
    last: terminus::LAST,
    name: "Terminus",
};

// L12 layout: tests live in the sibling file `fonts_tests.rs`, declared as a
// `#[path]` child so `super::` reaches the `Font`/`JETBRAINS_MONO`/`TERMINUS`
// descriptors. The file is only compiled when this module is (the lib.rs target
// gate), so the inner declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "fonts_tests.rs"]
mod tests;
