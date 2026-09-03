//! Bounded startup Event contracts without retention or rendering mechanics.

pub use ::reovim_foundation::bound::{
    BoundDescriptor, BoundError, CheckedByteCount, CheckedCount,
};
pub use ::reovim_foundation::carrier::CarrierSafetyDescriptor;
use reovim_foundation::contract::ContractId;
use reovim_foundation::identity::InstanceId;

use crate::clock::{ClockDescriptor, ClockError, MonotonicInstant};
use crate::lifecycle::{LifecycleIdentity, ReadinessOutcome};
use crate::operation::ContractSelection;

mod admission;
mod canonical;
mod counts;
mod cursor;
mod descriptor;
mod identity;
mod limits;
mod proposal;
mod record;
mod remediation;
mod sequence;
mod snapshot;
mod snapshot_validation;

/// Exact maximum for one canonical `kernel.startup` schema-v1 record.
pub const STARTUP_CANONICAL_MAX_BYTES: u64 = 495;

/// Exact resident allocation ceiling for the Event Ring.
pub const EVENT_RING_MAX_BYTES: u64 = 1_048_576;

/// Contract, family, catalog, Stream, and owner identities for startup Events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartupEventIdentities {
    contract: ContractId,
    family: ContractId,
    remediation_catalog: ContractId,
    canonical_reference: ContractId,
    stream: LifecycleIdentity,
    lifecycle_owner: LifecycleIdentity,
}

/// Hard bounds for startup proposal, admission, canonical, and snapshot work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupEventLimits {
    component_counts: BoundDescriptor,
    synchronous_work: BoundDescriptor,
    temporary_bytes: BoundDescriptor,
    canonical_bytes: BoundDescriptor,
    ring_bytes: BoundDescriptor,
    snapshot_records: CheckedCount,
    snapshot_bytes: CheckedByteCount,
}

/// Immutable startup Event contract and exact clock binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupEventDescriptor {
    contract: ContractSelection,
    identities: StartupEventIdentities,
    clock: ClockDescriptor,
    limits: StartupEventLimits,
    canonical_reference_safety: CarrierSafetyDescriptor,
}

/// Fixed bootstrap-health state retained outside Event admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapHealth {
    /// No startup diagnostic remains pending.
    Clear,
    /// A fixed startup diagnostic was imported once readiness allowed it.
    Imported,
    /// A startup diagnostic remains pending for later bounded import.
    Pending,
}

/// Severity derived from one startup readiness outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Startup completed without diagnostic degradation.
    Info,
    /// Startup completed with degraded diagnostics.
    Warning,
    /// Startup activation failed.
    Error,
}

/// Closed remediation codes in the startup catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemediationCode {
    /// Restore or replace the degraded diagnostic path.
    DiagnosticPathDegraded,
    /// Inspect and resolve the failed activation.
    ActivationFailed,
}

/// Versioned operator remediation selected by the startup contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemediationRef {
    catalog: ContractId,
    catalog_version: u64,
    code: RemediationCode,
}

/// Whether a startup Event requires an operator action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actionability {
    /// No operator action is prescribed.
    None,
    /// The closed catalog identifies one safe operator action.
    Operator(RemediationRef),
}

/// Checked active, degraded, and unavailable component counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartupComponentCounts {
    active: CheckedCount,
    degraded: CheckedCount,
    unavailable: CheckedCount,
    total: CheckedCount,
}

/// Unadmitted startup fields with contract-derived diagnostic classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartupProposal {
    lifecycle_owner: LifecycleIdentity,
    readiness: ReadinessOutcome,
    counts: StartupComponentCounts,
    bootstrap_health: BootstrapHealth,
    severity: Severity,
    actionability: Actionability,
}

/// Nonzero total sequence within one Event Stream incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSequence(u64);

/// Editor-stamped startup timestamp from the descriptor's current clock.
#[derive(Clone, Copy)]
pub struct EventTimestamp {
    instant: MonotonicInstant,
    elapsed_nanoseconds: u64,
}

/// Dispatch-scoped producer and correlation provenance for admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventAdmissionScope {
    stream: LifecycleIdentity,
    lifecycle_owner: LifecycleIdentity,
    producer: InstanceId,
    correlation: InstanceId,
}

/// Fixed-size saturating counter with visible saturation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaturatingCounter {
    value: u64,
    saturated: bool,
}

/// Stateless reader cursor for one Stream incarnation and next sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventCursor {
    stream_incarnation: u128,
    position: CursorPosition,
}

/// Explicit next-sequence or exhausted cursor position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorPosition {
    /// The reader requests this next sequence.
    Next(EventSequence),
    /// Sequence [`u64::MAX`] was admitted and no next value exists.
    Exhausted,
}

/// Validated retained sequence coverage, including an empty Stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedSequenceRange {
    earliest: Option<EventSequence>,
    latest: Option<EventSequence>,
}

/// Caller-owned record and canonical-byte limits for one snapshot attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotRequest {
    charge_owner: LifecycleIdentity,
    maximum_records: CheckedCount,
    maximum_bytes: CheckedByteCount,
}

/// Copied record/canonical-byte counts and retained eviction totals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotStatistics {
    copied_records: CheckedCount,
    copied_bytes: CheckedByteCount,
    evicted_records: SaturatingCounter,
    evicted_bytes: SaturatingCounter,
}

/// Whether one snapshot copied every record in its retained range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotCoverage {
    /// Every retained record in scope was copied.
    Complete,
    /// Capacity ended before this first complete record could be copied.
    Partial {
        /// First retained sequence not present in this snapshot.
        first_uncopied: EventSequence,
    },
}

/// Bounded snapshot coverage and loss metadata before the record join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotMetadata {
    charge_owner: LifecycleIdentity,
    stream_incarnation: u128,
    retained: RetainedSequenceRange,
    statistics: SnapshotStatistics,
    next_cursor: EventCursor,
    coverage: SnapshotCoverage,
}

/// Borrowed canonical bytes whose bounds and line shape were validated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalEventBytes<'a> {
    bytes: &'a [u8],
    size: CheckedByteCount,
}

/// Borrowed family-neutral identity and canonical bytes for Event fan-out.
///
/// This value proves structural coherence. Possession alone does not prove
/// that the record crossed the atomic admission cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalEventRecordRef<'a> {
    stream: LifecycleIdentity,
    family: ContractId,
    schema_version: u64,
    sequence: EventSequence,
    bytes: &'a [u8],
    size: CheckedByteCount,
}

/// Complete immutable startup record prepared for one admission commit.
#[derive(Clone, Copy)]
pub struct StartupEventRecord<'a> {
    scope: EventAdmissionScope,
    family: ContractId,
    schema_version: u64,
    sequence: EventSequence,
    timestamp: EventTimestamp,
    proposal: StartupProposal,
    canonical: CanonicalEventBytes<'a>,
}

/// Caller-owned view of complete copied startup records and their metadata.
#[derive(Clone, Copy)]
pub struct StartupSnapshot<'a> {
    metadata: SnapshotMetadata,
    records: &'a [StartupEventRecord<'a>],
}

/// Typed cause for rejecting an unadmitted proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventRejectCause {
    /// The Event Stream has not published readiness.
    StreamNotReady,
    /// The caller does not hold the expected lifecycle authority.
    InvalidAuthority,
    /// Proposal fields violate their structural contract.
    InvalidStructure,
    /// One proposal or synchronous observation exceeded a declared bound.
    BoundExceeded,
    /// Severity or Actionability differs from the readiness result.
    InvalidActionability,
    /// The injected clock could not provide a valid current instant.
    Clock(ClockError),
}

/// Typed cause for dropping an otherwise valid admission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventDropCause {
    /// The declared rate policy rejected the attempt.
    RateLimited,
    /// Non-waiting synchronization authority was unavailable.
    SynchronizationPressure,
    /// Required bounded storage could not be allocated.
    AllocationFailure,
    /// Exact charge preparation failed before mutation.
    ChargePreparationFailure,
    /// Complete canonical rendering failed before mutation.
    RenderingFailure,
}

/// Typed cause that fences and closes startup admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventClosedCause {
    /// Event Stream shutdown has begun.
    StreamClosing,
    /// The Event Stream is closed.
    StreamClosed,
    /// A newer Event Stream generation replaced the observed generation.
    StreamReplaced,
    /// The bound clock is closing.
    ClockClosing,
    /// The bound clock is closed.
    ClockClosed,
    /// The bound clock generation was replaced.
    ClockReplaced,
    /// The nonzero sequence domain is exhausted.
    SequenceExhausted,
}

/// Complete producer-visible result of one startup admission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventAdmissionOutcome {
    /// A complete record committed under this sequence.
    Accepted(EventSequence),
    /// Validation or required observation rejected the proposal.
    Rejected(EventRejectCause),
    /// Pressure or fallible preparation dropped the proposal.
    Dropped(EventDropCause),
    /// The Stream or one bound lifecycle dependency is closed.
    Closed(EventClosedCause),
}

/// Typed construction and validation failures for Event contract values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    /// A Foundation bound rejected a value before work.
    Bound(BoundError),
    /// A bound charge owner differs from the Stream instance.
    ChargeOwnerMismatch,
    /// Contract selection and startup identity name different families.
    DescriptorContractMismatch,
    /// The borrowed canonical-reference carrier declaration is incoherent.
    IncoherentCarrier,
    /// The selected startup Event contract family differs.
    UnsupportedContract,
    /// The requested startup Event contract version is incompatible.
    IncompatibleContract,
    /// Proposal lifecycle-owner stamp differs from the descriptor.
    ProposalOwnerMismatch,
    /// Admission scope does not match the bound Stream or lifecycle owner.
    AdmissionScopeMismatch,
    /// Readiness and component-state counts contradict one another.
    InvalidProposalStructure,
    /// Stored severity or Actionability differs from readiness.
    InvalidProposalClassification,
    /// Zero cannot identify an admitted Event sequence.
    UnassignedSequence,
    /// The sequence domain cannot advance without wrapping.
    SequenceExhausted,
    /// A saturated counter carried a value below [`u64::MAX`].
    InvalidCounterSaturation,
    /// Earliest retained sequence follows latest retained sequence.
    InvalidRetainedRange,
    /// A snapshot request or complete record exceeds granted capacity.
    SnapshotBoundExceeded,
    /// Snapshot coverage, range, counts, canonical bytes, or cursor disagree.
    InvalidSnapshotMetadata,
    /// A prepared record disagrees with its descriptor or canonical bytes.
    InvalidStartupRecord,
    /// Canonical bytes are empty.
    EmptyCanonicalBytes,
    /// Canonical bytes contain a non-ASCII octet.
    NonAsciiCanonicalBytes,
    /// Canonical bytes do not contain exactly one terminal line feed.
    InvalidCanonicalFraming,
    /// Canonical byte length cannot fit the portable size carrier.
    CanonicalLengthOverflow,
    /// A timestamp differs from the descriptor's current clock generation.
    Clock(ClockError),
}

/// Semantic port implemented by one scoped startup admission capability.
///
/// Assembly and Kernel Runtime grant the implementation only after validating
/// dispatch authority. [`StartupEventAdmission::admit`] revalidates that scope
/// before proposal traversal, clock observation, allocation, or rendering.
pub trait StartupEventAdmission {
    /// Returns the immutable Event and clock binding for this authority.
    fn descriptor(&self) -> &StartupEventDescriptor;

    /// Returns the dispatch scope bound to this admission handle.
    fn scope(&self) -> EventAdmissionScope;

    /// Attempts one bounded, non-waiting, no-callback startup admission.
    fn admit(&self, proposal: StartupProposal) -> EventAdmissionOutcome;
}
