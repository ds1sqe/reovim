use reovim_ext_client_tui_cap_cell::{Cell, CellCapability, CellColor, CellStyle};

use crate::{
    FullBlockRasterizer,
    raster::{RasterCell, RecordingRasterOutput},
    rasterizer::{ViewHint, ViewRasterizer},
};

fn cell_at(ch: char) -> Cell {
    Cell::new(ch, CellStyle::default())
}

fn make_grid() -> CellCapability {
    let mut g = CellCapability::new(3, 2);
    let red = CellStyle::plain().with_fg(CellColor::Named(1));
    g.write_cell(0, 0, Cell::new('a', red)).unwrap();
    g.write_cell(1, 0, cell_at('b')).unwrap();
    g.write_cell(2, 0, cell_at('c')).unwrap();
    g.write_cell(0, 1, cell_at('d')).unwrap();
    g.write_cell(1, 1, cell_at('e')).unwrap();
    g.write_cell(2, 1, cell_at('f')).unwrap();
    g
}

#[test]
fn hint_is_full_block() {
    assert_eq!(FullBlockRasterizer::new().hint(), ViewHint::FullBlock);
}

#[test]
fn full_block_emits_one_cell_per_logical_cell() {
    let grid = make_grid();
    let mut out = RecordingRasterOutput::new(3, 2);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 6);
}

#[test]
fn full_block_preserves_char_and_position() {
    let grid = make_grid();
    let mut out = RecordingRasterOutput::new(3, 2);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    let chars: Vec<char> = out.cells().iter().map(|c| c.ch).collect();
    assert_eq!(chars, vec!['a', 'b', 'c', 'd', 'e', 'f']);
    let positions: Vec<(u16, u16)> = out.cells().iter().map(|c| (c.x, c.y)).collect();
    assert_eq!(
        positions,
        vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)]
    );
}

#[test]
fn full_block_preserves_style() {
    let grid = make_grid();
    let mut out = RecordingRasterOutput::new(3, 2);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells()[0].style.fg, Some(CellColor::Named(1)));
}

#[test]
fn full_block_clips_to_output_width() {
    let grid = make_grid();
    let mut out = RecordingRasterOutput::new(2, 2);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    for c in out.cells() {
        assert!(c.x < 2);
    }
    assert_eq!(out.cells().len(), 4);
}

#[test]
fn full_block_clips_to_output_height() {
    let grid = make_grid();
    let mut out = RecordingRasterOutput::new(3, 1);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    for c in out.cells() {
        assert!(c.y < 1);
    }
    assert_eq!(out.cells().len(), 3);
}

#[test]
fn full_block_on_empty_grid_emits_nothing() {
    let grid = CellCapability::new(0, 0);
    let mut out = RecordingRasterOutput::new(10, 10);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    assert!(out.cells().is_empty());
}

#[test]
fn full_block_smaller_grid_than_output_writes_only_grid_area() {
    let grid = CellCapability::new(2, 1);
    let mut out = RecordingRasterOutput::new(10, 5);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 2);
    let positions: Vec<(u16, u16)> = out.cells().iter().map(|c| (c.x, c.y)).collect();
    assert_eq!(positions, vec![(0, 0), (1, 0)]);
}

#[test]
fn full_block_new_equals_default() {
    let a = FullBlockRasterizer::new();
    let b = FullBlockRasterizer;
    assert_eq!(a.hint(), b.hint());
}

#[test]
fn full_block_default_grid_cell_is_space_with_default_style() {
    let grid = CellCapability::new(1, 1);
    let mut out = RecordingRasterOutput::new(1, 1);
    FullBlockRasterizer::new().rasterize(&grid, &mut out);
    let only = out.cells()[0];
    assert_eq!(
        only,
        RasterCell {
            x: 0,
            y: 0,
            ch: ' ',
            style: CellStyle::default(),
        }
    );
}
