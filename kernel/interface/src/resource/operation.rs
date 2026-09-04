//! Resource-scoped operation identities and owned requests.

use core::fmt;

use crate::cancellation::CancellationCause;
use crate::lifecycle::LifecycleIdentity;

use super::access::ResourceAccessDescriptor;
use super::carrier::ResourceRegionCarrier;
use super::{ResourceAccessStamp, ResourceIdentity, ResourceSnapshotStamp};

/// Identity of one Resource operation request under an adapter generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceRequestIdentity(u128);

impl ResourceRequestIdentity {
    /// Constructs an authority-supplied request identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque request identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of cancellation authority for one Resource request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCancellationIdentity(u128);

impl ResourceCancellationIdentity {
    /// Constructs an authority-supplied cancellation identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque cancellation identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Root causal identity shared by related Resource operations and Events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCorrelationIdentity(u128);

impl ResourceCorrelationIdentity {
    /// Constructs an authority-supplied root correlation identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque root correlation identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of a deadline governed and observed outside the Resource port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceDeadlineIdentity(u128);

impl ResourceDeadlineIdentity {
    /// Constructs an authority-supplied deadline identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque deadline identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Whether one Resource operation has a separately governed deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceDeadline {
    /// No time-based cancellation deadline applies.
    None,
    /// The named outside authority observes and enforces the deadline.
    Governed(ResourceDeadlineIdentity),
}

/// Closed operation kinds admitted by the initial Resource Access port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceOperationKind {
    /// Establish one provisional Access and publish Ready atomically.
    Open,
    /// Observe bounded regions from one exact Access generation.
    Observation,
    /// Close admission and resolve or expose retained ownership.
    Close,
}

/// Exact adapter, kind, request, cancellation, deadline, and causal stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceOperationStamp {
    adapter: LifecycleIdentity,
    kind: ResourceOperationKind,
    request: ResourceRequestIdentity,
    cancellation: ResourceCancellationIdentity,
    correlation: ResourceCorrelationIdentity,
    deadline: ResourceDeadline,
}

impl ResourceOperationStamp {
    /// Constructs one caller-owned operation stamp.
    #[must_use]
    pub const fn new(
        adapter: LifecycleIdentity,
        kind: ResourceOperationKind,
        request: ResourceRequestIdentity,
        cancellation: ResourceCancellationIdentity,
        correlation: ResourceCorrelationIdentity,
        deadline: ResourceDeadline,
    ) -> Self {
        Self { adapter, kind, request, cancellation, correlation, deadline }
    }

    /// Returns the exact adapter generation receiving this operation.
    #[must_use]
    pub const fn adapter(self) -> LifecycleIdentity {
        self.adapter
    }

    /// Returns the operation kind.
    #[must_use]
    pub const fn kind(self) -> ResourceOperationKind {
        self.kind
    }

    /// Returns the request identity.
    #[must_use]
    pub const fn request(self) -> ResourceRequestIdentity {
        self.request
    }

    /// Returns the cancellation-authority identity.
    #[must_use]
    pub const fn cancellation(self) -> ResourceCancellationIdentity {
        self.cancellation
    }

    /// Returns the root causal correlation identity.
    #[must_use]
    pub const fn correlation(self) -> ResourceCorrelationIdentity {
        self.correlation
    }

    /// Returns the separately governed deadline identity, when present.
    #[must_use]
    pub const fn deadline(self) -> ResourceDeadline {
        self.deadline
    }
}

/// Failure to bind a Resource request to a descriptor and operation kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceRequestError {
    /// The request targets another adapter incarnation or generation.
    AdapterGenerationMismatch,
    /// The operation stamp carries another request kind.
    OperationKindMismatch {
        /// Operation kind required by the request type.
        expected: ResourceOperationKind,
        /// Operation kind supplied by the stamp.
        actual: ResourceOperationKind,
    },
    /// Close operations cannot be cancelled through the request port.
    CloseIsNotCancellable,
}

impl fmt::Display for ResourceRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AdapterGenerationMismatch => {
                "Resource request targets another adapter generation"
            }
            Self::OperationKindMismatch { .. } => {
                "Resource request carries another operation kind"
            }
            Self::CloseIsNotCancellable => {
                "Resource close is not cancellable through this port"
            }
        })
    }
}

impl core::error::Error for ResourceRequestError {}

/// Caller-owned request to provisionally open one Resource identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceOpenRequest {
    operation: ResourceOperationStamp,
    resource: ResourceIdentity,
}

impl ResourceOpenRequest {
    /// Binds an open request to the exact descriptor generation.
    ///
    /// # Errors
    ///
    /// Returns a typed adapter-generation or operation-kind mismatch.
    pub const fn new(
        descriptor: &ResourceAccessDescriptor,
        operation: ResourceOperationStamp,
        resource: ResourceIdentity,
    ) -> Result<Self, ResourceRequestError> {
        match validate_operation(
            descriptor,
            operation,
            ResourceOperationKind::Open,
        ) {
            Ok(()) => Ok(Self { operation, resource }),
            Err(error) => Err(error),
        }
    }

    /// Returns the exact operation stamp.
    #[must_use]
    pub const fn operation(self) -> ResourceOperationStamp {
        self.operation
    }

    /// Returns the opaque Resource identity requested for open.
    #[must_use]
    pub const fn resource(self) -> ResourceIdentity {
        self.resource
    }
}

/// State selection for one bounded Resource observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceObservationTarget {
    /// Observe current state through one exact Access generation.
    Current(ResourceAccessStamp),
    /// Observe one exact current or retained immutable Snapshot.
    Snapshot(ResourceSnapshotStamp),
}

impl ResourceObservationTarget {
    /// Returns the Access generation governing this target.
    #[must_use]
    pub const fn access(self) -> ResourceAccessStamp {
        match self {
            Self::Current(access) => access,
            Self::Snapshot(snapshot) => snapshot.access(),
        }
    }
}

/// Move-only request for one bounded Resource-native observation.
pub struct ResourceObservationRequest {
    operation: ResourceOperationStamp,
    target: ResourceObservationTarget,
    regions: ResourceRegionCarrier,
}

impl fmt::Debug for ResourceObservationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservationRequest")
            .field("operation", &self.operation)
            .field("target", &self.target)
            .field("regions", &self.regions)
            .finish()
    }
}

impl ResourceObservationRequest {
    /// Binds an owned region carrier to an observation operation.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch together with unchanged `regions` ownership.
    pub const fn new(
        descriptor: &ResourceAccessDescriptor,
        operation: ResourceOperationStamp,
        target: ResourceObservationTarget,
        regions: ResourceRegionCarrier,
    ) -> Result<Self, (ResourceRequestError, ResourceRegionCarrier)> {
        match validate_operation(
            descriptor,
            operation,
            ResourceOperationKind::Observation,
        ) {
            Ok(()) => Ok(Self { operation, target, regions }),
            Err(error) => Err((error, regions)),
        }
    }

    /// Returns the exact operation stamp.
    #[must_use]
    pub const fn operation(&self) -> ResourceOperationStamp {
        self.operation
    }

    /// Returns the exact current or historical observation target.
    #[must_use]
    pub const fn target(&self) -> ResourceObservationTarget {
        self.target
    }

    /// Borrows the qualified native region carrier.
    #[must_use]
    pub const fn regions(&self) -> &ResourceRegionCarrier {
        &self.regions
    }

    /// Releases the request and returns its native region carrier.
    #[must_use]
    pub fn into_regions(self) -> ResourceRegionCarrier {
        self.regions
    }
}

/// Copyable request for cooperative cancellation of admitted Resource work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCancellationRequest {
    operation: ResourceOperationStamp,
    cause: CancellationCause,
}

impl ResourceCancellationRequest {
    /// Constructs cancellation for an open or observation operation.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceRequestError::CloseIsNotCancellable`] for close.
    pub const fn new(
        operation: ResourceOperationStamp,
        cause: CancellationCause,
    ) -> Result<Self, ResourceRequestError> {
        if operation_kind_code(operation.kind()) == 2 {
            return Err(ResourceRequestError::CloseIsNotCancellable);
        }
        Ok(Self { operation, cause })
    }

    /// Returns the exact target operation.
    #[must_use]
    pub const fn operation(self) -> ResourceOperationStamp {
        self.operation
    }

    /// Returns why cooperative cancellation was requested.
    #[must_use]
    pub const fn cause(self) -> CancellationCause {
        self.cause
    }
}

/// Caller-owned request to close one exact adapter generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCloseRequest {
    operation: ResourceOperationStamp,
}

impl ResourceCloseRequest {
    /// Binds a close request to the exact descriptor generation.
    ///
    /// # Errors
    ///
    /// Returns a typed adapter-generation or operation-kind mismatch.
    pub const fn new(
        descriptor: &ResourceAccessDescriptor,
        operation: ResourceOperationStamp,
    ) -> Result<Self, ResourceRequestError> {
        match validate_operation(
            descriptor,
            operation,
            ResourceOperationKind::Close,
        ) {
            Ok(()) => Ok(Self { operation }),
            Err(error) => Err(error),
        }
    }

    /// Returns the exact close operation stamp.
    #[must_use]
    pub const fn operation(self) -> ResourceOperationStamp {
        self.operation
    }
}

const fn validate_operation(
    descriptor: &ResourceAccessDescriptor,
    operation: ResourceOperationStamp,
    expected: ResourceOperationKind,
) -> Result<(), ResourceRequestError> {
    if !descriptor.adapter().same_generation(operation.adapter()) {
        return Err(ResourceRequestError::AdapterGenerationMismatch);
    }
    if operation_kind_code(operation.kind()) != operation_kind_code(expected) {
        return Err(ResourceRequestError::OperationKindMismatch {
            expected,
            actual: operation.kind(),
        });
    }
    Ok(())
}

const fn operation_kind_code(kind: ResourceOperationKind) -> u8 {
    match kind {
        ResourceOperationKind::Open => 0,
        ResourceOperationKind::Observation => 1,
        ResourceOperationKind::Close => 2,
    }
}
