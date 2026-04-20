//! Tests for `CellGridRenderCodec`.

use {
    super::{CellGridRender, CellGridRenderCodec},
    reovim_render_codec::{Codec, KIND_CELL_GRID, RenderPayloadError},
};

fn body_with_cells(
    width: u32,
    height: u32,
    cursor_col: u32,
    cursor_row: u32,
    cells: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&width.to_be_bytes());
    buf.extend_from_slice(&height.to_be_bytes());
    buf.extend_from_slice(&cursor_col.to_be_bytes());
    buf.extend_from_slice(&cursor_row.to_be_bytes());
    buf.extend_from_slice(&u32::try_from(cells.len()).unwrap().to_be_bytes());
    buf.extend_from_slice(cells);
    buf
}

#[test]
fn codec_kind_is_cell_grid() {
    let codec = CellGridRenderCodec::new();
    assert_eq!(codec.kind(), KIND_CELL_GRID);
}

#[test]
fn encode_produces_20byte_header_plus_cells() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 24, 5, 2, vec![0xAA, 0xBB, 0xCC]);
    let body = codec.encode(&target).unwrap();
    assert_eq!(body.len(), 20 + 3);
    assert_eq!(&body[..4], &80u32.to_be_bytes());
    assert_eq!(&body[4..8], &24u32.to_be_bytes());
    assert_eq!(&body[8..12], &5u32.to_be_bytes());
    assert_eq!(&body[12..16], &2u32.to_be_bytes());
    assert_eq!(&body[16..20], &3u32.to_be_bytes());
    assert_eq!(&body[20..], &[0xAA, 0xBB, 0xCC]);
}

#[test]
fn roundtrip_empty_cells() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 24, 0, 0, Vec::new());
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridRender>().unwrap(), target);
}

#[test]
fn roundtrip_nonempty_cells() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(200, 50, 100, 25, vec![1, 2, 3, 4, 5]);
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridRender>().unwrap(), target);
}

#[test]
fn roundtrip_zero_dimensions_zero_cursor() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(0, 0, 0, 0, Vec::new());
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridRender>().unwrap(), target);
}

#[test]
fn roundtrip_max_dimensions() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(u32::MAX, u32::MAX, u32::MAX - 1, u32::MAX - 1, Vec::new());
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridRender>().unwrap(), target);
}

#[test]
fn decode_empty_body_returns_too_short() {
    let codec = CellGridRenderCodec::new();
    let err = codec.decode(&[]).unwrap_err();
    assert_eq!(err, RenderPayloadError::TooShort { got: 0, min: 20 });
}

#[test]
fn decode_short_header_returns_too_short() {
    let codec = CellGridRenderCodec::new();
    let err = codec.decode(&[0u8; 19]).unwrap_err();
    assert_eq!(err, RenderPayloadError::TooShort { got: 19, min: 20 });
}

#[test]
fn decode_header_says_more_cells_than_body_has_returns_too_short() {
    // header declares cells_len = 10 but body only has 20 bytes total.
    let codec = CellGridRenderCodec::new();
    let mut body = Vec::new();
    body.extend_from_slice(&80u32.to_be_bytes());
    body.extend_from_slice(&24u32.to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&10u32.to_be_bytes());
    // no cells bytes appended — total length 20, but required 30.
    let err = codec.decode(&body).unwrap_err();
    assert_eq!(err, RenderPayloadError::TooShort { got: 20, min: 30 });
}

#[test]
fn encode_cursor_overrun_width_returns_invalid_data() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 24, 80, 0, Vec::new());
    let err = codec.encode(&target).unwrap_err();
    assert!(matches!(err, RenderPayloadError::InvalidData { .. }));
}

#[test]
fn encode_cursor_overrun_height_returns_invalid_data() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 24, 0, 24, Vec::new());
    let err = codec.encode(&target).unwrap_err();
    assert!(matches!(err, RenderPayloadError::InvalidData { .. }));
}

#[test]
fn encode_zero_width_nonzero_cursor_col_returns_invalid_data() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(0, 24, 1, 0, Vec::new());
    let err = codec.encode(&target).unwrap_err();
    assert!(matches!(err, RenderPayloadError::InvalidData { .. }));
}

#[test]
fn encode_zero_height_nonzero_cursor_row_returns_invalid_data() {
    let codec = CellGridRenderCodec::new();
    let target = CellGridRender::new(80, 0, 0, 1, Vec::new());
    let err = codec.encode(&target).unwrap_err();
    assert!(matches!(err, RenderPayloadError::InvalidData { .. }));
}

#[test]
fn decode_cursor_overrun_returns_invalid_data() {
    let codec = CellGridRenderCodec::new();
    let body = body_with_cells(80, 24, 99, 0, &[]);
    let err = codec.decode(&body).unwrap_err();
    assert!(matches!(err, RenderPayloadError::InvalidData { .. }));
}

#[test]
fn encode_wrong_type_returns_invalid_data() {
    let codec = CellGridRenderCodec::new();
    let not_a_target = 42u32;
    let err = codec.encode(&not_a_target).unwrap_err();
    assert!(matches!(err, RenderPayloadError::InvalidData { .. }));
}

#[test]
fn decode_extra_bytes_ignored() {
    let codec = CellGridRenderCodec::new();
    let mut body = body_with_cells(80, 24, 5, 5, &[0x11, 0x22]);
    body.extend_from_slice(&[0xDE, 0xAD]);
    let decoded = codec.decode(&body).unwrap();
    let target = decoded.downcast_ref::<CellGridRender>().unwrap();
    assert_eq!(*target, CellGridRender::new(80, 24, 5, 5, vec![0x11, 0x22]));
}

#[test]
fn codec_through_trait_object() {
    let codec: Box<dyn Codec> = Box::new(CellGridRenderCodec::new());
    assert_eq!(codec.kind(), KIND_CELL_GRID);
    let target = CellGridRender::new(10, 20, 3, 4, vec![9, 8, 7]);
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridRender>().unwrap(), target);
}

#[test]
fn roundtrip_unicode_opaque_cells() {
    let codec = CellGridRenderCodec::new();
    // cells are opaque bytes — they happen to be utf-8 here, but the codec
    // does not care.
    let target = CellGridRender::new(5, 1, 0, 0, "héllo".as_bytes().to_vec());
    let body = codec.encode(&target).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridRender>().unwrap(), target);
}
