//! Root readiness, pre-open client attachment and scoped endpoint discovery.
//!
//! A root coordinates owners; it is not another Buffer/History authority.
//! Copyable request metadata is retained unchanged by callers on refusal.

use alloc::vec::Vec;
use core::fmt;

use crate::bound::CheckedCount;
use crate::buffer::{BufferIncarnationStamp, BufferStateStamp};
use crate::cancellation::{
    CancellationCause, CancellationOutcome, DeadlineState,
};
use crate::lifecycle::{
    LifecycleIdentity, LifecycleState, ReadinessOutcome, RollbackOutcome,
    ShutdownOutcome,
};
use crate::namespace::ResourceEntry;
use crate::resource::ResourceId;

use super::identity::InteractionRequestIdentity;
use super::port::InteractionSessionPort;
use super::session::InteractionClientStamp;

/// Aggregate root-owned reservations; exceeding them never admits work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootLimits {
    /// Concurrent client attachments, each with separately bounded Sessions.
    pub clients: CheckedCount,
    /// Outstanding open requests and retained terminal/replay records.
    pub opens: CheckedCount,
    /// Maximum work admitted by one root operation.
    pub work: CheckedCount,
}

/// Exact root generation and bounded lifecycle work requested by its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootLifecycleRequest {
    /// Current lifecycle owner, not a replacement generation.
    pub root: LifecycleIdentity,
    /// Work reserved before startup or bounded teardown.
    pub work: CheckedCount,
    /// Deadline state observed by the outside owner; not proof work stopped.
    pub deadline: DeadlineState,
}

/// Client identity plus an already issued, live authorization handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootAttachRequest {
    /// Root generation to which this authorization applies.
    pub root: LifecycleIdentity,
    /// Independently owned Client Runtime generation and selected role.
    pub client: InteractionClientStamp,
    /// Root-issued authorization binding exact client, resource scope and
    /// permitted open/join operations. Root rechecks its live record; this
    /// structural value does not grant authority by itself.
    pub authorization: LifecycleIdentity,
}

/// Live root attachment before any Resource or Buffer need be open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootClientHandle {
    /// Root owner retaining this attachment and its granted scope.
    pub root: LifecycleIdentity,
    /// Exact independently owned Client Runtime.
    pub client: InteractionClientStamp,
    /// Root-issued attachment incarnation and replacement fence.
    pub attachment: LifecycleIdentity,
}

/// Independent editing context or authorized sharing of an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootOpenTarget {
    /// Open a granted source; Runtime selects the native adapter request.
    Independent(ResourceId),
    /// Join this exact Buffer context; never infer sharing from a locator.
    Existing(ResourceId),
}

/// One client-scoped open, with no client-selected capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootOpenRequest {
    /// Live attachment; its grants are checked before resource lookup.
    pub client: RootClientHandle,
    /// Request identity within this attachment's lifetime.
    pub request: InteractionRequestIdentity,
    /// Explicit source or existing Buffer in this attachment's namespace.
    pub target: RootOpenTarget,
    /// Work reservation, including bounded initialization/attachment work.
    pub work: CheckedCount,
}

/// Exact lookup/cancellation identity issued after open admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootOpenTicket {
    /// Attachment owning this request and its terminal reservation.
    pub client: RootClientHandle,
    /// Admitted request within that exact lifetime.
    pub request: InteractionRequestIdentity,
}

/// Public Buffer state after resource readiness and initialization succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootOpenedBuffer {
    /// Exact initialized or joined state; not a provisional Buffer stamp.
    pub state: BufferStateStamp,
    /// Granted local editing capability, distinct from source publication.
    pub editable: bool,
    /// Granted shared-History operations, still subject to edge eligibility.
    pub history: bool,
}

/// Failed open, including the actual provisional-owner rollback disposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootOpenFailure {
    /// Typed reason no endpoint was published.
    pub cause: RootFailure,
    /// Incomplete rollback remains owned by the root/Resource owner even
    /// after this terminal result is delivered; it is not discarded work.
    pub rollback: RollbackOutcome,
}

/// Exactly one terminal open result, separately retrieved after admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootOpenCompletion {
    /// Exact attachment/request receiving this result.
    pub ticket: RootOpenTicket,
    /// Published Buffer metadata or an explicit failed-open rollback.
    pub outcome: Result<RootOpenedBuffer, RootOpenFailure>,
}

/// Root admission, lookup or readiness failure without implicit side effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootFailure {
    /// No matching live authorization/grant permits this action.
    Denied,
    /// Required root/resource dependencies have not reached readiness.
    NotReady,
    /// Requested resource/operation capability is unsupported.
    Unsupported,
    /// Required state or owner is unavailable.
    Unavailable,
    /// Identity, target or structural request fields are invalid.
    InvalidRequest,
    /// A repeated request identity names different immutable request metadata.
    ConflictingRequest,
    /// Root/client/Buffer/adapter lifetime or generation has been replaced.
    Stale,
    /// A declared attachment, retained-result or work limit is exhausted.
    Pressure,
    /// Cancellation fenced the operation before publication.
    Cancelled,
    /// The outside owner observed the operation deadline.
    Deadline,
    /// Root or attachment shutdown has begun.
    Closing,
    /// The owner is closed.
    Closed,
    /// No retained request has this exact ticket.
    UnknownRequest,
    /// Its terminal result has already transferred to the caller.
    AlreadyDelivered,
    /// A selected provider failed without a usable result.
    ProviderFailure,
}

impl fmt::Display for RootFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Interaction root: {self:?}")
    }
}

impl core::error::Error for RootFailure {}

/// Root operations consumed by the first boot/attach/open/close journey.
///
/// Providers check live generations, root-issued grants and bounds before
/// lookup or work. Startup performs bounded work per call; `None` means
/// Starting, and another call drives the same activation without restarting.
/// Failure reports actual rollback via `ReadinessOutcome::ActivationFailed`,
/// never partial Ready.
/// Deferred Resource opens use `open`/`take_open`; `None` means pending.
/// Identical live retries reuse a ticket; changed requests conflict.
///
/// The root chooses no renderer/client state and owns no second editing state.
/// An associated endpoint borrows the same Buffer authority after successful
/// open, then separately negotiates its Representation and initial snapshot.
pub trait InteractionRootPort {
    /// Buffer-scoped public endpoint, with independently owned Session state.
    type Endpoint: InteractionSessionPort;

    /// Returns the exact root generation.
    fn owner(&self) -> LifecycleIdentity;
    /// Returns current lifecycle, not a claim that any Client is ready.
    fn state(&self) -> LifecycleState;
    /// Returns aggregate root reservations.
    fn limits(&self) -> RootLimits;
    /// Starts the root or returns bounded activation/rollback failure.
    ///
    /// # Errors
    /// Returns a stale root, deadline, work-bound or closed-owner failure.
    fn start(
        &mut self,
        request: RootLifecycleRequest,
    ) -> Result<Option<ReadinessOutcome>, RootFailure>;
    /// Attaches an authorized Client without creating/opening a Buffer.
    ///
    /// # Errors
    /// Returns denied, stale, not-ready or attachment-pressure failure.
    fn attach_client(
        &mut self,
        request: RootAttachRequest,
    ) -> Result<RootClientHandle, RootFailure>;
    /// Returns the attachment's authorized namespace root without opening it.
    ///
    /// # Errors
    /// Returns denied, not-ready or closed-owner failure.
    fn root(&self, client: RootClientHandle)
    -> Result<ResourceId, RootFailure>;
    /// Resolves relative to the attachment's root without running the subject.
    ///
    /// # Errors
    /// Returns invalid path, denied, unavailable, pressure or closed failure.
    fn resolve(
        &self,
        client: RootClientHandle,
        path: &str,
    ) -> Result<ResourceId, RootFailure> {
        self.resolve_at(client, self.root(client)?, path)
    }
    /// Resolves a relative path; empty names the parent. Rejects leading `/`,
    /// empty segments, `.` and `..`. Checks access at every component.
    ///
    /// # Errors
    /// Returns invalid path, denied, unavailable, pressure or closed failure.
    fn resolve_at(
        &self,
        client: RootClientHandle,
        parent: ResourceId,
        path: &str,
    ) -> Result<ResourceId, RootFailure>;
    /// Returns a complete, owned listing in name order; leaves may reject it.
    ///
    /// # Errors
    /// Returns denied, unavailable, unsupported, pressure or closed failure.
    fn children(
        &self,
        client: RootClientHandle,
        parent: ResourceId,
    ) -> Result<Vec<ResourceEntry>, RootFailure>;
    /// Reserves one bounded terminal result or refuses unchanged metadata.
    ///
    /// # Errors
    /// Returns invalid authority/target, conflicting retry or bounded pressure.
    fn open(
        &mut self,
        request: RootOpenRequest,
    ) -> Result<RootOpenTicket, RootFailure>;
    /// Retrieves or replays a retained terminal result; rollback stays owned.
    ///
    /// # Errors
    /// Returns stale/denied or unknown-ticket failure.
    fn take_open(
        &mut self,
        ticket: RootOpenTicket,
    ) -> Result<Option<RootOpenCompletion>, RootFailure>;
    /// Cooperatively cancels exact admitted work without claiming rollback.
    ///
    /// # Errors
    /// Returns stale/denied, unknown-ticket or already-delivered failure.
    fn cancel_open(
        &mut self,
        ticket: RootOpenTicket,
        cause: CancellationCause,
    ) -> Result<CancellationOutcome, RootFailure>;
    /// Borrows only an opened/joined endpoint under the client's live grant.
    /// Closing/incomplete owners retain this route for terminal draining;
    /// the endpoint itself fences new admission. A closed root rejects it.
    ///
    /// # Errors
    /// Returns denied/stale, not-open or closed-owner failure.
    fn endpoint(
        &mut self,
        client: RootClientHandle,
        buffer: BufferIncarnationStamp,
    ) -> Result<&mut Self::Endpoint, RootFailure>;
    /// Stops this client's admission and drains its owned Session work.
    /// Shared Buffers/History remain independent; detachment does not save or
    /// discard them. Incomplete work remains owned after the return.
    ///
    /// # Errors
    /// Returns an invalid owner/attachment or a rejected work reservation.
    fn detach_client(
        &mut self,
        client: RootClientHandle,
        budget: RootLifecycleRequest,
    ) -> Result<ShutdownOutcome, RootFailure>;
    /// Stops root admission and boundedly drains children and rollback work.
    /// `Closed` requires all owned execution/releases terminal; otherwise the
    /// root remains live and returns `IncompleteShutdown`. Repeated closes
    /// are idempotent and never unload work merely because its wait ended.
    ///
    /// # Errors
    /// Returns invalid root authority or a rejected work reservation.
    fn shutdown(
        &mut self,
        budget: RootLifecycleRequest,
    ) -> Result<ShutdownOutcome, RootFailure>;
}
