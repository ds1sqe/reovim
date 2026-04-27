//! [`HalfBlockRasterizer`] — 2 stacked logical cells → 1 terminal cell
//! via `▀` / `▄` / space.
//!
//! Chrome modules receive a [`TuiPlatformCapabilities`] reporting the
//! **logical** grid height (terminal height × 2), not the terminal
//! height. Bottom-docked chrome lands at the logical bottom row, which
//! rasterizes to the terminal-bottom row.
//!
//! ## Encoding policy
//!
//! For each pair of stacked logical cells at (top = `(x, 2y)`, bot =
//! `(x, 2y+1)`), compute `visible_color_of(top)` and
//! `visible_color_of(bot)` and emit one terminal cell at `(x, y)` by:
//!
//! | Top color      | Bot color      | Char | Style                       |
//! |----------------|----------------|------|-----------------------------|
//! | Default        | Default        | ` `  | plain                       |
//! | C (same)       | C (same)       | ` `  | bg = C                      |
//! | C              | Default        | `▀`  | fg = C                      |
//! | Default        | C              | `▄`  | fg = C                      |
//! | C1             | C2 (differ)    | `▀`  | fg = C1, bg = C2            |
//!
//! Attributes are not carried through — half-block cells are graphical,
//! not textual. The top layer's character style is discarded; only its
//! *visible color* (fg with bg fallback) participates.
//!
//! Odd logical heights: the final logical row has a real top but a
//! synthetic default-blank bottom (treated as `CellColor::Default`).
//!
//! [`TuiPlatformCapabilities`]: reovim_ext_client_tui_cap_cell

use reovim_ext_client_tui_cap_cell::{Cell, CellCapability, CellColor, CellStyle};

use crate::{
    raster::{RasterCell, RasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

/// Upper half block, `▀` (U+2580).
pub(crate) const HALF_UPPER: char = '\u{2580}';
/// Lower half block, `▄` (U+2584).
pub(crate) const HALF_LOWER: char = '\u{2584}';

/// Rasterizer for [`ViewHint::HalfBlock`].
///
/// Stateless. One instance per client is enough.
#[derive(Debug, Default, Clone, Copy)]
pub struct HalfBlockRasterizer;

impl HalfBlockRasterizer {
    /// Construct a new rasterizer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ViewRasterizer for HalfBlockRasterizer {
    fn rasterize(&self, grid: &CellCapability, out: &mut dyn RasterOutput) {
        let (w_out, h_out) = out.size();
        let w = grid.width().min(w_out);
        let h_pairs = grid.height().div_ceil(2);
        let h_term = h_pairs.min(h_out);
        for y in 0..h_term {
            let top_y = y.saturating_mul(2);
            let bot_y = top_y.saturating_add(1);
            for x in 0..w {
                let top = grid.get_cell(x, top_y);
                let bot = grid.get_cell(x, bot_y);
                let (ch, style) = encode_pair(top, bot);
                out.set_cell(RasterCell { x, y, ch, style });
            }
        }
    }

    fn hint(&self) -> ViewHint {
        ViewHint::HalfBlock
    }
}

/// What color a cell *looks like* when rendered at half-height.
///
/// - Space glyphs read as their background (or Default if bg is None).
/// - Non-space glyphs read as their foreground, falling back to
///   background if fg is None, and finally to Default.
pub(crate) fn visible_color_of(cell: &Cell) -> CellColor {
    if cell.ch == ' ' {
        cell.style.bg.unwrap_or(CellColor::Default)
    } else {
        cell.style
            .fg
            .or(cell.style.bg)
            .unwrap_or(CellColor::Default)
    }
}

/// Encode a pair of stacked cells into one terminal cell. `None` on
/// either side (out-of-bounds in odd grids, or missing rows in a
/// degenerate grid) is treated as a default-blank cell.
fn encode_pair(top: Option<&Cell>, bot: Option<&Cell>) -> (char, CellStyle) {
    let t = top.map_or(CellColor::Default, visible_color_of);
    let b = bot.map_or(CellColor::Default, visible_color_of);
    match (t, b) {
        (CellColor::Default, CellColor::Default) => (' ', CellStyle::plain()),
        (a, b) if a == b => (' ', CellStyle::plain().with_bg(a)),
        (a, CellColor::Default) => (HALF_UPPER, CellStyle::plain().with_fg(a)),
        (CellColor::Default, b) => (HALF_LOWER, CellStyle::plain().with_fg(b)),
        (a, b) => (HALF_UPPER, CellStyle::plain().with_fg(a).with_bg(b)),
    }
}
