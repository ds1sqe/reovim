//! `CellGridSurface` concrete type and `Codec` implementation for
//! `KIND_CELL_GRID` (0x0001).
//!
//! Body layout: 8 bytes, big-endian.
//!
//! ```text
//! ┌─────────────────┬─────────────────┐
//! │ width   (u32 BE)│ height (u32 BE) │
//! │ 4 bytes         │ 4 bytes         │
//! └─────────────────┴─────────────────┘
//! ```

use {
    reovim_surface_codec::{Codec, KIND_CELL_GRID, SurfacePayloadError},
    std::any::Any,
};

/// A cell-grid rendering surface (TUI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellGridSurface {
    /// Width in cells.
    pub width: u32,
    /// Height in cells.
    pub height: u32,
}

impl CellGridSurface {
    /// Size of the encoded body in bytes.
    pub const BODY_LEN: usize = 8;

    /// Construct a cell-grid surface.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Codec for [`CellGridSurface`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CellGridCodec;

impl CellGridCodec {
    /// Construct a new codec instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Codec for CellGridCodec {
    fn kind(&self) -> u16 {
        KIND_CELL_GRID
    }

    fn encode(&self, value: &dyn Any) -> Result<Vec<u8>, SurfacePayloadError> {
        let surface =
            value
                .downcast_ref::<CellGridSurface>()
                .ok_or(SurfacePayloadError::InvalidData {
                    reason: "expected CellGridSurface",
                })?;
        let mut buf = Vec::with_capacity(CellGridSurface::BODY_LEN);
        buf.extend_from_slice(&surface.width.to_be_bytes());
        buf.extend_from_slice(&surface.height.to_be_bytes());
        Ok(buf)
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, SurfacePayloadError> {
        if body.len() < CellGridSurface::BODY_LEN {
            return Err(SurfacePayloadError::TooShort {
                got: body.len(),
                min: CellGridSurface::BODY_LEN,
            });
        }
        let width = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
        let height = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        Ok(Box::new(CellGridSurface { width, height }))
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
