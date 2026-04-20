//! Tests for `PixelCodec`.

use {
    super::{PixelCodec, PixelSurface},
    reovim_surface_codec::{Codec, KIND_PIXEL_BUFFER, SurfacePayloadError},
};

#[test]
fn codec_kind_is_pixel_buffer() {
    let codec = PixelCodec::new();
    assert_eq!(codec.kind(), KIND_PIXEL_BUFFER);
}

#[test]
fn encode_produces_12_be_bytes() {
    let codec = PixelCodec::new();
    let surface = PixelSurface::new(1920, 1080, 96);
    let body = codec.encode(&surface).unwrap();
    assert_eq!(body.len(), 12);
    assert_eq!(&body[0..4], &1920u32.to_be_bytes());
    assert_eq!(&body[4..8], &1080u32.to_be_bytes());
    assert_eq!(&body[8..12], &96u32.to_be_bytes());
}

#[test]
fn decode_parses_12_be_bytes() {
    let codec = PixelCodec::new();
    let mut body = Vec::new();
    body.extend_from_slice(&3840u32.to_be_bytes());
    body.extend_from_slice(&2160u32.to_be_bytes());
    body.extend_from_slice(&144u32.to_be_bytes());
    let decoded = codec.decode(&body).unwrap();
    let surface = decoded.downcast_ref::<PixelSurface>().unwrap();
    assert_eq!(*surface, PixelSurface::new(3840, 2160, 144));
}

#[test]
fn encode_decode_roundtrip() {
    let codec = PixelCodec::new();
    let surface = PixelSurface::new(800, 600, 72);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    let recovered = decoded.downcast_ref::<PixelSurface>().unwrap();
    assert_eq!(*recovered, surface);
}

#[test]
fn roundtrip_zero_fields() {
    let codec = PixelCodec::new();
    let surface = PixelSurface::new(0, 0, 0);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<PixelSurface>().unwrap(), surface);
}

#[test]
fn roundtrip_max_fields() {
    let codec = PixelCodec::new();
    let surface = PixelSurface::new(u32::MAX, u32::MAX, u32::MAX);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<PixelSurface>().unwrap(), surface);
}

#[test]
fn decode_empty_body_returns_too_short() {
    let codec = PixelCodec::new();
    let err = codec.decode(&[]).unwrap_err();
    assert_eq!(err, SurfacePayloadError::TooShort { got: 0, min: 12 });
}

#[test]
fn decode_short_body_returns_too_short() {
    let codec = PixelCodec::new();
    let err = codec.decode(&[0u8; 11]).unwrap_err();
    assert_eq!(err, SurfacePayloadError::TooShort { got: 11, min: 12 });
}

#[test]
fn encode_wrong_type_returns_invalid_data() {
    let codec = PixelCodec::new();
    let not_a_surface = "hello";
    let err = codec.encode(&not_a_surface).unwrap_err();
    matches!(err, SurfacePayloadError::InvalidData { .. });
}

#[test]
fn decode_extra_bytes_ignored() {
    let codec = PixelCodec::new();
    let mut body = Vec::new();
    body.extend_from_slice(&100u32.to_be_bytes());
    body.extend_from_slice(&200u32.to_be_bytes());
    body.extend_from_slice(&300u32.to_be_bytes());
    body.extend_from_slice(&[0xDE, 0xAD]);
    let decoded = codec.decode(&body).unwrap();
    let surface = decoded.downcast_ref::<PixelSurface>().unwrap();
    assert_eq!(*surface, PixelSurface::new(100, 200, 300));
}

#[test]
fn codec_through_trait_object() {
    let codec: Box<dyn Codec> = Box::new(PixelCodec::new());
    assert_eq!(codec.kind(), KIND_PIXEL_BUFFER);
    let surface = PixelSurface::new(640, 480, 96);
    let body = codec.encode(&surface).unwrap();
    let decoded = codec.decode(&body).unwrap();
    assert_eq!(*decoded.downcast_ref::<PixelSurface>().unwrap(), surface);
}
