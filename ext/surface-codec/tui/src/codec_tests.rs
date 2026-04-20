//! Tests for `CellGridCodec`.

use {
    super::{CellGridCodec, CellGridSurface},
    reovim_surface_codec::{Codec, KIND_CELL_GRID, SurfacePayloadError},
};

#[test]
fn codec_kind_is_cell_grid() {
    let codec = CellGridCodec::new();
    assert_eq!(codec.kind(), KIND_CELL_GRID);
}

#[test]
fn encode_produces_8_be_bytes() {
    let codec = CellGridCodec::new();
    let surface = CellGridSurface::new(80, 24);
    let body = codec.encode(&surface).unwrap();
    assert_eq!(body.len(), 8);
    assert_eq!(&body[..4], &80u32.to_be_bytes());
    assert_eq!(&body[4..], &24u32.to_be_bytes());
}

#[test]
fn decode_parses_8_be_bytes() {
    let codec = CellGridCodec::new();
    let mut body = Vec::new();
    body.extend_from_slice(&120u32.to_be_bytes());
    body.extend_from_slice(&30u32.to_be_bytes());
    let decoded = codec.decode(&body).unwrap();
    let surface = decoded.downcast_ref::<CellGridSurface>().unwrap();
    assert_eq!(*surface, CellGridSurface::new(120, 30));
}

#[test]
fn encode_decode_roundtrip() {
    let codec = CellGridCodec::new();
    let surface = CellGridSurface::new(200, 50);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    let recovered = decoded.downcast_ref::<CellGridSurface>().unwrap();
    assert_eq!(*recovered, surface);
}

#[test]
fn roundtrip_zero_dimensions() {
    let codec = CellGridCodec::new();
    let surface = CellGridSurface::new(0, 0);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridSurface>().unwrap(), surface);
}

#[test]
fn roundtrip_max_dimensions() {
    let codec = CellGridCodec::new();
    let surface = CellGridSurface::new(u32::MAX, u32::MAX);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridSurface>().unwrap(), surface);
}

#[test]
fn decode_empty_body_returns_too_short() {
    let codec = CellGridCodec::new();
    let err = codec.decode(&[]).unwrap_err();
    assert_eq!(err, SurfacePayloadError::TooShort { got: 0, min: 8 });
}

#[test]
fn decode_short_body_returns_too_short() {
    let codec = CellGridCodec::new();
    let err = codec.decode(&[0, 0, 0, 0, 0, 0, 0]).unwrap_err();
    assert_eq!(err, SurfacePayloadError::TooShort { got: 7, min: 8 });
}

#[test]
fn encode_wrong_type_returns_invalid_data() {
    let codec = CellGridCodec::new();
    let not_a_surface = 42u32;
    let err = codec.encode(&not_a_surface).unwrap_err();
    matches!(err, SurfacePayloadError::InvalidData { .. });
}

#[test]
fn decode_extra_bytes_ignored() {
    let codec = CellGridCodec::new();
    let mut body = Vec::new();
    body.extend_from_slice(&80u32.to_be_bytes());
    body.extend_from_slice(&24u32.to_be_bytes());
    body.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let decoded = codec.decode(&body).unwrap();
    let surface = decoded.downcast_ref::<CellGridSurface>().unwrap();
    assert_eq!(*surface, CellGridSurface::new(80, 24));
}

#[test]
fn codec_through_trait_object() {
    let codec: Box<dyn Codec> = Box::new(CellGridCodec::new());
    assert_eq!(codec.kind(), KIND_CELL_GRID);
    let surface = CellGridSurface::new(10, 20);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<CellGridSurface>().unwrap(), surface);
}
