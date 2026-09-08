//! Bounded read-only causal and semantic-state evidence, not editing authority.

use alloc::{boxed::Box, vec::Vec};
use core::fmt;

use crate::bound::{CheckedByteCount, CheckedCount};
use crate::buffer::BufferStateStamp;
use crate::cancellation::DeadlineState;
use crate::interaction::identity::InteractionRequestIdentity;
use crate::interaction::session::InteractionSessionHandle;
use crate::interaction::update::{
    RepresentationUpdateCarrierStamp, RepresentationUpdateIdentityStamp,
};
use crate::lifecycle::LifecycleIdentity;
use crate::semantic::stamp::{CoordinateSpaceStamp, ProjectionStamp};

use super::EventSequence;
use super::editing::{EditingContext, EditingStage};

/// Node identity never loses its producing stream incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceLocation {
    /// Exact diagnostic stream lifecycle.
    pub stream: LifecycleIdentity,
    /// Actual admitted event sequence, not an inferred timestamp relation.
    pub sequence: EventSequence,
}

/// Query seeds; request/update seeds include all matching retained stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceStart {
    /// Reverse the incoming causes of one actual event.
    Event(TraceLocation),
    /// Include actual fan-out of an exact origin request, then its ancestry.
    Request {
        /// Origin, not the receiving observer.
        session: InteractionSessionHandle,
        /// Request identity within that Session incarnation.
        request: InteractionRequestIdentity,
    },
    /// Include delivery/client stages of one exact qualified update.
    Update(RepresentationUpdateIdentityStamp),
}

/// Store limits apply before allocation; graph and captures are not the ring.
#[derive(Clone, Copy, Debug)]
pub struct TraceLimits {
    /// Maximum graph nodes; each has at most two explicit incoming edges.
    pub nodes: CheckedCount,
    /// Maximum simultaneously retained semantic snapshots.
    pub snapshots: CheckedCount,
    /// Combined graph/snapshot metadata allocation, including spare capacity.
    pub metadata_bytes: CheckedByteCount,
    /// Sum of retained snapshot payload allocations.
    pub payload_bytes: CheckedByteCount,
}

/// Caller-bounded traversal; no clock inference or unlimited recursive walk.
#[derive(Clone, Copy, Debug)]
pub struct TraceQueryBudget {
    /// Follow the primary causal relation.
    pub primary: bool,
    /// Follow separately recorded joins.
    pub joins: bool,
    /// Returned nodes and traversal frontier capacity.
    pub nodes: CheckedCount,
    /// Incoming edge examinations.
    pub edges: CheckedCount,
    /// Maximum distance from any selected seed.
    pub depth: CheckedCount,
    /// Store/edge examinations, including seed lookup.
    pub work: CheckedCount,
    /// All query-owned allocation, including spare vector capacity.
    pub bytes: CheckedByteCount,
    /// Deadline observation supplied by the independently clocked caller.
    pub deadline: DeadlineState,
}

/// Safe scalar node; parents retain the same stream as this event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceNode {
    /// Exact source event identity.
    pub location: TraceLocation,
    /// Stage actually observed, never filled in from a correlation alone.
    pub stage: EditingStage,
    /// Clock incarnation qualifying these elapsed ticks.
    pub clock: LifecycleIdentity,
    /// Actual observation, not reconstructed from neighboring records.
    pub nanoseconds: u64,
    /// Origin, observer, update/state and up to two causal parents.
    pub context: EditingContext,
}

/// Known limitations of retained/traversed evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceCut {
    /// Every requested incoming edge reached a retained node or known root.
    Complete,
    /// No captured node matched the requested origin/update/event.
    NotCaptured,
    /// A referenced admitted record was evicted from graph retention.
    Evicted,
    /// Depth, work, node, edge or output allocation budget was reached.
    Budget,
}

/// Explicit missing parent, not a made-up bridge over lost observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceGap {
    /// The missing source identity, still qualified by its stream.
    pub location: TraceLocation,
    /// Why it could not be included.
    pub cause: TraceCut,
}

/// Read-only captured subgraph. A budget cut never means complete ancestry.
#[derive(Debug)]
pub struct TraceReport {
    /// Actual retained seeds and ancestors; relations live in node contexts.
    pub nodes: Vec<TraceNode>,
    /// Missing exact parents, without guessed relationships.
    pub gaps: Vec<TraceGap>,
    /// Completeness of this bounded query.
    pub cut: TraceCut,
    /// Earliest retained graph event, not a claim about earlier capture.
    pub oldest: Option<TraceLocation>,
}

/// Qualified snapshot metadata contains no claim of native-source capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticCaptureKey {
    /// Exact produced update, independent of its delivery cursor.
    pub update: RepresentationUpdateIdentityStamp,
    /// Exact captured state; a caller cannot substitute another observation.
    pub state: BufferStateStamp,
}

/// Qualified snapshot metadata contains no claim of native-source capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticCaptureStamp {
    /// Actual produced snapshot/update identity.
    pub update: RepresentationUpdateIdentityStamp,
    /// Exact immutable Buffer version that the producer interpreted.
    pub state: BufferStateStamp,
    /// Resulting Projection and its semantic Representation identity.
    pub projection: ProjectionStamp,
    /// Meaning and units of semantic coordinates, not a text position.
    pub coordinate: CoordinateSpaceStamp,
    /// Selected semantic carrier family/version.
    pub schema: RepresentationUpdateCarrierStamp,
}

/// Authorized copy of actually captured semantic state.
#[derive(Debug)]
pub struct SemanticCapture {
    /// Exact source qualification, retained with the payload.
    pub stamp: SemanticCaptureStamp,
    /// Snapshot bytes, not the full native Resource or today's state.
    pub payload: Box<[u8]>,
}

/// Typed query/capture failure cannot affect editing acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceError {
    /// No matching debug read grant permits disclosure.
    Denied,
    /// Stream/grant or requested producer generation differs.
    Replaced,
    /// The requested state was not captured under this selected profile.
    NotCaptured,
    /// Required evidence has left bounded retention.
    Evicted,
    /// Limits, allocation or non-waiting local ownership prevent completion.
    Pressure,
    /// The independent caller observed the query deadline.
    Deadline,
    /// Qualification or graph ancestry is incoherent.
    Invalid,
}

impl fmt::Display for TraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "trace evidence: {self:?}")
    }
}
impl core::error::Error for TraceError {}

/// Public debug consumer port; grants are issued explicitly by assembly.
pub trait TraceReadPort {
    /// Returns bounded actual incoming evidence from the selected stream.
    ///
    /// # Errors
    /// Returns denied, stale, expired deadline or bounded-storage failure.
    fn backtrace(
        &self,
        grant: LifecycleIdentity,
        start: TraceStart,
        budget: TraceQueryBudget,
    ) -> Result<TraceReport, TraceError>;

    /// Copies one captured semantic version under its own output-byte budget.
    ///
    /// # Errors
    /// Returns denied, absent/evicted, replaced or bounded-copy failure.
    fn captured(
        &self,
        grant: LifecycleIdentity,
        key: &SemanticCaptureKey,
        bytes: CheckedByteCount,
    ) -> Result<SemanticCapture, TraceError>;
}
