//! [`BrailleRasterizer`] — 2×4 logical sub-grid → 1 terminal cell via
//! the Unicode Braille Patterns block (U+2800..U+28FF).
//!
//! Each terminal cell covers a 2×4 sub-grid of the logical
//! [`CellCapability`]. The eight logical cells at
//! `(2 * x_term + dx, 4 * y_term + dy)` for `dx ∈ {0, 1}` and
//! `dy ∈ {0..=3}` are tested for "visible content"
//! (`visible_color_of(cell) != CellColor::Default`) and their on/off
//! bits are combined into an 8-bit mask. The emitted glyph is
//! `'⠀' + mask` (U+2800 + mask), so a fully-on 2×4 sub-grid emits
//! `⣿` (U+28FF) and an empty one degrades to a plain space.
//!
//! ## Dot-to-bit mapping
//!
//! Unicode Braille places the four right-column dots in the high
//! nibble rather than interleaving with the left column, and the
//! fourth row of dots (dots 7 and 8) at bit positions 6 and 7 rather
//! than 3 and 7. The table encodes that irregularity:
//!
//! ```text
//! logical (dx, dy) → bit
//!
//!  (0, 0) → 0x01        (1, 0) → 0x08
//!  (0, 1) → 0x02        (1, 1) → 0x10
//!  (0, 2) → 0x04        (1, 2) → 0x20
//!  (0, 3) → 0x40        (1, 3) → 0x80
//! ```
//!
//! ## Color policy
//!
//! Braille glyphs encode only one foreground color per terminal cell
//! and carry no intrinsic background. The rasterizer picks the first
//! non-`Default` [`visible_color_of`] result in scan order
//! `(0,0) → (1,0) → (0,1) → (1,1) → (0,2) → (1,2) → (0,3) → (1,3)` as
//! the glyph's foreground. When every dot is off, the cell degrades
//! to `' '` with a plain style. Attributes are discarded — Braille
//! cells are graphical, not textual.
//!
//! Chrome modules are handed a `TuiPlatformCapabilities` reporting the
//! *logical* grid size `(width * 2, height * 4)`, not the physical
//! terminal size. The caps-doubling invariant documented at the
//! `HalfBlock` seam in `docs/architecture/client/rendering.md` extends
//! to the Braille factor: `width * 2`, `height * 4`.

use reovim_ext_client_tui_cap_cell::{CellCapability, CellColor, CellStyle};

use crate::{
    half_block::visible_color_of,
    raster::{RasterCell, RasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

/// Blank Braille Pattern (U+2800). Not emitted as-is — the rasterizer
/// degrades empty sub-grids to `' '` to stay byte-compatible with the
/// `HalfBlock` blank-cell contract.
pub(crate) const BRAILLE_BLANK: char = '\u{2800}';

/// `DOT_BITS[dx][dy]` gives the glyph bit set by the logical cell at
/// offset `(dx, dy)` within a 2×4 Braille sub-grid.
pub(crate) const DOT_BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// Rasterizer for [`ViewHint::Braille`].
///
/// Stateless. One instance per client is enough.
#[derive(Debug, Default, Clone, Copy)]
pub struct BrailleRasterizer;

impl BrailleRasterizer {
    /// Construct a new rasterizer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ViewRasterizer for BrailleRasterizer {
    fn rasterize(&self, grid: &CellCapability, out: &mut dyn RasterOutput) {
        let (w_out, h_out) = out.size();
        let w_term = grid.width().div_ceil(2).min(w_out);
        let h_term = grid.height().div_ceil(4).min(h_out);
        for y in 0..h_term {
            for x in 0..w_term {
                let mask = dot_mask_of_subgrid(grid, x, y);
                let (ch, style) = if mask == 0 {
                    (' ', CellStyle::plain())
                } else {
                    let fg = dominant_fg(grid, x, y).unwrap_or(CellColor::Default);
                    let glyph = char::from_u32(u32::from(BRAILLE_BLANK) + u32::from(mask))
                        .expect("0x2800..=0x28FF is a valid Unicode range with no surrogates");
                    (glyph, CellStyle::plain().with_fg(fg))
                };
                out.set_cell(RasterCell { x, y, ch, style });
            }
        }
    }

    fn hint(&self) -> ViewHint {
        ViewHint::Braille
    }
}

/// Compute the 8-bit Braille dot mask for the sub-grid anchored at
/// terminal coordinate `(x_term, y_term)`. Cells whose
/// [`visible_color_of`] returns [`CellColor::Default`] — including
/// out-of-bounds cells, which read as synthetic defaults — contribute
/// a zero bit.
pub(crate) fn dot_mask_of_subgrid(grid: &CellCapability, x_term: u16, y_term: u16) -> u8 {
    let mut mask: u8 = 0;
    let base_x = x_term.saturating_mul(2);
    let base_y = y_term.saturating_mul(4);
    for dx in 0..2u16 {
        for dy in 0..4u16 {
            let gx = base_x.saturating_add(dx);
            let gy = base_y.saturating_add(dy);
            if let Some(cell) = grid.get_cell(gx, gy)
                && visible_color_of(cell) != CellColor::Default
            {
                mask |= DOT_BITS[dx as usize][dy as usize];
            }
        }
    }
    mask
}

/// Return the first non-[`CellColor::Default`] [`visible_color_of`] in
/// the 2×4 sub-grid anchored at `(x_term, y_term)`, scanned row-major
/// over dots `(0,0), (1,0), (0,1), (1,1), (0,2), (1,2), (0,3), (1,3)`.
/// `None` when every cell is a default blank.
pub(crate) fn dominant_fg(grid: &CellCapability, x_term: u16, y_term: u16) -> Option<CellColor> {
    let base_x = x_term.saturating_mul(2);
    let base_y = y_term.saturating_mul(4);
    for dy in 0..4u16 {
        for dx in 0..2u16 {
            let gx = base_x.saturating_add(dx);
            let gy = base_y.saturating_add(dy);
            if let Some(cell) = grid.get_cell(gx, gy) {
                let c = visible_color_of(cell);
                if c != CellColor::Default {
                    return Some(c);
                }
            }
        }
    }
    None
}
