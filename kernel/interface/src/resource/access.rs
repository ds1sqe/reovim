//! Resource Access descriptors, readiness, and terminal values.

use core::fmt;

use crate::bound::CheckedCount;
use crate::cancellation::ExecutionOutcome;
use crate::lifecycle::{Generation, LifecycleIdentity, RollbackOutcome};
use crate::operation::{ContractSelection, OperationDescriptor};

use super::bounds::ResourceAccessBounds;
use super::capability::{ChangeDiscovery, ResourceCapabilities};
use super::{ResourceAccessStamp, ResourceSnapshotStamp, ResourceValueSchemas};

/// Immutable declaration for one Resource Adapter contract and owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceAccessDescriptor {
    contract: ContractSelection,
    adapter: LifecycleIdentity,
    owner: LifecycleIdentity,
    schemas: ResourceValueSchemas,
    capabilities: ResourceCapabilities,
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
        bounds: &ResourceAccessBounds,
    ) -> Self {
        Self {
            contract,
            adapter,
            owner,
            schemas,
            capabilities,
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

    /// Returns all transfer, state, and retention bounds.
    #[must_use]
    pub const fn bounds(&self) -> &ResourceAccessBounds {
        &self.bounds
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
        OperationDescriptor::external_observation_deferred()
    }

    /// Returns the bounded Snapshot observation operation classification.
    #[must_use]
    pub const fn observation_operation(&self) -> OperationDescriptor {
        OperationDescriptor::external_observation_deferred()
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

/// Terminal result of one bounded Resource observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceObservationOutcome {
    /// One exact immutable Snapshot was observed.
    Observed(ResourceSnapshotStamp),
    /// No Snapshot was produced for the stated semantic reason.
    NotObserved(ResourceObservationFailure),
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
