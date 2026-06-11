//! 6.3 §9 — Domain tree types.
//!
//! `FocusTransition` (Q-4) carries the paired before/after snapshots of the
//! focus chain for one transition.  Lifetime follows the same init-call
//! pattern as [`crate::config::ConfigSlice`]: kernel-owned during the
//! observer callback; the observer copies what it retains.
use core::ffi::c_void;

use crate::{
    coordination::PositionCarrierWire,
    ids::{BufferId, ClientId, DomainAttachmentId, DomainId, PendingAttachmentId, WindowId},
};

/// A focus-chain entry on the wire: either pending or resolved (6.3 §9).
///
/// Uses `#[repr(C, u8)]` (payload-carrying enum convention from 6.3 §1).
///
/// ```rust
/// use reovim_uapi_abi::tree::FocusEntryWire;
/// use reovim_uapi_abi::ids::DomainAttachmentId;
///
/// let entry = FocusEntryWire::Resolved(DomainAttachmentId(7));
/// if let FocusEntryWire::Resolved(id) = entry {
///     assert_eq!(id.0, 7);
/// }
/// ```
#[repr(C, u8)]
#[derive(Debug, Clone, Copy)]
pub enum FocusEntryWire {
    /// Attachment is still being resolved.
    Pending(PendingAttachmentId),
    /// Attachment is fully resolved.
    Resolved(DomainAttachmentId),
}

/// A focus-chain snapshot entry with its domain identity (6.3 §9).
///
/// ```rust
/// use reovim_uapi_abi::tree::FocusEntrySnapshot;
/// use reovim_uapi_abi::ids::{DomainAttachmentId, DomainId};
/// use core::num::NonZeroU32;
///
/// let snap = FocusEntrySnapshot::Resolved {
///     id: DomainAttachmentId(1),
///     domain_id: DomainId(NonZeroU32::new(2).unwrap()),
/// };
/// if let FocusEntrySnapshot::Resolved { id, .. } = snap {
///     assert_eq!(id.0, 1);
/// }
/// ```
#[repr(C, u8)]
#[derive(Debug, Clone, Copy)]
pub enum FocusEntrySnapshot {
    /// Pending attachment snapshot.
    Pending {
        /// Pending attachment identifier.
        id: PendingAttachmentId,
        /// Domain this attachment targets.
        domain_id: DomainId,
    },
    /// Resolved attachment snapshot.
    Resolved {
        /// Resolved attachment identifier.
        id: DomainAttachmentId,
        /// Domain this attachment is resolved to.
        domain_id: DomainId,
    },
}

/// Focus-chain transition event (Q-4, 6.3 §9).
///
/// Carries the paired before/after snapshots of the focus chain for one
/// transition.  All pointers are kernel-owned; the observer must copy
/// anything it retains before the callback returns.
///
/// ```rust
/// use reovim_uapi_abi::tree::FocusTransition;
/// use reovim_uapi_abi::ids::{BufferId, ClientId, WindowId};
/// use core::ptr;
///
/// let ft = FocusTransition {
///     session_id_handle: ptr::null(),
///     client_id: ClientId(0),
///     buffer_id: BufferId(0),
///     window_id: WindowId(0),
///     seq: 0,
///     before: ptr::null(),
///     before_len: 0,
///     after: ptr::null(),
///     after_len: 0,
/// };
/// assert_eq!(ft.seq, 0);
/// ```
#[repr(C)]
pub struct FocusTransition {
    /// Opaque session handle (`SessionId` is `Arc<str>`; crosses boundary as
    /// `*const c_void`).
    pub session_id_handle: *const c_void,
    /// Client that experienced the transition.
    pub client_id: ClientId,
    /// Active buffer at time of transition.
    pub buffer_id: BufferId,
    /// Active window at time of transition.
    pub window_id: WindowId,
    /// Monotonic sequence number per session.
    pub seq: u64,
    /// Previous focus chain; `before_len` entries.
    pub before: *const FocusEntrySnapshot,
    /// Number of entries in `before`.
    pub before_len: usize,
    /// New focus chain; `after_len` entries.
    pub after: *const FocusEntrySnapshot,
    /// Number of entries in `after`.
    pub after_len: usize,
}

/// Wire representation of a domain scope: start + end position carriers
/// plus flags (6.3 §9).
///
/// ```rust
/// use reovim_uapi_abi::tree::DomainScopeWire;
/// use reovim_uapi_abi::coordination::{PositionCarrierWire, PositionHeader};
/// use reovim_uapi_abi::slices::ByteSlice;
/// use core::ptr;
///
/// let zero_carrier = PositionCarrierWire {
///     header:  PositionHeader { bytes: [0u8; 8] },
///     content: ByteSlice { data: ptr::null(), len: 0 },
/// };
/// let scope = DomainScopeWire {
///     start: zero_carrier,
///     end:   zero_carrier,
///     flags: 0,
///     pad:  [0u8; 7],
/// };
/// assert_eq!(scope.flags, 0);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DomainScopeWire {
    /// Start-of-scope position carrier.
    pub start: PositionCarrierWire,
    /// End-of-scope position carrier.
    pub end: PositionCarrierWire,
    /// Scope flags (semantics TBD per caller context).
    pub flags: u8,
    /// Padding to maintain alignment.
    pub pad: [u8; 7],
}
