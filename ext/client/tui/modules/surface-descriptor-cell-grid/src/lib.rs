#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Cell-grid surface descriptor handler (kind = 0x0001).
//!
//! Decodes an 8-byte body `[width_be:u32, height_be:u32]` from the
//! `SurfaceDescriptorProto` envelope into a typed
//! [`CellGridSurfaceInfo`] `{ width: u16, height: u16 }`. Values
//! exceeding `u16::MAX` are rejected as `OutOfRange` rather than
//! silently truncated.
//!
//! This is the first production consumer of the Plan 17 codec
//! foundation; it is statically pre-registered in the TUI startup
//! path so the [`SurfaceDescriptorHandlerRegistry`] is populated
//! before the notification loop receives its first `SurfaceChanged`
//! message.

use {
    reovim_client_subsys_codec::{
        SurfaceApplyContext, SurfaceDescriptorApplyError, SurfaceDescriptorHandler,
        SurfaceDescriptorHandlerError,
    },
    std::any::Any,
};

/// Kind discriminant claimed by the cell-grid surface descriptor
/// handler.
pub const KIND_CELL_GRID: u16 = 0x0001;

/// Minimum body length required by the cell-grid descriptor
/// (`u32` width + `u32` height, big-endian).
pub const CELL_GRID_BODY_LEN: usize = 8;

/// Typed descriptor produced by [`CellGridSurfaceHandler::decode`].
///
/// Callers downcast the boxed value via
/// `Box::downcast::<CellGridSurfaceInfo>()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellGridSurfaceInfo {
    /// New grid width in cells.
    pub width: u16,
    /// New grid height in cells.
    pub height: u16,
}

/// Handler for the cell-grid surface descriptor (kind = `0x0001`).
///
/// Stateless — the same instance may be registered once and reused
/// for every incoming `SurfaceChanged` notification.
#[derive(Debug, Default, Clone, Copy)]
pub struct CellGridSurfaceHandler;

impl CellGridSurfaceHandler {
    /// Create a new handler instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Private helper: parse the 8-byte cell-grid body into a typed
/// descriptor without the `Box<dyn Any + Send>` indirection.
///
/// Used by both [`CellGridSurfaceHandler::decode`] (which boxes the
/// result for the trait's object-safe return) and
/// [`CellGridSurfaceHandler::decode_and_apply`] (which uses the result
/// directly). Extracting this eliminates the `.expect` downcast-panic
/// site that the trait-level decode required prior to Plan 17-β.2b.
fn decode_info(body: &[u8]) -> Result<CellGridSurfaceInfo, SurfaceDescriptorHandlerError> {
    if body.len() < CELL_GRID_BODY_LEN {
        return Err(SurfaceDescriptorHandlerError::TooShort {
            got: body.len(),
            min: CELL_GRID_BODY_LEN,
        });
    }
    let width_u32 = u32::from_be_bytes([body[0], body[1], body[2], body[3]]);
    let height_u32 = u32::from_be_bytes([body[4], body[5], body[6], body[7]]);

    let width = u16::try_from(width_u32).map_err(|_| {
        SurfaceDescriptorHandlerError::OutOfRange {
            reason: "width exceeds u16::MAX",
        }
    })?;
    let height = u16::try_from(height_u32).map_err(|_| {
        SurfaceDescriptorHandlerError::OutOfRange {
            reason: "height exceeds u16::MAX",
        }
    })?;

    Ok(CellGridSurfaceInfo { width, height })
}

impl SurfaceDescriptorHandler for CellGridSurfaceHandler {
    fn kind(&self) -> u16 {
        KIND_CELL_GRID
    }

    fn decode(
        &self,
        body: &[u8],
    ) -> Result<Box<dyn Any + Send>, SurfaceDescriptorHandlerError> {
        decode_info(body).map(|info| {
            let boxed: Box<dyn Any + Send> = Box::new(info);
            boxed
        })
    }

    fn decode_and_apply(
        &self,
        body: &[u8],
        ctx: &mut dyn SurfaceApplyContext,
    ) -> Result<(), SurfaceDescriptorApplyError> {
        let info = decode_info(body)?;
        if info.width == 0 || info.height == 0 {
            return Err(SurfaceDescriptorApplyError::StateApplyRejected {
                reason: "zero surface dimension",
            });
        }
        ctx.set_surface_size(info.width, info.height);
        Ok(())
    }
}

#[cfg(test)]
mod lib_tests;
