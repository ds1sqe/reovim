use reovim_ext_client_tui_cap_cell::CellStyle;

use crate::raster::{RasterCell, RasterOutput, RecordingRasterOutput};

fn cell(x: u16, y: u16, ch: char) -> RasterCell {
    RasterCell {
        x,
        y,
        ch,
        style: CellStyle::default(),
    }
}

#[test]
fn recorder_reports_declared_size() {
    let r = RecordingRasterOutput::new(40, 10);
    assert_eq!(r.size(), (40, 10));
}

#[test]
fn recorder_accumulates_in_bounds_writes_in_order() {
    let mut r = RecordingRasterOutput::new(4, 2);
    r.set_cell(cell(0, 0, 'a'));
    r.set_cell(cell(1, 0, 'b'));
    r.set_cell(cell(0, 1, 'c'));
    let cells = r.cells();
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[0].ch, 'a');
    assert_eq!(cells[1].ch, 'b');
    assert_eq!(cells[2].ch, 'c');
}

#[test]
fn recorder_drops_writes_with_x_out_of_bounds() {
    let mut r = RecordingRasterOutput::new(2, 2);
    r.set_cell(cell(2, 0, 'x'));
    assert!(r.cells().is_empty());
}

#[test]
fn recorder_drops_writes_with_y_out_of_bounds() {
    let mut r = RecordingRasterOutput::new(2, 2);
    r.set_cell(cell(0, 2, 'y'));
    assert!(r.cells().is_empty());
}

#[test]
fn recorder_accepts_writes_at_boundary() {
    let mut r = RecordingRasterOutput::new(2, 2);
    r.set_cell(cell(1, 1, 'z'));
    assert_eq!(r.cells().len(), 1);
}
