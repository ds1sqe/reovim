//! Tests for [`HalfBlockRasterizer`] and its internal encoding helpers.
//!
//! Target: 100% line coverage. The encoding-policy matrix is covered
//! once per row via `encode_*` tests, plus integration tests that run
//! the full `rasterize` pass over canonical grids including odd-height
//! and terminal-clamp cases.

use reovim_ext_client_tui_cap_cell::{Cell, CellAttrs, CellCapability, CellColor, CellStyle};

use crate::{
    half_block::{HALF_LOWER, HALF_UPPER, HalfBlockRasterizer, visible_color_of},
    raster::{RasterCell, RecordingRasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

// ============================================================================
// hint()
// ============================================================================

#[test]
fn hint_is_half_block() {
    assert_eq!(HalfBlockRasterizer::new().hint(), ViewHint::HalfBlock);
}

// ============================================================================
// visible_color_of — 5 MC/DC branches
// ============================================================================

#[test]
fn visible_color_returns_default_when_glyph_space_and_bg_none() {
    let cell = Cell::new(' ', CellStyle::plain());
    assert_eq!(visible_color_of(&cell), CellColor::Default);
}

#[test]
fn visible_color_returns_bg_when_glyph_space_and_bg_set() {
    let cell = Cell::new(' ', CellStyle::plain().with_bg(CellColor::Ansi256(5)));
    assert_eq!(visible_color_of(&cell), CellColor::Ansi256(5));
}

#[test]
fn visible_color_returns_fg_when_non_space_and_fg_set() {
    let cell = Cell::new('A', CellStyle::plain().with_fg(CellColor::Rgb(10, 20, 30)));
    assert_eq!(visible_color_of(&cell), CellColor::Rgb(10, 20, 30));
}

#[test]
fn visible_color_falls_back_to_bg_when_non_space_and_fg_none() {
    let cell = Cell::new('A', CellStyle::plain().with_bg(CellColor::Named(3)));
    assert_eq!(visible_color_of(&cell), CellColor::Named(3));
}

#[test]
fn visible_color_returns_default_for_null_char_and_no_colors() {
    let cell = Cell::new('\0', CellStyle::plain());
    assert_eq!(visible_color_of(&cell), CellColor::Default);
}

// ============================================================================
// Encoding-policy rows via single-pair rasterize
// ============================================================================

/// Build a 1×2 grid where the top cell is `top`, the bottom is `bot`,
/// run the rasterizer, return the single emitted cell.
fn rasterize_pair(top: Cell, bot: Cell) -> RasterCell {
    let mut grid = CellCapability::new(1, 2);
    grid.write_cell(0, 0, top).unwrap();
    grid.write_cell(0, 1, bot).unwrap();
    let mut out = RecordingRasterOutput::new(1, 1);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 1);
    out.cells()[0]
}

#[test]
fn encode_both_default_emits_space_plain() {
    let cell = rasterize_pair(Cell::default(), Cell::default());
    assert_eq!(cell.ch, ' ');
    assert_eq!(cell.style, CellStyle::plain());
}

#[test]
fn encode_same_non_default_color_emits_space_with_shared_bg() {
    let col = CellColor::Ansi256(9);
    let top = Cell::new(' ', CellStyle::plain().with_bg(col));
    let bot = Cell::new(' ', CellStyle::plain().with_bg(col));
    let cell = rasterize_pair(top, bot);
    assert_eq!(cell.ch, ' ');
    assert_eq!(cell.style, CellStyle::plain().with_bg(col));
}

#[test]
fn encode_top_only_emits_upper_half_with_fg() {
    let col = CellColor::Rgb(200, 50, 50);
    let top = Cell::new('X', CellStyle::plain().with_fg(col));
    let bot = Cell::default();
    let cell = rasterize_pair(top, bot);
    assert_eq!(cell.ch, HALF_UPPER);
    assert_eq!(cell.style, CellStyle::plain().with_fg(col));
}

#[test]
fn encode_bottom_only_emits_lower_half_with_fg() {
    let col = CellColor::Rgb(50, 200, 50);
    let top = Cell::default();
    let bot = Cell::new('Y', CellStyle::plain().with_fg(col));
    let cell = rasterize_pair(top, bot);
    assert_eq!(cell.ch, HALF_LOWER);
    assert_eq!(cell.style, CellStyle::plain().with_fg(col));
}

#[test]
fn encode_mixed_colors_emits_upper_half_with_fg_and_bg() {
    let top_col = CellColor::Ansi256(1);
    let bot_col = CellColor::Ansi256(4);
    let top = Cell::new('A', CellStyle::plain().with_fg(top_col));
    let bot = Cell::new('B', CellStyle::plain().with_fg(bot_col));
    let cell = rasterize_pair(top, bot);
    assert_eq!(cell.ch, HALF_UPPER);
    assert_eq!(cell.style, CellStyle::plain().with_fg(top_col).with_bg(bot_col));
}

#[test]
fn encode_discards_top_attributes() {
    // BOLD on the top cell must not leak into the half-block output:
    // half-block cells are graphical, not textual.
    let top = Cell::new(
        'A',
        CellStyle::plain()
            .with_fg(CellColor::Ansi256(2))
            .with_attrs(CellAttrs::BOLD),
    );
    let bot = Cell::default();
    let cell = rasterize_pair(top, bot);
    assert!(!cell.style.attrs.contains(CellAttrs::BOLD));
}

// ============================================================================
// Integration: full rasterize over canonical grids
// ============================================================================

#[test]
fn rasterize_emits_width_times_half_height_cells() {
    // 4×6 logical grid → 4×3 terminal grid = 12 cells.
    let grid = CellCapability::new(4, 6);
    let mut out = RecordingRasterOutput::new(4, 3);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 12);
}

#[test]
fn rasterize_clamps_to_output_width() {
    // Grid wider than output: only first 2 columns rendered.
    let grid = CellCapability::new(6, 2);
    let mut out = RecordingRasterOutput::new(2, 1);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 2);
    assert!(out.cells().iter().all(|c| c.x < 2));
}

#[test]
fn rasterize_clamps_to_output_height() {
    // Grid taller than output: first row only.
    let grid = CellCapability::new(2, 10);
    let mut out = RecordingRasterOutput::new(2, 1);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 2);
    assert!(out.cells().iter().all(|c| c.y == 0));
}

#[test]
fn rasterize_odd_height_grid_treats_unpaired_bottom_as_default() {
    // 1×3 grid — the third row is unpaired; should produce a 2-row
    // terminal output with the final row treating the missing bottom
    // as default-blank.
    let mut grid = CellCapability::new(1, 3);
    let col = CellColor::Ansi256(6);
    grid.write_cell(0, 0, Cell::new(' ', CellStyle::plain().with_bg(col)))
        .unwrap();
    grid.write_cell(0, 1, Cell::new(' ', CellStyle::plain().with_bg(col)))
        .unwrap();
    grid.write_cell(0, 2, Cell::new(' ', CellStyle::plain().with_bg(col)))
        .unwrap();
    let mut out = RecordingRasterOutput::new(1, 2);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 2);
    // Row 0: both halves colored → space + bg=col
    assert_eq!(out.cells()[0].ch, ' ');
    assert_eq!(out.cells()[0].style, CellStyle::plain().with_bg(col));
    // Row 1: top colored, bottom synthetic Default → ▀ + fg=col
    assert_eq!(out.cells()[1].ch, HALF_UPPER);
    assert_eq!(out.cells()[1].style, CellStyle::plain().with_fg(col));
}

#[test]
fn rasterize_empty_grid_emits_nothing() {
    let grid = CellCapability::new(0, 0);
    let mut out = RecordingRasterOutput::new(10, 10);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert!(out.cells().is_empty());
}

#[test]
fn rasterize_zero_size_output_emits_nothing() {
    let grid = CellCapability::new(4, 4);
    let mut out = RecordingRasterOutput::new(0, 0);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
    assert!(out.cells().is_empty());
}

#[test]
fn default_constructor_equivalent_to_new() {
    // Stateless: Default::default() and new() behave identically.
    let grid = CellCapability::new(1, 2);
    let mut a = RecordingRasterOutput::new(1, 1);
    let mut b = RecordingRasterOutput::new(1, 1);
    HalfBlockRasterizer::new().rasterize(&grid, &mut a);
    HalfBlockRasterizer.rasterize(&grid, &mut b);
    assert_eq!(a.cells(), b.cells());
}
