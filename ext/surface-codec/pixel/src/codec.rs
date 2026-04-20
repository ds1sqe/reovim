//! `PixelSurface` concrete type and `Codec` implementation for
//! `KIND_PIXEL_BUFFER` (0x0002).
//!
//! Body layout: 12 bytes, big-endian.
//!
//! ```text
//! ┌──────────────────┬──────────────────┬──────────────┐
//! │ width_px (u32 BE)│ height_px(u32 BE)│ dpi  (u32 BE)│
//! │ 4 bytes          │ 4 bytes          │ 4 bytes      │
//! └──────────────────┴──────────────────┴──────────────┘
//! ```

use {
    reovim_surface_codec::{Codec, KIND_PIXEL_BUFFER, SurfacePayloadError},
    std::any::Any,
};

/// A pixel-raster rendering surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSurface {
    /// Width in pixels.
    pub width_px: u32,
    /// Height in pixels.
    pub height_px: u32,
    /// Dots per inch.
    pub dpi: u32,
}

impl PixelSurface {
    /// Size of the encoded body in bytes.
    pub const BODY_LEN: usize = 12;

    /// Construct a pixel surface.
    #[must_use]
    pub const fn new(width_px: u32, height_px: u32, dpi: u32) -> Self {
        Self {
            width_px,
            height_px,
            dpi,
        }
    }
}

/// Codec for [`PixelSurface`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PixelCodec;

impl PixelCodec {
    /// Construct a new codec instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Codec for PixelCodec {
    fn kind(&self) -> u16 {
        KIND_PIXEL_BUFFER
    }

    fn encode(&self, value: &dyn Any) -> Result<Vec<u8>, SurfacePayloadError> {
        let surface =
            value
                .downcast_ref::<PixelSurface>()
                .ok_or(SurfacePayloadError::InvalidData {
                    reason: "expected PixelSurface",
                })?;
        let mut buf = Vec::with_capacity(PixelSurface::BODY_LEN);
        buf.extend_from_slice(&surface.width_px.to_be_bytes());
        buf.extend_from_slice(&surface.height_px.to_be_bytes());
        buf.extend_from_slice(&surface.dpi.to_be_bytes());
        Ok(buf)
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, SurfacePayloadError> {
        if body.len() < PixelSurface::BODY_LEN {
            return Err(SurfacePayloadError::TooShort {
                got: body.len(),
                min: PixelSurface::BODY_LEN,
            });
        }
        let width_px = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
        let height_px = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        let dpi = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
        Ok(Box::new(PixelSurface {
            width_px,
            height_px,
            dpi,
        }))
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
