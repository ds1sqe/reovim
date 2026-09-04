//! Resource Access descriptors, readiness, and terminal values.

use core::fmt;

use crate::bound::CheckedCount;
use crate::cancellation::ExecutionOutcome;
use crate::lifecycle::{Generation, LifecycleIdentity, RollbackOutcome};
use crate::operation::{ContractSelection, OperationDescriptor};

use super::bounds::ResourceAccessBounds;
use super::capability::{ChangeDiscovery, ResourceCapabilities};
use super::carrier::{
    ResourceCarrierContracts, ResourceCarrierError, ResourceCarrierRejection,
    ResourceCarrierRole, ResourceCarrierStamp, ResourcePayloadCarrier,
    ResourcePayloadPreparation, ResourceRegionCarrier,
    ResourceRegionPreparation,
};
use super::operation::ResourceObservationTarget;
use super::{ResourceAccessStamp, ResourceSnapshotStamp, ResourceValueSchemas};

/// Immutable declaration for one Resource Adapter contract and owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceAccessDescriptor {
    contract: ContractSelection,
    adapter: LifecycleIdentity,
    owner: LifecycleIdentity,
    schemas: ResourceValueSchemas,
    capabilities: ResourceCapabilities,
    carriers: ResourceCarrierContracts,
    bounds: ResourceAccessBounds,
}

impl ResourceAccessDescriptor {
    /// Constructs a descriptor from already validated independent parts.
    #[must_use]
    pub fn new(
        contract: ContractSelection,
        adapter: LifecycleIdentity,
        owner: LifecycleIdentity,
        schemas: ResourceValueSchemas,
        capabilities: ResourceCapabilities,
        carriers: &ResourceCarrierContracts,
        bounds: &ResourceAccessBounds,
    ) -> Self {
        Self {
            contract,
            adapter,
            owner,
            schemas,
            capabilities,
            carriers: carriers.clone(),
            bounds: bounds.clone(),
        }
    }

    /// Returns the selected Resource Adapter contract.
    #[must_use]
    pub const fn contract(&self) -> ContractSelection {
        self.contract
    }

    /// Returns the adapter instance, incarnation, and generation.
    #[must_use]
    pub const fn adapter(&self) -> LifecycleIdentity {
        self.adapter
    }

    /// Returns the lifecycle owner responsible for the adapter.
    #[must_use]
    pub const fn owner(&self) -> LifecycleIdentity {
        self.owner
    }

    /// Returns the Resource-native value schemas.
    #[must_use]
    pub const fn schemas(&self) -> ResourceValueSchemas {
        self.schemas
    }

    /// Returns the independently selected capability dimensions.
    #[must_use]
    pub const fn capabilities(&self) -> ResourceCapabilities {
        self.capabilities
    }

    /// Returns the independently qualified region and payload carriers.
    #[must_use]
    pub const fn carriers(&self) -> &ResourceCarrierContracts {
        &self.carriers
    }

    /// Returns all transfer, state, and retention bounds.
    #[must_use]
    pub const fn bounds(&self) -> &ResourceAccessBounds {
        &self.bounds
    }

    /// Validates a region carrier before the caller allocates its bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed identity, schema, arithmetic, or hard-bound failure.
    pub const fn prepare_region_carrier(
        &self,
        stamp: ResourceCarrierStamp,
        regions: u64,
        bytes: u64,
    ) -> Result<ResourceRegionPreparation, ResourceCarrierError> {
        super::carrier::prepare_regions(
            self.carriers.regions(),
            self.bounds.transfer(),
            stamp,
            regions,
            bytes,
        )
    }

    /// Binds an exact region allocation to its prevalidated preparation.
    ///
    /// # Errors
    ///
    /// Returns the unchanged allocation with a typed failure if descriptor
    /// terms changed or the allocated length differs.
    pub fn bind_region_carrier(
        &self,
        preparation: ResourceRegionPreparation,
        bytes: ::alloc::boxed::Box<[u8]>,
    ) -> Result<ResourceRegionCarrier, ResourceCarrierRejection> {
        super::carrier::bind_regions(
            self.carriers.regions(),
            self.bounds.transfer(),
            preparation,
            bytes,
        )
    }

    /// Validates a payload carrier before the caller allocates its bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed identity, schema, arithmetic, or hard-bound failure.
    pub const fn prepare_payload_carrier(
        &self,
        stamp: ResourceCarrierStamp,
        regions: u64,
        bytes: u64,
    ) -> Result<ResourcePayloadPreparation, ResourceCarrierError> {
        super::carrier::prepare_payload(
            self.carriers.payload(),
            self.bounds.transfer(),
            stamp,
            regions,
            bytes,
        )
    }

    /// Binds an exact payload allocation to its validated preparation.
    ///
    /// # Errors
    ///
    /// Returns the unchanged allocation with a typed failure if descriptor
    /// terms changed or the allocated length differs.
    pub fn bind_payload_carrier(
        &self,
        preparation: ResourcePayloadPreparation,
        bytes: ::alloc::boxed::Box<[u8]>,
    ) -> Result<ResourcePayloadCarrier, ResourceCarrierRejection> {
        super::carrier::bind_payload(
            self.carriers.payload(),
            self.bounds.transfer(),
            preparation,
            bytes,
        )
    }

    /// Returns the side-effect-free descriptor inspection operation.
    #[must_use]
    pub const fn inspection_operation(&self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Returns the side-effect-free applicability operation.
    #[must_use]
    pub const fn applicability_operation(&self) -> OperationDescriptor {
        OperationDescriptor::external_observation_inline()
    }

    /// Returns the provisional open operation classification.
    #[must_use]
    pub const fn open_operation(&self) -> OperationDescriptor {
        OperationDescriptor::external_observation_deferred_admission()
    }

    /// Returns the bounded Snapshot observation operation classification.
    #[must_use]
    pub const fn observation_operation(&self) -> OperationDescriptor {
        OperationDescriptor::external_observation_deferred_admission()
    }

    /// Returns the non-waiting local completion-retrieval classification.
    #[must_use]
    pub const fn completion_delivery_operation(&self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Returns the cooperative cancellation-request classification.
    #[must_use]
    pub const fn cancellation_operation(&self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Returns the selected change-delivery operation classification.
    #[must_use]
    pub const fn change_operation(&self) -> Option<OperationDescriptor> {
        match self.capabilities.change_discovery() {
            ChangeDiscovery::None => None,
            ChangeDiscovery::Invalidation
            | ChangeDiscovery::OrderedResumableFeed => {
                Some(OperationDescriptor::external_observation_stream())
            }
        }
    }

    /// Returns the bounded idempotent close operation classification.
    #[must_use]
    pub const fn close_operation(&self) -> OperationDescriptor {
        OperationDescriptor::local_deferred()
    }

    /// Validates one requested contract identity and version.
    ///
    /// # Errors
    ///
    /// Returns a typed unsupported-family or incompatible-version result.
    pub const fn negotiate(
        &self,
        requested_contract: u128,
        requested_version: u64,
    ) -> Result<(), ResourceNegotiationError> {
        if requested_contract != self.contract.identity() {
            return Err(ResourceNegotiationError::UnsupportedContract);
        }
        if !self.contract.supports(requested_version) {
            return Err(ResourceNegotiationError::IncompatibleContract);
        }
        Ok(())
    }
}

/// Side-effect-free applicability result before revealing Resource lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceApplicabilityOutcome {
    /// The adapter can attempt the requested Resource family under authority.
    Applicable,
    /// Required authority was denied before lookup or observation.
    Denied,
    /// The requested Resource or adapter family is unsupported.
    Unsupported,
    /// The family is known but requested contract terms are incompatible.
    Incompatible,
}

/// Typed Resource Adapter contract negotiation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceNegotiationError {
    /// Another Resource Adapter contract family was requested.
    UnsupportedContract,
    /// The requested version lies outside the compatible range.
    IncompatibleContract,
}

impl fmt::Display for ResourceNegotiationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedContract => {
                "Resource Adapter contract is unsupported"
            }
            Self::IncompatibleContract => {
                "Resource Adapter contract version is incompatible"
            }
        })
    }
}

impl core::error::Error for ResourceNegotiationError {}

/// Independently evaluated freshness of one immutable Snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceFreshness {
    /// The Snapshot satisfies its declared freshness predicate.
    Fresh,
    /// A previously valid Snapshot no longer satisfies freshness.
    Stale,
    /// Freshness can be neither proved nor disproved.
    Indeterminate,
}

/// Initial synchronization checkpoint paired with a Ready Snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceChangeCheckpoint {
    /// No change-discovery capability was selected.
    NotRequired,
    /// Invalidation delivery begins after this adapter generation.
    Invalidation(Generation),
    /// An ordered feed begins after the exact schema-qualified cursor.
    Feed {
        /// Subscription incarnation fencing future deliveries.
        subscription: LifecycleIdentity,
        /// Adapter-issued opaque cursor equality token.
        cursor: u128,
    },
}

/// Coherent initial Access, Snapshot, and change checkpoint publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceReadyState {
    snapshot: ResourceSnapshotStamp,
    change_checkpoint: ResourceChangeCheckpoint,
}

impl ResourceReadyState {
    /// Validates and constructs the only public Ready value.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a mismatched Revision schema or
    /// change-discovery checkpoint.
    pub const fn new(
        descriptor: &ResourceAccessDescriptor,
        snapshot: ResourceSnapshotStamp,
        change_checkpoint: ResourceChangeCheckpoint,
    ) -> Result<Self, ResourceReadyError> {
        if !same_contract(
            descriptor.schemas().revision(),
            snapshot.revision().schema(),
        ) {
            return Err(ResourceReadyError::RevisionSchemaMismatch);
        }
        if !checkpoint_matches(
            descriptor.capabilities().change_discovery(),
            change_checkpoint,
        ) {
            return Err(ResourceReadyError::ChangeCheckpointMismatch);
        }
        Ok(Self { snapshot, change_checkpoint })
    }

    /// Returns the promoted Resource Access generation.
    #[must_use]
    pub const fn access(self) -> ResourceAccessStamp {
        self.snapshot.access()
    }

    /// Returns the validated immutable initial Snapshot.
    #[must_use]
    pub const fn snapshot(self) -> ResourceSnapshotStamp {
        self.snapshot
    }

    /// Returns the checkpoint closing the initial change race.
    #[must_use]
    pub const fn change_checkpoint(self) -> ResourceChangeCheckpoint {
        self.change_checkpoint
    }
}

/// Typed failure to form one coherent Ready publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceReadyError {
    /// The Snapshot Revision uses another selected schema.
    RevisionSchemaMismatch,
    /// The checkpoint differs from the selected change capability.
    ChangeCheckpointMismatch,
}

impl fmt::Display for ResourceReadyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RevisionSchemaMismatch => {
                "initial Snapshot uses another Revision schema"
            }
            Self::ChangeCheckpointMismatch => {
                "initial change checkpoint does not match capability"
            }
        })
    }
}

impl core::error::Error for ResourceReadyError {}

/// Cause of a provisional open that publishes no Resource Access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOpenFailure {
    /// Revealing lookup authority was denied.
    Denied,
    /// The requested Resource or adapter contract is unsupported.
    Unsupported,
    /// The selected contract or descriptor terms are incompatible.
    Incompatible,
    /// Descriptor or bound validation failed before observation.
    InvalidDescriptor,
    /// The proposed initial Snapshot failed structural validation.
    InvalidSnapshot,
    /// Required source state became stale during provisional open.
    Stale,
    /// Required current or historical state is unavailable.
    Unavailable,
    /// Current source state could not be determined.
    Indeterminate,
    /// Bounded capacity rejected the open before publication.
    Pressure,
    /// Cancellation retired the provisional operation.
    Cancelled,
    /// A newer adapter generation replaced the provisional operation.
    Replaced,
    /// Adapter shutdown began before Ready publication.
    Closing,
    /// The adapter was already closed.
    Closed,
    /// Another bounded adapter failure prevented Ready publication.
    AdapterFailure,
}

/// Closed status of one provisional Resource open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOpenStatus {
    /// Every readiness predicate passed and one Access became public.
    Ready,
    /// No Access was published and provisional state was rolled back.
    NotReady,
}

/// Allocation-free terminal result of one provisional Resource open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceOpenOutcome {
    ready: Option<ResourceReadyState>,
    failure: Option<ResourceOpenFailure>,
    rollback: Option<RollbackOutcome>,
}

impl ResourceOpenOutcome {
    /// Constructs a successful coherent Ready publication.
    #[must_use]
    pub const fn ready(state: ResourceReadyState) -> Self {
        Self { ready: Some(state), failure: None, rollback: None }
    }

    /// Constructs a failed open after terminal provisional rollback.
    #[must_use]
    pub const fn not_ready(
        failure: ResourceOpenFailure,
        rollback: RollbackOutcome,
    ) -> Self {
        Self { ready: None, failure: Some(failure), rollback: Some(rollback) }
    }

    /// Returns whether this result published a Ready Access.
    #[must_use]
    pub const fn status(&self) -> ResourceOpenStatus {
        if self.ready.is_some() {
            ResourceOpenStatus::Ready
        } else {
            ResourceOpenStatus::NotReady
        }
    }

    /// Returns the coherent Ready state when publication succeeded.
    #[must_use]
    pub const fn ready_state(&self) -> Option<&ResourceReadyState> {
        self.ready.as_ref()
    }

    /// Returns the failure cause when no Access was published.
    #[must_use]
    pub const fn failure(&self) -> Option<ResourceOpenFailure> {
        self.failure
    }

    /// Returns provisional rollback disposition after a failed open.
    #[must_use]
    pub const fn rollback(&self) -> Option<RollbackOutcome> {
        self.rollback
    }
}

/// Cause of a bounded Snapshot observation that produced no Snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceObservationFailure {
    /// Address or Extent failed its Resource-family range rules.
    InvalidRange,
    /// A schema-qualified carrier is unsupported.
    UnsupportedSchema,
    /// The request names another or retired Resource Access generation.
    StaleAccess,
    /// Required current or historical state is unavailable.
    Unavailable,
    /// Requested state is known to be stale.
    Stale,
    /// Current state or freshness cannot be determined.
    Indeterminate,
    /// Bounded capacity rejected observation or delivery.
    Pressure,
    /// Cancellation retired the observation.
    Cancelled,
    /// A newer Access generation fenced installation.
    Replaced,
    /// Resource Access shutdown has begun.
    Closing,
    /// The Resource Access is closed.
    Closed,
    /// Another bounded adapter failure prevented observation.
    AdapterFailure,
}

/// Failure to bind one payload to an exact observation result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceObservedStateError {
    /// The resulting Snapshot belongs to another Access generation.
    AccessMismatch,
    /// An exact historical request produced another immutable Snapshot.
    SnapshotMismatch,
    /// The resulting Revision uses another Resource-native schema.
    RevisionSchemaMismatch,
    /// Payload covers a different native-region count than requested.
    RegionCountMismatch,
    /// Payload carrier identity or version differs from the descriptor.
    Carrier(ResourceCarrierError),
}

impl fmt::Display for ResourceObservedStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AccessMismatch => {
                "observed Snapshot belongs to another Resource Access"
            }
            Self::SnapshotMismatch => {
                "observed Snapshot differs from the requested Snapshot"
            }
            Self::RevisionSchemaMismatch => {
                "observed Snapshot uses another Revision schema"
            }
            Self::RegionCountMismatch => {
                "observed payload covers another native-region count"
            }
            Self::Carrier(_) => {
                "observed payload carrier differs from the descriptor"
            }
        })
    }
}

impl core::error::Error for ResourceObservedStateError {}

/// Exact immutable Snapshot stamp and bounded Resource-native payload.
pub struct ResourceObservedState {
    snapshot: ResourceSnapshotStamp,
    payload: ResourcePayloadCarrier,
}

impl fmt::Debug for ResourceObservedState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservedState")
            .field("snapshot", &self.snapshot)
            .field("payload", &self.payload)
            .finish()
    }
}

impl ResourceObservedState {
    /// Binds an observed payload to one requested Access and region count.
    ///
    /// # Errors
    ///
    /// Returns a typed Access, Revision, region-count, or carrier mismatch.
    pub const fn new(
        descriptor: &ResourceAccessDescriptor,
        expected_target: ResourceObservationTarget,
        expected_regions: CheckedCount,
        snapshot: ResourceSnapshotStamp,
        payload: ResourcePayloadCarrier,
    ) -> Result<Self, (ResourceObservedStateError, ResourcePayloadCarrier)>
    {
        if let Err(error) =
            validate_observation_target(expected_target, snapshot)
        {
            return Err((error, payload));
        }
        if !same_contract(
            descriptor.schemas().revision(),
            snapshot.revision().schema(),
        ) {
            return Err((
                ResourceObservedStateError::RevisionSchemaMismatch,
                payload,
            ));
        }
        if expected_regions.value() != payload.regions().value() {
            return Err((
                ResourceObservedStateError::RegionCountMismatch,
                payload,
            ));
        }
        if let Err(error) = descriptor
            .carriers()
            .payload()
            .validate_stamp(ResourceCarrierRole::Payload, payload.stamp())
        {
            return Err((ResourceObservedStateError::Carrier(error), payload));
        }
        Ok(Self { snapshot, payload })
    }

    /// Returns the exact immutable Snapshot observation stamp.
    #[must_use]
    pub const fn snapshot(&self) -> ResourceSnapshotStamp {
        self.snapshot
    }

    /// Borrows the qualified Resource-native payload.
    #[must_use]
    pub const fn payload(&self) -> &ResourcePayloadCarrier {
        &self.payload
    }

    /// Releases the observation and returns its payload carrier.
    #[must_use]
    pub fn into_payload(self) -> ResourcePayloadCarrier {
        self.payload
    }
}

const fn validate_observation_target(
    target: ResourceObservationTarget,
    snapshot: ResourceSnapshotStamp,
) -> Result<(), ResourceObservedStateError> {
    match target {
        ResourceObservationTarget::Current(access) => {
            if access.same_generation(snapshot.access()) {
                Ok(())
            } else {
                Err(ResourceObservedStateError::AccessMismatch)
            }
        }
        ResourceObservationTarget::Snapshot(expected) => {
            if expected.same_snapshot(snapshot) {
                Ok(())
            } else {
                Err(ResourceObservedStateError::SnapshotMismatch)
            }
        }
    }
}

/// Closed status of one terminal Resource observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceObservationStatus {
    /// One exact immutable Snapshot and payload were observed.
    Observed,
    /// No Snapshot was produced.
    NotObserved,
}

/// Allocation-free terminal result of one bounded Resource observation.
pub struct ResourceObservationOutcome {
    observed: Option<ResourceObservedState>,
    failure: Option<ResourceObservationFailure>,
}

impl fmt::Debug for ResourceObservationOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservationOutcome")
            .field("status", &self.status())
            .field("observed", &self.observed)
            .field("failure", &self.failure)
            .finish()
    }
}

impl ResourceObservationOutcome {
    /// Constructs a successful immutable Snapshot observation.
    #[must_use]
    pub const fn observed(state: ResourceObservedState) -> Self {
        Self { observed: Some(state), failure: None }
    }

    /// Constructs a terminal observation that produced no Snapshot.
    #[must_use]
    pub const fn not_observed(failure: ResourceObservationFailure) -> Self {
        Self { observed: None, failure: Some(failure) }
    }

    /// Returns whether one Snapshot payload was observed.
    #[must_use]
    pub const fn status(&self) -> ResourceObservationStatus {
        if self.observed.is_some() {
            ResourceObservationStatus::Observed
        } else {
            ResourceObservationStatus::NotObserved
        }
    }

    /// Borrows the observed Snapshot and payload when successful.
    #[must_use]
    pub const fn observed_state(&self) -> Option<&ResourceObservedState> {
        self.observed.as_ref()
    }

    /// Returns why no Snapshot was produced, when applicable.
    #[must_use]
    pub const fn failure(&self) -> Option<ResourceObservationFailure> {
        self.failure
    }

    /// Returns Snapshot-payload ownership when observation succeeded.
    #[must_use]
    pub fn into_observed_state(self) -> Option<ResourceObservedState> {
        self.observed
    }
}

/// Typed incoherence in an incomplete close report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCloseError {
    /// The report retained neither nonterminal execution nor an obligation.
    NoRetainedOwnership,
}

impl fmt::Display for ResourceCloseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("incomplete close retains no owned obligation")
    }
}

impl core::error::Error for ResourceCloseError {}

/// Bounded ownership retained after Resource Access failed to close fully.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceIncompleteClose {
    execution: ExecutionOutcome,
    unresolved: CheckedCount,
}

impl ResourceIncompleteClose {
    /// Constructs an incomplete result with explicit retained ownership.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceCloseError::NoRetainedOwnership`] when execution is
    /// terminal and no unresolved release or reconcile obligation remains.
    pub const fn new(
        execution: ExecutionOutcome,
        unresolved: CheckedCount,
    ) -> Result<Self, ResourceCloseError> {
        if execution.is_terminal() && unresolved.value() == 0 {
            return Err(ResourceCloseError::NoRetainedOwnership);
        }
        Ok(Self { execution, unresolved })
    }

    /// Returns the separately observed execution disposition.
    #[must_use]
    pub const fn execution(self) -> ExecutionOutcome {
        self.execution
    }

    /// Returns retained release or reconcile obligations.
    #[must_use]
    pub const fn unresolved(self) -> CheckedCount {
        self.unresolved
    }
}

/// Idempotent terminal result of one Resource Access close request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCloseOutcome {
    /// Execution and release obligations are terminal.
    Closed,
    /// Bounded shutdown ended while ownership remains explicit.
    Incomplete(ResourceIncompleteClose),
    /// The close request targets another adapter generation.
    StaleAdapter,
}

const fn checkpoint_matches(
    capability: ChangeDiscovery,
    checkpoint: ResourceChangeCheckpoint,
) -> bool {
    discovery_code(capability) == checkpoint_code(checkpoint)
}

const fn discovery_code(value: ChangeDiscovery) -> u8 {
    match value {
        ChangeDiscovery::None => 0,
        ChangeDiscovery::Invalidation => 1,
        ChangeDiscovery::OrderedResumableFeed => 2,
    }
}

const fn checkpoint_code(value: ResourceChangeCheckpoint) -> u8 {
    match value {
        ResourceChangeCheckpoint::NotRequired => 0,
        ResourceChangeCheckpoint::Invalidation(_) => 1,
        ResourceChangeCheckpoint::Feed { .. } => 2,
    }
}

const fn same_contract(
    left: ContractSelection,
    right: ContractSelection,
) -> bool {
    left.identity() == right.identity()
        && left.selected() == right.selected()
        && left.minimum() == right.minimum()
        && left.maximum() == right.maximum()
}
