//! `reovim-subsys-domain` — the Domain contract tier (DAG2).
//!
//! This crate holds the contract surface shared between the kernel (which
//! routes through it) and `ext/server/domain/*` Domains (which implement it).
//! Per the layer DAG, the contract tier depends only on Foundation
//! (`reovim-arch`); both `ServerKernel` and `ServerExt` are permitted to
//! depend on `ServerContracts`, but not on each other. Moving these types here
//! is what lets `reovim-domain-text` drop its kernel dependency without
//! breaking the kernel ← contract ← ext layering.
//!
//! ## Contract surface
//!
//! | Module | Types |
//! |---|---|
//! | [`id`] | `DomainId`, `BufferId`, `WindowId` |
//! | [`contract`] | `OnRawInputHandler`, `RenderProjector` |
//! | [`projection`] | `Projection`, `ProjectionSpan`, `ProjectionDecodeError` |
//! | [`carrier`] | `PositionCarrier`, `CursorCarrier`, carrier headers |
//! | [`routing`] | manifests, handler/projector ids, dispatch row metadata |
//! | [`tree`] | attachments, focus chains, pending records, transitions |
//! | [`state`] | view-slot keys, register scopes, undo/edit records |
//! | [`service`] | service flags, row states, descriptor metadata, leases |
//! | [`stream`] | stream state, backpressure, scheme/handle metadata |
//!
//! The kernel re-exports these at their historical paths
//! (`reovim_kernel::router::*`, `reovim_kernel::projection::*`,
//! `reovim_kernel::session::{BufferId, WindowId}`) so existing consumers compile
//! unchanged.
#![no_std]

pub mod carrier;
pub mod contract;
pub mod id;
pub mod projection;
pub mod routing;
pub mod service;
pub mod state;
pub mod stream;
pub mod tree;

// Re-export the contract types at the crate root for ergonomic consumption.
pub use {
    carrier::{
        CarrierStatus, CursorCarrier, CursorHeader, KERNEL_FLAG, PositionCarrier, PositionHeader,
    },
    contract::{OnRawInputHandler, RawInputResult, RenderProjector},
    id::{
        BufferId, CdylibId, ClientId, DomainAttachmentId, DomainId, PendingAttachmentId,
        RegisterId, ReplayQueueId, ServiceKey, ServiceLeaseId, SessionId, SlotKindId, StreamId,
        WindowId,
    },
    projection::{
        FullProjection, FullProjectionSpan, GlyphHint, OverlayBlob, Projection,
        ProjectionDecodeError, ProjectionDelivery, ProjectionRange, ProjectionSlotKey,
        ProjectionSpan, StyleRef,
    },
    routing::{
        BandedPriority, DispatchEntryMeta, DomainApiVersion, DomainManifest, HandlerEntry,
        HandlerId, PriorityBand, PriorityRange, ProjectorEntry, ProjectorId, RegistrationError,
        RegistrationPhase, RowKind,
    },
    service::{
        ServiceAccessError, ServiceBorrow, ServiceDescriptorMeta, ServiceFlags, ServiceLease,
        ServiceRowState,
    },
    state::{
        EditOrigin, EditRecord, RegisterKey, RegisterScope, UndoGroup, UndoGroupId, UndoStack,
        ViewSlotFlags, ViewSlotKey, ViewSlotScope,
    },
    stream::{
        BackpressureBlock, BackpressureCounters, StreamControlClass, StreamControlOp, StreamHandle,
        StreamOpts, StreamScheme, StreamState,
    },
    tree::{
        DispatchVerdict, DomainAllocError, DomainAttachment, DomainScope, FocusChain, FocusEntry,
        FocusEntrySnapshot, FocusTransition, PendingAttachment,
    },
};

// ── L12 sibling test files (kernel-selftest bin runs these) ───────────────────
//
// Tests live in sibling `*_tests.rs` files compiled under the `selftest`
// feature; there are no inline `#[cfg(test)] mod tests {}` blocks anywhere in
// this crate (checked by scripts/check-test-layout.sh).

#[cfg(feature = "selftest")]
#[path = "id_tests.rs"]
mod id_tests;

#[cfg(feature = "selftest")]
#[path = "contract_tests.rs"]
mod contract_tests;

#[cfg(feature = "selftest")]
#[path = "carrier_tests.rs"]
mod carrier_tests;

#[cfg(feature = "selftest")]
#[path = "projection_tests.rs"]
mod projection_tests;

#[cfg(feature = "selftest")]
#[path = "routing_tests.rs"]
mod routing_tests;

#[cfg(feature = "selftest")]
#[path = "tree_tests.rs"]
mod tree_tests;

#[cfg(feature = "selftest")]
#[path = "state_tests.rs"]
mod state_tests;

#[cfg(feature = "selftest")]
#[path = "service_tests.rs"]
mod service_tests;

#[cfg(feature = "selftest")]
#[path = "stream_tests.rs"]
mod stream_tests;
