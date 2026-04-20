#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Render codec uapi — opaque envelope + `Codec` trait for domain-neutral
//! render targets.
//!
//! This crate is the closed-mechanism tier of the Plan 14 Phase R
//! three-tier pattern (mirror of the Phase S surface-codec uapi):
//!
//! ```text
//! uapi/render-codec/              ← CLOSED mechanism (this crate)
//!   RenderDescriptor + Codec trait + header accessors
//!
//! server/lib/subsys/render-codec/ ← CONTRACT + REGISTRY (Phase R.2)
//!   RenderCodecRegistry trait + default impl
//!
//! ext/render-codec/<variant>/     ← PER-VARIANT IMPL (Phase R.3+)
//!   ext/render-codec/tui/         CellGridRender codec (kind 0x0001)
//! ```
//!
//! The uapi carries no cell layout, style vocabulary, or platform
//! interpretation — concrete render shapes (cell-grid, pixel, ...) live
//! in the corresponding `ext/render-codec/<variant>/` module crates.

mod descriptor;

#[cfg(test)]
mod descriptor_tests;

pub use descriptor::{
    // Well-known kinds (in-repo allocation, 0x0001–0x00FF).
    KIND_CELL_GRID,
    RenderDescriptor,
    render_body,
    render_kind,
};

use std::any::Any;

/// Error returned when a render codec cannot decode a payload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderPayloadError {
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

impl std::fmt::Display for RenderPayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { got, min } => {
                write!(f, "render payload too short: got {got} bytes, need at least {min}")
            }
            Self::WrongType { expected, actual } => {
                write!(
                    f,
                    "render payload wrong type: expected kind {expected:#06x}, got {actual:#06x}"
                )
            }
            Self::InvalidData { reason } => write!(f, "render payload invalid: {reason}"),
        }
    }
}

impl std::error::Error for RenderPayloadError {}

/// Contract for render codecs.
///
/// Implementations decode `RenderDescriptor::body` bytes into a concrete
/// render-target value (e.g. `CellGridRender { width, height, ... }`) and
/// encode concrete render values back into bytes for wire transport.
pub trait Codec: Send + Sync + 'static {
    /// The kind value this codec handles. Must be unique per concern.
    fn kind(&self) -> u16;

    /// Encode a concrete render value back into bytes.
    ///
    /// # Errors
    ///
    /// Returns [`RenderPayloadError::InvalidData`] if the value cannot be
    /// encoded, [`RenderPayloadError::WrongType`] if the `value` type is
    /// not the one this codec handles.
    fn encode(&self, value: &dyn Any) -> Result<Vec<u8>, RenderPayloadError>;

    /// Decode body bytes into a type-erased concrete render value.
    ///
    /// # Errors
    ///
    /// Returns [`RenderPayloadError::TooShort`] if `body` is smaller than
    /// the codec's minimum length, [`RenderPayloadError::InvalidData`] on
    /// malformed body.
    fn decode(&self, body: &[u8]) -> Result<Box<dyn Any + Send>, RenderPayloadError>;
}
