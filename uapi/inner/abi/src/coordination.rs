//! 6.3 §5 — Coordination carriers.
//!
//! `PositionCarrier` and `CursorCarrier` cross the boundary as an 8-byte
//! opaque header followed by a [`crate::slices::ByteSlice`] content.
//! The wire-level pair is exposed here; byte-layout semantics are defined
//! in CR2.
use crate::slices::ByteSlice;

/// Opaque 8-byte position header (6.3 §5, CR2).
///
/// ```rust
/// use reovim_uapi_abi::coordination::PositionHeader;
///
/// let h = PositionHeader { bytes: [0u8; 8] };
/// assert_eq!(h.bytes.len(), 8);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionHeader {
    /// Opaque 8-byte header; byte-layout per CR2.
    pub bytes: [u8; 8],
}

/// Opaque 8-byte cursor header (6.3 §5, CR2).
///
/// ```rust
/// use reovim_uapi_abi::coordination::CursorHeader;
///
/// let h = CursorHeader { bytes: [1u8; 8] };
/// assert_eq!(h.bytes[0], 1);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorHeader {
    /// Opaque 8-byte header; byte-layout per CR2.
    pub bytes: [u8; 8],
}

/// Wire-level position carrier: header + content slice (6.3 §5).
///
/// ```rust
/// use reovim_uapi_abi::coordination::{PositionCarrierWire, PositionHeader};
/// use reovim_uapi_abi::slices::ByteSlice;
/// use core::ptr;
///
/// let wire = PositionCarrierWire {
///     header:  PositionHeader { bytes: [0u8; 8] },
///     content: ByteSlice { data: ptr::null(), len: 0 },
/// };
/// assert_eq!(wire.header.bytes, [0u8; 8]);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PositionCarrierWire {
    /// Opaque position header.
    pub header: PositionHeader,
    /// Position content bytes, caller-owned-for-call.
    pub content: ByteSlice,
}

/// Wire-level cursor carrier: header + content slice (6.3 §5).
///
/// ```rust
/// use reovim_uapi_abi::coordination::{CursorCarrierWire, CursorHeader};
/// use reovim_uapi_abi::slices::ByteSlice;
/// use core::ptr;
///
/// let wire = CursorCarrierWire {
///     header:  CursorHeader { bytes: [0u8; 8] },
///     content: ByteSlice { data: ptr::null(), len: 0 },
/// };
/// assert_eq!(wire.header.bytes, [0u8; 8]);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CursorCarrierWire {
    /// Opaque cursor header.
    pub header: CursorHeader,
    /// Cursor content bytes, caller-owned-for-call.
    pub content: ByteSlice,
}
