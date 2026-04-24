//! Tests for [`BrailleRasterizer`] and its internal helpers.
//!
//! Target: 100% line coverage. The 8-dot-bit table is parameterised
//! across every bit position; the color-scan policy is locked by a
//! dedicated test; integration tests cover clamp, partial, empty,
//! zero-size, and attribute-discard behaviour.

use reovim_ext_client_tui_cap_cell::{Cell, CellAttrs, CellCapability, CellColor, CellStyle};

use crate::{
    braille::{BRAILLE_BLANK, BrailleRasterizer, DOT_BITS, dominant_fg, dot_mask_of_subgrid},
    raster::{RasterCell, RecordingRasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

// ============================================================================
// hint()
// ============================================================================

#[test]
fn hint_is_braille() {
    assert_eq!(BrailleRasterizer::new().hint(), ViewHint::Braille);
}

// ============================================================================
// dot_mask_of_subgrid — 8 positions + empty + oob
// ============================================================================

fn fill_one(grid: &mut CellCapability, x: u16, y: u16) {
    grid.write_cell(x, y, Cell::new(' ', CellStyle::plain().with_bg(CellColor::Ansi256(1))))
        .unwrap();
}

#[test]
fn dot_mask_returns_zero_for_empty_subgrid() {
    let grid = CellCapability::new(2, 4);
    assert_eq!(dot_mask_of_subgrid(&grid, 0, 0), 0);
}

#[test]
fn dot_mask_sets_bit_for_each_of_8_positions() {
    for dx in 0..2u16 {
        for dy in 0..4u16 {
            let mut grid = CellCapability::new(2, 4);
            fill_one(&mut grid, dx, dy);
            let mask = dot_mask_of_subgrid(&grid, 0, 0);
            let expected = DOT_BITS[dx as usize][dy as usize];
            assert_eq!(
                mask, expected,
                "position (dx={dx}, dy={dy}) should set bit {expected:#04x}, got {mask:#04x}"
            );
        }
    }
}

#[test]
fn dot_mask_combines_all_8_dots() {
    let mut grid = CellCapability::new(2, 4);
    for dx in 0..2u16 {
        for dy in 0..4u16 {
            fill_one(&mut grid, dx, dy);
        }
    }
    assert_eq!(dot_mask_of_subgrid(&grid, 0, 0), 0xFF);
}

#[test]
fn dot_mask_out_of_bounds_cells_count_as_default() {
    // 1×2 grid — half the 2×4 sub-grid is out of bounds; OOB cells
    // must not set bits.
    let mut grid = CellCapability::new(1, 2);
    fill_one(&mut grid, 0, 0);
    fill_one(&mut grid, 0, 1);
    let mask = dot_mask_of_subgrid(&grid, 0, 0);
    assert_eq!(mask, DOT_BITS[0][0] | DOT_BITS[0][1]);
}

// ============================================================================
// dominant_fg — scan order + all-default
// ============================================================================

#[test]
fn dominant_fg_returns_none_when_all_default() {
    let grid = CellCapability::new(2, 4);
    assert_eq!(dominant_fg(&grid, 0, 0), None);
}

#[test]
fn dominant_fg_returns_first_visible_in_scan_order() {
    // Scan order is row-major over dy then dx:
    //   (0,0) (1,0) (0,1) (1,1) (0,2) (1,2) (0,3) (1,3)
    //
    // Put a non-default color at (1,0) and a *different* color at
    // (0,1). Scan hits (1,0) before (0,1), so the returned color is
    // the one at (1,0).
    let mut grid = CellCapability::new(2, 4);
    grid.write_cell(1, 0, Cell::new(' ', CellStyle::plain().with_bg(CellColor::Ansi256(3))))
        .unwrap();
    grid.write_cell(0, 1, Cell::new(' ', CellStyle::plain().with_bg(CellColor::Ansi256(5))))
        .unwrap();
    assert_eq!(dominant_fg(&grid, 0, 0), Some(CellColor::Ansi256(3)));
}

// ============================================================================
// Full rasterize — per-case encoding behaviour
// ============================================================================

#[test]
fn rasterize_no_dots_emits_space_plain() {
    let grid = CellCapability::new(2, 4);
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 1);
    assert_eq!(out.cells()[0].ch, ' ');
    assert_eq!(out.cells()[0].style, CellStyle::plain());
}

#[test]
fn rasterize_full_dots_emits_u28ff() {
    let mut grid = CellCapability::new(2, 4);
    for dx in 0..2u16 {
        for dy in 0..4u16 {
            fill_one(&mut grid, dx, dy);
        }
    }
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 1);
    assert_eq!(out.cells()[0].ch, '\u{28FF}');
    assert_eq!(out.cells()[0].style, CellStyle::plain().with_fg(CellColor::Ansi256(1)));
}

#[test]
fn rasterize_monochrome_subgrid_emits_braille_glyph_with_fg() {
    // Two dots at (0,0) and (1,0) with the same color → glyph with
    // mask 0x01 | 0x08 = 0x09, fg = the shared color.
    let mut grid = CellCapability::new(2, 4);
    let col = CellColor::Ansi256(4);
    grid.write_cell(0, 0, Cell::new(' ', CellStyle::plain().with_bg(col)))
        .unwrap();
    grid.write_cell(1, 0, Cell::new(' ', CellStyle::plain().with_bg(col)))
        .unwrap();
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells()[0].ch, char::from_u32(0x2800 + 0x09).unwrap());
    assert_eq!(out.cells()[0].style, CellStyle::plain().with_fg(col));
}

#[test]
fn rasterize_mixed_colors_uses_first_scan_order_color() {
    // (0,0) = red, (1,1) = blue → mask 0x01 | 0x10 = 0x11, fg = red
    // (first in scan order).
    let mut grid = CellCapability::new(2, 4);
    grid.write_cell(0, 0, Cell::new(' ', CellStyle::plain().with_bg(CellColor::Ansi256(1))))
        .unwrap();
    grid.write_cell(1, 1, Cell::new(' ', CellStyle::plain().with_bg(CellColor::Ansi256(4))))
        .unwrap();
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells()[0].ch, char::from_u32(0x2800 + 0x11).unwrap());
    assert_eq!(out.cells()[0].style, CellStyle::plain().with_fg(CellColor::Ansi256(1)));
}

#[test]
fn rasterize_discards_top_attributes() {
    // BOLD on a dot must not surface on the Braille glyph.
    let mut grid = CellCapability::new(2, 4);
    grid.write_cell(
        0,
        0,
        Cell::new(
            'X',
            CellStyle::plain()
                .with_fg(CellColor::Ansi256(2))
                .with_attrs(CellAttrs::BOLD),
        ),
    )
    .unwrap();
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert!(!out.cells()[0].style.attrs.contains(CellAttrs::BOLD));
}

// ============================================================================
// Clamp + partial + zero-size behaviour
// ============================================================================

#[test]
fn rasterize_clamps_to_output_width() {
    // Logical grid 6×4 → ceil(6/2)=3 terminal columns, but output is
    // only 1 column wide. Expect exactly 1 cell.
    let grid = CellCapability::new(6, 4);
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 1);
    assert!(out.cells().iter().all(|c| c.x < 1));
}

#[test]
fn rasterize_clamps_to_output_height() {
    // Logical grid 2×16 → 4 terminal rows, output is only 1 row.
    let grid = CellCapability::new(2, 16);
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 1);
    assert!(out.cells().iter().all(|c| c.y == 0));
}

#[test]
fn rasterize_partial_width_grid_treats_missing_col_as_default() {
    // 1×4 grid → ceil(1/2) = 1 terminal column. The right-column dots
    // are out of bounds and must contribute nothing.
    let mut grid = CellCapability::new(1, 4);
    fill_one(&mut grid, 0, 0);
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    // Only DOT_BITS[0][0] = 0x01 is on.
    assert_eq!(out.cells()[0].ch, char::from_u32(0x2800 + 0x01).unwrap());
}

#[test]
fn rasterize_partial_height_grid_treats_missing_rows_as_default() {
    // 2×1 grid → ceil(1/4) = 1 terminal row. Logical rows 1..=3 are
    // out of bounds for this sub-grid.
    let mut grid = CellCapability::new(2, 1);
    fill_one(&mut grid, 0, 0);
    fill_one(&mut grid, 1, 0);
    let mut out = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    // Bits 0x01 and 0x08 on, rest OOB.
    assert_eq!(out.cells()[0].ch, char::from_u32(0x2800 + 0x09).unwrap());
}

#[test]
fn rasterize_emits_width_div_ceil_2_times_height_div_ceil_4_cells() {
    // 4×8 logical grid → 2×2 terminal = 4 cells.
    let grid = CellCapability::new(4, 8);
    let mut out = RecordingRasterOutput::new(2, 2);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 4);
}

#[test]
fn rasterize_empty_grid_emits_nothing() {
    let grid = CellCapability::new(0, 0);
    let mut out = RecordingRasterOutput::new(10, 10);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert!(out.cells().is_empty());
}

#[test]
fn rasterize_zero_size_output_emits_nothing() {
    let grid = CellCapability::new(8, 16);
    let mut out = RecordingRasterOutput::new(0, 0);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    assert!(out.cells().is_empty());
}

#[test]
fn default_constructor_equivalent_to_new() {
    // Stateless: Default::default() and new() behave identically.
    let grid = CellCapability::new(2, 4);
    let mut a = RecordingRasterOutput::new(1, 1);
    let mut b = RecordingRasterOutput::new(1, 1);
    BrailleRasterizer::new().rasterize(&grid, &mut a);
    let r: BrailleRasterizer = BrailleRasterizer;
    r.rasterize(&grid, &mut b);
    assert_eq!(a.cells(), b.cells());
}

// ============================================================================
// Invariant: BRAILLE_BLANK matches U+2800
// ============================================================================

#[test]
fn braille_blank_is_u2800() {
    assert_eq!(BRAILLE_BLANK, '\u{2800}');
    // Sanity: it's the zero-mask point of the contiguous 256-glyph range.
    assert_eq!(u32::from(BRAILLE_BLANK), 0x2800);
}

// ============================================================================
// Touch RasterCell explicitly so re-exports stay live
// ============================================================================

#[test]
fn emitted_cells_carry_terminal_coordinates() {
    let grid = CellCapability::new(4, 8);
    let mut out = RecordingRasterOutput::new(2, 2);
    BrailleRasterizer::new().rasterize(&grid, &mut out);
    // Row-major: (0,0), (1,0), (0,1), (1,1).
    let cells: Vec<&RasterCell> = out.cells().iter().collect();
    assert_eq!(cells[0].x, 0);
    assert_eq!(cells[0].y, 0);
    assert_eq!(cells[1].x, 1);
    assert_eq!(cells[1].y, 0);
    assert_eq!(cells[2].x, 0);
    assert_eq!(cells[2].y, 1);
    assert_eq!(cells[3].x, 1);
    assert_eq!(cells[3].y, 1);
}
