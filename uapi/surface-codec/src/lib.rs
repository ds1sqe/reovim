#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Surface codec uapi — opaque envelope + `Codec` trait for domain-neutral
//! rendering surfaces.
//!
//! This crate is the closed-mechanism tier of the Plan 14 Phase S
//! three-tier pattern:
//!
//! ```text
//! uapi/surface-codec/              ← CLOSED mechanism (this crate)
//!   SurfaceDescriptor + Codec trait + header accessors
//!
//! server/lib/subsys/surface-codec/ ← CONTRACT + REGISTRY (Phase S.2)
//!   SurfaceCodecRegistry trait + default impl
//!
//! ext/surface-codec/<variant>/     ← PER-VARIANT IMPL (Phase S.3–S.4)
//!   ext/surface-codec/tui/         CellGridSurface codec (kind 0x0001)
//!   ext/surface-codec/pixel/       PixelSurface codec (kind 0x0002)
//! ```
//!
//! The uapi carries no geometry, rendering model, or platform vocabulary —
//! concrete surface shapes (cell grids, pixel buffers, ...) live in the
//! corresponding `ext/surface-codec/<variant>/` module crates.

mod descriptor;

#[cfg(test)]
mod descriptor_tests;

pub use descriptor::{
    // Well-known kinds (in-repo allocation, 0x0001–0x00FF).
    KIND_CELL_GRID,
    KIND_PIXEL_BUFFER,
    SurfaceDescriptor,
    surface_body,
    surface_kind,
};

use std::any::Any;

/// Error returned when a surface codec cannot decode a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfacePayloadError {
    /// Body shorter than the minimum expected for this kind.
    TooShort {
        /// Number of bytes received.
        got: usize,
        /// Minimum bytes required.
        min: usize,
    },
    /// Kind byte does not match this codec.
    WrongType {
        /// Expected kind value this codec handles.
        expected: u16,
        /// Actual kind value in the envelope.
        actual: u16,
    },
    /// Body bytes are well-formed length-wise but fail a codec-specific
    /// semantic check.
    InvalidData {
        /// Stringified error class.
        reason: &'static str,
    },
}

impl std::fmt::Display for SurfacePayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { got, min } => {
                write!(f, "surface payload too short: got {got} bytes, need at least {min}")
            }
            Self::WrongType { expected, actual } => {
                write!(
                    f,
                    "surface payload wrong type: expected kind {expected:#06x}, got {actual:#06x}"
                )
            }
            Self::InvalidData { reason } => write!(f, "surface payload invalid: {reason}"),
        }
    }
}

impl std::error::Error for SurfacePayloadError {}

/// Contract for surface codecs.
///
/// Implementations decode `SurfaceDescriptor::body` bytes into a concrete
/// surface value (e.g. `CellGridSurface { width, height }`) and encode
/// concrete surface values back into bytes for wire transport.
pub trait Codec: Send + Sync + 'static {
    /// The kind value this codec handles. Must be unique per concern.
    fn kind(&self) -> u16;

    /// Encode a concrete surface value back into bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SurfacePayloadError::InvalidData`] if the value cannot be
    /// encoded, [`SurfacePayloadError::WrongType`] if the `value` type is
    /// not the one this codec handles.
    fn encode(&self, value: &dyn Any) -> Result<Vec<u8>, SurfacePayloadError>;

    /// Decode body bytes into a type-erased concrete surface value.
    ///
    /// # Errors
    ///
    /// Returns [`SurfacePayloadError::TooShort`] if `body` is smaller than
    /// the codec's minimum length, [`SurfacePayloadError::InvalidData`] on
    /// malformed body.
    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, SurfacePayloadError>;
}
