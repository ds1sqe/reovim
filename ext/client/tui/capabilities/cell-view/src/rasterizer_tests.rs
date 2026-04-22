use reovim_ext_client_tui_cap_cell::CellCapability;

use crate::{
    FullBlockRasterizer,
    raster::RecordingRasterOutput,
    rasterizer::{ViewHint, ViewRasterizer},
};

#[test]
fn trait_is_object_safe() {
    let r: Box<dyn ViewRasterizer> = Box::new(FullBlockRasterizer::new());
    assert_eq!(r.hint(), ViewHint::FullBlock);
}

#[test]
fn object_safe_rasterize_routes_to_impl() {
    let r: Box<dyn ViewRasterizer> = Box::new(FullBlockRasterizer::new());
    let grid = CellCapability::new(1, 1);
    let mut out = RecordingRasterOutput::new(1, 1);
    r.rasterize(&grid, &mut out);
    assert_eq!(out.cells().len(), 1);
}
