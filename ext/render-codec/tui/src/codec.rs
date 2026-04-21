//! `CellGridRender` concrete type and `Codec` implementation for
//! `KIND_CELL_GRID` (0x0001).
//!
//! Body layout: 20-byte fixed header (big-endian) + variable-length
//! opaque cells payload.
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────┐
//! │ width (u32 BE) │ height (u32 BE) │ 4 + 4 bytes            │
//! ├───────────────────────────────────────────────────────────┤
//! │ cursor_col (u32 BE) │ cursor_row (u32 BE) │ 4 + 4 bytes   │
//! ├───────────────────────────────────────────────────────────┤
//! │ cells_len (u32 BE) │ 4 bytes                              │
//! ├───────────────────────────────────────────────────────────┤
//! │ cells (cells_len bytes, opaque)                           │
//! └───────────────────────────────────────────────────────────┘
//! Minimum body length: 20 bytes (cells_len = 0).
//! ```

use {
    reovim_render_codec::{Codec, KIND_CELL_GRID, RenderPayloadError},
    std::any::Any,
};

/// A cell-grid render target.
///
/// Carries the minimum needed for the opaque-submit seam:
///   - grid dimensions so the adapter can validate against its surface.
///   - cursor position so the adapter can apply the overlay.
///   - an opaque cell-data blob the domain view produced. The codec only
///     frames these bytes; it does not interpret them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGridRender {
    /// Grid width in cells.
    pub width: u32,
    /// Grid height in cells.
    pub height: u32,
    /// Cursor column (0-based, must be < `width`).
    pub cursor_col: u32,
    /// Cursor row (0-based, must be < `height`).
    pub cursor_row: u32,
    /// Opaque cell-data payload (domain-owned encoding).
    pub cells: Vec<u8>,
}

impl CellGridRender {
    /// Size of the fixed header in bytes.
    pub const HEADER_LEN: usize = 20;

    /// Construct a cell-grid render target.
    #[must_use]
    pub const fn new(
        width: u32,
        height: u32,
        cursor_col: u32,
        cursor_row: u32,
        cells: Vec<u8>,
    ) -> Self {
        Self {
            width,
            height,
            cursor_col,
            cursor_row,
            cells,
        }
    }
}

/// Codec for [`CellGridRender`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CellGridRenderCodec;

impl CellGridRenderCodec {
    /// Construct a new codec instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Codec for CellGridRenderCodec {
    fn kind(&self) -> u16 {
        KIND_CELL_GRID
    }

    fn encode(&self, value: &dyn Any) -> Result<Vec<u8>, RenderPayloadError> {
        let target =
            value
                .downcast_ref::<CellGridRender>()
                .ok_or(RenderPayloadError::InvalidData {
                    reason: "expected CellGridRender",
                })?;
        // Reject cursor out of bounds at encode time — the codec frames
        // valid payloads only. Zero-dimension grids have no valid cursor
        // position, but cells_len may still be nonzero for a pre-resize
        // buffer; the invariant is cursor < dim when dim > 0, cursor == 0
        // when dim == 0.
        if (target.width != 0 && target.cursor_col >= target.width)
            || (target.height != 0 && target.cursor_row >= target.height)
            || (target.width == 0 && target.cursor_col != 0)
            || (target.height == 0 && target.cursor_row != 0)
        {
            return Err(RenderPayloadError::InvalidData {
                reason: "cursor out of bounds",
            });
        }
        let cells_len =
            u32::try_from(target.cells.len()).map_err(|_| RenderPayloadError::InvalidData {
                reason: "cells payload exceeds u32::MAX",
            })?;
        let mut buf = Vec::with_capacity(CellGridRender::HEADER_LEN + target.cells.len());
        buf.extend_from_slice(&target.width.to_be_bytes());
        buf.extend_from_slice(&target.height.to_be_bytes());
        buf.extend_from_slice(&target.cursor_col.to_be_bytes());
        buf.extend_from_slice(&target.cursor_row.to_be_bytes());
        buf.extend_from_slice(&cells_len.to_be_bytes());
        buf.extend_from_slice(&target.cells);
        Ok(buf)
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, RenderPayloadError> {
        // Two-step length validation: header first, then header + cells_len.
        if body.len() < CellGridRender::HEADER_LEN {
            return Err(RenderPayloadError::TooShort {
                got: body.len(),
                min: CellGridRender::HEADER_LEN,
            });
        }
        let width = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
        let height = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);
        let cursor_col = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
        let cursor_row = u32::from_be_bytes([body[12], body[13], body[14], body[15]]);
        let cells_len = u32::from_be_bytes([body[16], body[17], body[18], body[19]]) as usize;
        let required = CellGridRender::HEADER_LEN.saturating_add(cells_len);
        if body.len() < required {
            return Err(RenderPayloadError::TooShort {
                got: body.len(),
                min: required,
            });
        }
        if (width != 0 && cursor_col >= width)
            || (height != 0 && cursor_row >= height)
            || (width == 0 && cursor_col != 0)
            || (height == 0 && cursor_row != 0)
        {
            return Err(RenderPayloadError::InvalidData {
                reason: "cursor out of bounds",
            });
        }
        let cells =
            body[CellGridRender::HEADER_LEN..CellGridRender::HEADER_LEN + cells_len].to_vec();
        Ok(Box::new(CellGridRender {
            width,
            height,
            cursor_col,
            cursor_row,
            cells,
        }))
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
