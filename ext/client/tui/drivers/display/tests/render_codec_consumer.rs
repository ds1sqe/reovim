//! Phase R.4 — narrow render-codec consumer proof.
//!
//! Wires `reovim_render_codec_tui::CellGridRenderCodec` through a
//! `RenderCodecRegistry` into a `RenderTarget` adapter. Exercises the
//! full encode → submit → registry-lookup → decode path end-to-end.
//!
//! This satisfies mission known-gap #5 ("render-codec has zero
//! consumers") without touching the interactive render hot path. The
//! production TUI render engine migration is tracked as follow-on R-b.

use {
    reovim_client_driver::render::{RenderError, RenderTarget},
    reovim_render_codec::{KIND_CELL_GRID, RenderDescriptor},
    reovim_render_codec_tui::{CellGridRender, CellGridRenderCodec},
    reovim_subsys_render_codec::{
        DefaultRenderCodecRegistry, KIND_CELL_GRID as SUBSYS_KIND_CELL_GRID,
        RenderCodecRegistry,
    },
    std::sync::Arc,
};

/// A minimal `RenderTarget` that looks up the cell-grid codec in the
/// registry, decodes the body, and records the decoded render value for
/// test inspection.
struct RecordingRenderTarget {
    registry: Arc<dyn RenderCodecRegistry>,
    recorded: Vec<CellGridRender>,
}

impl RecordingRenderTarget {
    fn new(registry: Arc<dyn RenderCodecRegistry>) -> Self {
        Self {
            registry,
            recorded: Vec::new(),
        }
    }
}

impl RenderTarget for RecordingRenderTarget {
    fn submit(&mut self, data: &[u8]) -> Result<(), RenderError> {
        let codec = self
            .registry
            .get(KIND_CELL_GRID)
            .ok_or_else(|| RenderError::InvalidData("no codec for KIND_CELL_GRID".into()))?;
        let decoded = codec
            .decode(data)
            .map_err(|e| RenderError::InvalidData(e.to_string()))?;
        let target = decoded
            .downcast_ref::<CellGridRender>()
            .ok_or_else(|| RenderError::InvalidData("decoded value is not CellGridRender".into()))?;
        self.recorded.push(target.clone());
        Ok(())
    }
}

fn registry_with_cellgrid_codec() -> Arc<DefaultRenderCodecRegistry> {
    let reg = Arc::new(DefaultRenderCodecRegistry::new());
    reg.register(Arc::new(CellGridRenderCodec::new()));
    reg
}

#[test]
fn roundtrip_nonempty_cells_through_submit() {
    let reg = registry_with_cellgrid_codec();
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 24, 10, 5, vec![0xAA, 0xBB]);
    let body = reovim_render_codec::Codec::encode(&codec, &target).unwrap();

    let mut adapter = RecordingRenderTarget::new(reg);
    adapter.submit(&body).unwrap();

    assert_eq!(adapter.recorded.len(), 1);
    assert_eq!(adapter.recorded[0], target);
}

#[test]
fn roundtrip_empty_cells_through_submit() {
    let reg = registry_with_cellgrid_codec();
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 24, 0, 0, Vec::new());
    let body = reovim_render_codec::Codec::encode(&codec, &target).unwrap();

    let mut adapter = RecordingRenderTarget::new(reg);
    adapter.submit(&body).unwrap();

    assert_eq!(adapter.recorded[0], target);
}

#[test]
fn registry_miss_returns_invalid_data() {
    let reg: Arc<DefaultRenderCodecRegistry> = Arc::new(DefaultRenderCodecRegistry::new());
    let mut adapter = RecordingRenderTarget::new(reg);
    let result = adapter.submit(&[0u8; 20]);
    assert!(matches!(result, Err(RenderError::InvalidData(_))));
    assert!(adapter.recorded.is_empty());
}

#[test]
fn short_body_returns_invalid_data() {
    let reg = registry_with_cellgrid_codec();
    let mut adapter = RecordingRenderTarget::new(reg);
    let result = adapter.submit(&[0u8; 5]);
    assert!(matches!(result, Err(RenderError::InvalidData(_))));
}

#[test]
fn cursor_overrun_returns_invalid_data() {
    let reg = registry_with_cellgrid_codec();
    let mut body = Vec::new();
    body.extend_from_slice(&80u32.to_be_bytes());
    body.extend_from_slice(&24u32.to_be_bytes());
    body.extend_from_slice(&100u32.to_be_bytes()); // cursor_col > width
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    let mut adapter = RecordingRenderTarget::new(reg);
    let result = adapter.submit(&body);
    assert!(matches!(result, Err(RenderError::InvalidData(_))));
}

#[test]
fn submit_preserves_cell_bytes_byte_for_byte() {
    let reg = registry_with_cellgrid_codec();
    let codec = CellGridRenderCodec::new();
    let cells: Vec<u8> = (0u8..200).collect();
    let target = CellGridRender::new(20, 10, 0, 0, cells.clone());
    let body = reovim_render_codec::Codec::encode(&codec, &target).unwrap();

    let mut adapter = RecordingRenderTarget::new(reg);
    adapter.submit(&body).unwrap();
    assert_eq!(adapter.recorded[0].cells, cells);
}

#[test]
fn descriptor_body_is_what_gets_submitted() {
    // Ties the uapi envelope to the adapter contract — the RenderDescriptor::body
    // bytes are exactly what the adapter accepts via submit.
    let reg = registry_with_cellgrid_codec();
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(5, 5, 1, 1, vec![0x42]);
    let body = reovim_render_codec::Codec::encode(&codec, &target).unwrap();
    let descriptor = RenderDescriptor::new(KIND_CELL_GRID, body);

    let mut adapter = RecordingRenderTarget::new(reg);
    adapter.submit(descriptor.body()).unwrap();
    assert_eq!(adapter.recorded[0], target);
}

#[test]
fn subsys_kind_constant_matches_uapi() {
    // Sanity check that the subsys re-export matches the uapi constant — this
    // wiring is what makes the adapter work with either import path.
    assert_eq!(SUBSYS_KIND_CELL_GRID, KIND_CELL_GRID);
}
