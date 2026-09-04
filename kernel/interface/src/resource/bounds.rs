//! Resource Access transfer, state, and retention bounds.

use core::fmt;

use crate::bound::{BoundDescriptor, BoundUnit};

use super::capability::{
    ChangeDiscovery, HistoricalAvailability, ResourceCapabilities,
};

/// Accounting rule for immutable Snapshot payload shared by consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourcePayloadAccounting {
    /// The Access owner is charged for every retained payload byte.
    OwnedByAccess,
    /// One Snapshot identity is charged once while immutable payload is shared.
    SharedBySnapshot,
}

/// Aggregate scope used by in-flight, queue, and terminal-retention bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCompletionScope {
    /// Aggregate accounting is isolated to one Access incarnation.
    AccessIncarnation,
    /// Aggregate accounting spans every Access of one adapter instance.
    AdapterInstance,
}

/// Named Resource bound whose unit or capacity was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceBoundField {
    /// Native regions addressed by one observation.
    Regions,
    /// Encoded schema-qualified request or metadata carrier bytes.
    CarrierBytes,
    /// Resource-native payload bytes retained by one result.
    PayloadBytes,
    /// Adapter-owned state bytes.
    OwnedBytes,
    /// Admitted operations that have not reached a terminal outcome.
    InFlight,
    /// Deferred operations waiting for execution or delivery.
    Queue,
    /// Immutable Snapshots retained for current or historical access.
    Snapshots,
    /// Terminal outcomes retained for caller reconciliation.
    Terminals,
}

/// Typed Resource bound-set construction failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceBoundsError {
    /// A named bound uses a unit different from its contract.
    WrongUnit {
        /// Rejected bound field.
        field: ResourceBoundField,
        /// Unit required by the field.
        expected: BoundUnit,
        /// Unit supplied by the descriptor.
        actual: BoundUnit,
    },
    /// The descriptor cannot admit any operation.
    InFlightCapacityRequired,
    /// Deferred terminal ownership has no retention capacity.
    TerminalRetentionRequired,
    /// Historical availability has no Snapshot retention capacity.
    HistoricalRetentionRequired,
    /// Change discovery has no bounded delivery queue.
    ChangeQueueRequired,
}

impl fmt::Display for ResourceBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongUnit { .. } => "Resource bound uses the wrong unit",
            Self::InFlightCapacityRequired => {
                "Resource Access requires in-flight capacity"
            }
            Self::TerminalRetentionRequired => {
                "Resource Access requires terminal retention"
            }
            Self::HistoricalRetentionRequired => {
                "historical access requires Snapshot retention"
            }
            Self::ChangeQueueRequired => {
                "change discovery requires queue capacity"
            }
        })
    }
}

impl core::error::Error for ResourceBoundsError {}

/// Bounds for one Resource-native transfer request and result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceTransferBounds {
    regions: BoundDescriptor,
    carrier_bytes: BoundDescriptor,
    payload_bytes: BoundDescriptor,
}

impl ResourceTransferBounds {
    /// Constructs transfer bounds after validating each exact Unit.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceBoundsError::WrongUnit`] for any mismatched field.
    pub const fn new(
        regions: BoundDescriptor,
        carrier_bytes: BoundDescriptor,
        payload_bytes: BoundDescriptor,
    ) -> Result<Self, ResourceBoundsError> {
        match require_unit(
            ResourceBoundField::Regions,
            regions,
            BoundUnit::Count,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_unit(
            ResourceBoundField::CarrierBytes,
            carrier_bytes,
            BoundUnit::Bytes,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_unit(
            ResourceBoundField::PayloadBytes,
            payload_bytes,
            BoundUnit::Bytes,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self { regions, carrier_bytes, payload_bytes })
    }

    /// Returns the native-region count bound.
    #[must_use]
    pub const fn regions(&self) -> BoundDescriptor {
        self.regions
    }

    /// Returns the schema-qualified carrier byte bound.
    #[must_use]
    pub const fn carrier_bytes(&self) -> BoundDescriptor {
        self.carrier_bytes
    }

    /// Returns the Resource-native payload byte bound.
    #[must_use]
    pub const fn payload_bytes(&self) -> BoundDescriptor {
        self.payload_bytes
    }
}

/// Bounds for retained state and deferred operation ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceStateBounds {
    owned_bytes: BoundDescriptor,
    in_flight: BoundDescriptor,
    queue: BoundDescriptor,
    snapshots: BoundDescriptor,
    terminals: BoundDescriptor,
}

impl ResourceStateBounds {
    /// Constructs state bounds after validating each exact Unit.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceBoundsError::WrongUnit`] for any mismatched field.
    pub const fn new(
        owned_bytes: BoundDescriptor,
        in_flight: BoundDescriptor,
        queue: BoundDescriptor,
        snapshots: BoundDescriptor,
        terminals: BoundDescriptor,
    ) -> Result<Self, ResourceBoundsError> {
        match require_unit(
            ResourceBoundField::OwnedBytes,
            owned_bytes,
            BoundUnit::Bytes,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_count(ResourceBoundField::InFlight, in_flight) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_count(ResourceBoundField::Queue, queue) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_count(ResourceBoundField::Snapshots, snapshots) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_count(ResourceBoundField::Terminals, terminals) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self { owned_bytes, in_flight, queue, snapshots, terminals })
    }

    /// Returns the adapter-owned byte bound.
    #[must_use]
    pub const fn owned_bytes(&self) -> BoundDescriptor {
        self.owned_bytes
    }

    /// Returns the admitted nonterminal-operation bound.
    #[must_use]
    pub const fn in_flight(&self) -> BoundDescriptor {
        self.in_flight
    }

    /// Returns the deferred execution or delivery queue bound.
    #[must_use]
    pub const fn queue(&self) -> BoundDescriptor {
        self.queue
    }

    /// Returns the immutable Snapshot retention bound.
    #[must_use]
    pub const fn snapshots(&self) -> BoundDescriptor {
        self.snapshots
    }

    /// Returns the terminal-outcome retention bound.
    #[must_use]
    pub const fn terminals(&self) -> BoundDescriptor {
        self.terminals
    }
}

/// Complete hard-bound declaration for one Resource Access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceAccessBounds {
    transfer: ResourceTransferBounds,
    state: ResourceStateBounds,
    payload_accounting: ResourcePayloadAccounting,
    completion_scope: ResourceCompletionScope,
}

impl ResourceAccessBounds {
    /// Constructs a bound set coherent with selected capabilities.
    ///
    /// # Errors
    ///
    /// Returns a typed capacity failure before the descriptor is admitted.
    pub fn new(
        transfer: &ResourceTransferBounds,
        state: &ResourceStateBounds,
        payload_accounting: ResourcePayloadAccounting,
        completion_scope: ResourceCompletionScope,
        capabilities: ResourceCapabilities,
    ) -> Result<Self, ResourceBoundsError> {
        if bound_is_zero(state.in_flight()) {
            return Err(ResourceBoundsError::InFlightCapacityRequired);
        }
        if bound_is_zero(state.terminals()) {
            return Err(ResourceBoundsError::TerminalRetentionRequired);
        }
        if history_needs_retention(capabilities, state.snapshots()) {
            return Err(ResourceBoundsError::HistoricalRetentionRequired);
        }
        if discovery_needs_queue(capabilities, state.queue()) {
            return Err(ResourceBoundsError::ChangeQueueRequired);
        }
        Ok(Self {
            transfer: transfer.clone(),
            state: state.clone(),
            payload_accounting,
            completion_scope,
        })
    }

    /// Returns transfer request/result bounds.
    #[must_use]
    pub const fn transfer(&self) -> &ResourceTransferBounds {
        &self.transfer
    }

    /// Returns retained state and deferred-operation bounds.
    #[must_use]
    pub const fn state(&self) -> &ResourceStateBounds {
        &self.state
    }

    /// Returns immutable payload accounting ownership.
    #[must_use]
    pub const fn payload_accounting(&self) -> ResourcePayloadAccounting {
        self.payload_accounting
    }

    /// Returns the aggregate completion-accounting scope.
    #[must_use]
    pub const fn completion_scope(&self) -> ResourceCompletionScope {
        self.completion_scope
    }
}

const fn require_count(
    field: ResourceBoundField,
    descriptor: BoundDescriptor,
) -> Result<(), ResourceBoundsError> {
    require_unit(field, descriptor, BoundUnit::Count)
}

const fn require_unit(
    field: ResourceBoundField,
    descriptor: BoundDescriptor,
    expected: BoundUnit,
) -> Result<(), ResourceBoundsError> {
    let actual = descriptor.unit();
    if same_unit(actual, expected) {
        return Ok(());
    }
    Err(ResourceBoundsError::WrongUnit { field, expected, actual })
}

const fn same_unit(left: BoundUnit, right: BoundUnit) -> bool {
    unit_code(left) == unit_code(right)
}

const fn unit_code(unit: BoundUnit) -> u8 {
    match unit {
        BoundUnit::Count => 0,
        BoundUnit::Bytes => 1,
        BoundUnit::DurationNanoseconds => 2,
    }
}

const fn bound_is_zero(bound: BoundDescriptor) -> bool {
    bound.aggregate().value() == 0
}

const fn history_needs_retention(
    capabilities: ResourceCapabilities,
    snapshots: BoundDescriptor,
) -> bool {
    match capabilities.history() {
        HistoricalAvailability::CurrentOnly => false,
        HistoricalAvailability::BoundedRetention => bound_is_zero(snapshots),
    }
}

const fn discovery_needs_queue(
    capabilities: ResourceCapabilities,
    queue: BoundDescriptor,
) -> bool {
    match capabilities.change_discovery() {
        ChangeDiscovery::None => false,
        ChangeDiscovery::Invalidation
        | ChangeDiscovery::OrderedResumableFeed => bound_is_zero(queue),
    }
}
