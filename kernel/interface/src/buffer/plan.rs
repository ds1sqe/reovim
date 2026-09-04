//! Immutable public plans for later private Buffer transitions.

use core::fmt;

use crate::bound::{BoundError, CheckedByteCount};
use crate::lifecycle::LifecycleIdentity;
use crate::operation::{ContractSelection, OperationDescriptor};

use super::BufferStateStamp;
use super::bounds::BufferPlanBounds;
use super::carrier::{
    BufferCarrierContracts, BufferCarrierError, BufferCarrierPreparation,
    BufferCarrierRole, BufferCarrierStamp, BufferPlanCarrier,
};

/// Identity of one lowered Buffer Transaction plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferTransactionIdentity(u128);

impl BufferTransactionIdentity {
    /// Constructs an authority-supplied Transaction identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque Transaction identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of the History transition proposed by one plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferHistoryTransitionIdentity(u128);

impl BufferHistoryTransitionIdentity {
    /// Constructs an authority-supplied History-transition identity.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the opaque History-transition identity scalar.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Identity of cancellation authority for one Buffer plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCancellationIdentity(u128);

impl BufferCancellationIdentity {
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

/// Root causal identity shared by one plan and its later outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCorrelationIdentity(u128);

impl BufferCorrelationIdentity {
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

/// Identity of a deadline governed outside Buffer State Lane authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferDeadlineIdentity(u128);

impl BufferDeadlineIdentity {
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

/// Whether one Buffer plan has a separately governed deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferDeadline {
    /// No time-based cancellation deadline applies.
    None,
    /// The named outside authority observes and enforces the deadline.
    Governed(BufferDeadlineIdentity),
}

/// Exact owner, Transaction, History, cancellation, deadline, and causal stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferPlanStamp {
    owner: LifecycleIdentity,
    transaction: BufferTransactionIdentity,
    history_transition: BufferHistoryTransitionIdentity,
    cancellation: BufferCancellationIdentity,
    correlation: BufferCorrelationIdentity,
    deadline: BufferDeadline,
}

impl BufferPlanStamp {
    /// Constructs one caller-owned plan stamp.
    #[must_use]
    pub const fn new(
        owner: LifecycleIdentity,
        transaction: BufferTransactionIdentity,
        history_transition: BufferHistoryTransitionIdentity,
        cancellation: BufferCancellationIdentity,
        correlation: BufferCorrelationIdentity,
        deadline: BufferDeadline,
    ) -> Self {
        Self {
            owner,
            transaction,
            history_transition,
            cancellation,
            correlation,
            deadline,
        }
    }

    /// Returns the exact descriptor-owner generation receiving the plan.
    #[must_use]
    pub const fn owner(self) -> LifecycleIdentity {
        self.owner
    }

    /// Returns the Transaction identity.
    #[must_use]
    pub const fn transaction(self) -> BufferTransactionIdentity {
        self.transaction
    }

    /// Returns the proposed History-transition identity.
    #[must_use]
    pub const fn history_transition(self) -> BufferHistoryTransitionIdentity {
        self.history_transition
    }

    /// Returns the cancellation-authority identity.
    #[must_use]
    pub const fn cancellation(self) -> BufferCancellationIdentity {
        self.cancellation
    }

    /// Returns the root causal correlation identity.
    #[must_use]
    pub const fn correlation(self) -> BufferCorrelationIdentity {
        self.correlation
    }

    /// Returns the separately governed deadline, when present.
    #[must_use]
    pub const fn deadline(self) -> BufferDeadline {
        self.deadline
    }

    /// Reports equality of every plan identity and authority stamp.
    #[must_use]
    pub const fn same_plan(self, other: Self) -> bool {
        self.owner.same_generation(other.owner)
            && self.transaction.value() == other.transaction.value()
            && self.history_transition.value()
                == other.history_transition.value()
            && self.cancellation.value() == other.cancellation.value()
            && self.correlation.value() == other.correlation.value()
            && same_deadline(self.deadline, other.deadline)
    }
}

const fn same_deadline(left: BufferDeadline, right: BufferDeadline) -> bool {
    match (left, right) {
        (BufferDeadline::None, BufferDeadline::None) => true,
        (BufferDeadline::Governed(left), BufferDeadline::Governed(right)) => {
            left.value() == right.value()
        }
        (BufferDeadline::None, BufferDeadline::Governed(_))
        | (BufferDeadline::Governed(_), BufferDeadline::None) => false,
    }
}

/// Descriptor for structural construction of Buffer Transaction plans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferPlanDescriptor {
    contract: ContractSelection,
    owner: LifecycleIdentity,
    carriers: BufferCarrierContracts,
    bounds: BufferPlanBounds,
}

impl BufferPlanDescriptor {
    /// Constructs a descriptor from independently validated parts.
    #[must_use]
    pub fn new(
        contract: ContractSelection,
        owner: LifecycleIdentity,
        carriers: &BufferCarrierContracts,
        bounds: &BufferPlanBounds,
    ) -> Self {
        Self {
            contract,
            owner,
            carriers: carriers.clone(),
            bounds: bounds.clone(),
        }
    }

    /// Returns the selected Buffer-plan contract.
    #[must_use]
    pub const fn contract(&self) -> ContractSelection {
        self.contract
    }

    /// Returns the exact lifecycle owner and generation.
    #[must_use]
    pub const fn owner(&self) -> LifecycleIdentity {
        self.owner
    }

    /// Returns the three independently selected carrier contracts.
    #[must_use]
    pub const fn carriers(&self) -> &BufferCarrierContracts {
        &self.carriers
    }

    /// Returns all carrier and total owned-byte bounds.
    #[must_use]
    pub const fn bounds(&self) -> &BufferPlanBounds {
        &self.bounds
    }

    /// Returns the required private Buffer Transition operation shape.
    #[must_use]
    pub const fn transition_operation(&self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Validates one carrier before the caller allocates its bytes.
    ///
    /// # Errors
    ///
    /// Returns a typed role, identity, schema, shape, or bound failure.
    pub const fn prepare_carrier(
        &self,
        role: BufferCarrierRole,
        stamp: BufferCarrierStamp,
        items: u64,
        bytes: u64,
    ) -> Result<BufferCarrierPreparation, BufferCarrierError> {
        super::carrier::prepare(
            self.carriers.contract(role),
            &self.bounds,
            role,
            stamp,
            items,
            bytes,
        )
    }

    /// Binds three exact carriers into one immutable Transaction plan.
    ///
    /// # Outcome
    ///
    /// Returns an accepted plan or a typed rejection retaining the unchanged
    /// stamp, target, and all three carriers.
    #[must_use]
    pub const fn bind_plan(
        &self,
        stamp: BufferPlanStamp,
        target: BufferStateStamp,
        origin: BufferPlanCarrier,
        operations: BufferPlanCarrier,
        preconditions: BufferPlanCarrier,
    ) -> BufferPlanBindingOutcome {
        let charge = validate_plan_parts(
            self,
            stamp,
            &origin,
            &operations,
            &preconditions,
        );
        let parts = BufferPlanParts {
            stamp,
            target,
            origin,
            operations,
            preconditions,
        };
        match charge {
            Ok(owned_bytes) => {
                BufferPlanBindingOutcome::Accepted(BufferTransactionPlan {
                    parts,
                    owned_bytes,
                })
            }
            Err(cause) => {
                BufferPlanBindingOutcome::Rejected(BufferPlanRejection {
                    cause,
                    parts,
                })
            }
        }
    }

    /// Revalidates a complete plan against this exact descriptor.
    ///
    /// # Errors
    ///
    /// Returns an owner, carrier, shape, or complete-byte bound failure.
    pub const fn validate_plan(
        &self,
        plan: &BufferTransactionPlan,
    ) -> Result<(), BufferPlanError> {
        match validate_plan_parts(
            self,
            plan.stamp(),
            plan.origin(),
            plan.operations(),
            plan.preconditions(),
        ) {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// Typed structural failure while binding a complete Transaction plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferPlanError {
    /// The plan targets another or retired descriptor-owner generation.
    OwnerGenerationMismatch,
    /// One role, type, schema, shape, or carrier bound was rejected.
    Carrier(BufferCarrierError),
    /// Complete plan-byte arithmetic or capacity was rejected.
    Bound(BoundError),
}

impl fmt::Display for BufferPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OwnerGenerationMismatch => {
                "Buffer plan targets another owner generation"
            }
            Self::Carrier(_) => "Buffer plan contains an invalid carrier",
            Self::Bound(_) => "Buffer plan exceeds its complete byte bound",
        })
    }
}

impl core::error::Error for BufferPlanError {}

/// Exact owning result of structurally binding one Buffer Transaction plan.
pub enum BufferPlanBindingOutcome {
    /// Structural validation succeeded and produced a complete plan.
    Accepted(BufferTransactionPlan),
    /// Structural validation failed without losing any proposal input.
    Rejected(BufferPlanRejection),
}

impl fmt::Debug for BufferPlanBindingOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted(plan) => {
                formatter.debug_tuple("Accepted").field(plan).finish()
            }
            Self::Rejected(rejection) => {
                formatter.debug_tuple("Rejected").field(rejection).finish()
            }
        }
    }
}

struct BufferPlanParts {
    stamp: BufferPlanStamp,
    target: BufferStateStamp,
    origin: BufferPlanCarrier,
    operations: BufferPlanCarrier,
    preconditions: BufferPlanCarrier,
}

/// Failed plan binding with all three carrier owners retained.
pub struct BufferPlanRejection {
    cause: BufferPlanError,
    parts: BufferPlanParts,
}

impl BufferPlanRejection {
    /// Returns the exact plan-binding failure.
    #[must_use]
    pub const fn cause(&self) -> BufferPlanError {
        self.cause
    }

    /// Returns the unchanged rejected plan stamp.
    #[must_use]
    pub const fn stamp(&self) -> BufferPlanStamp {
        self.parts.stamp
    }

    /// Returns the unchanged rejected target state.
    #[must_use]
    pub const fn target(&self) -> BufferStateStamp {
        self.parts.target
    }

    /// Borrows the unchanged origin carrier.
    #[must_use]
    pub const fn origin(&self) -> &BufferPlanCarrier {
        &self.parts.origin
    }

    /// Borrows the unchanged Operation carrier.
    #[must_use]
    pub const fn operations(&self) -> &BufferPlanCarrier {
        &self.parts.operations
    }

    /// Borrows the unchanged Precondition carrier.
    #[must_use]
    pub const fn preconditions(&self) -> &BufferPlanCarrier {
        &self.parts.preconditions
    }

    /// Returns ownership of every unchanged rejected carrier.
    #[must_use]
    pub fn into_carriers(
        self,
    ) -> (BufferPlanCarrier, BufferPlanCarrier, BufferPlanCarrier) {
        (self.parts.origin, self.parts.operations, self.parts.preconditions)
    }
}

impl fmt::Debug for BufferPlanRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BufferPlanRejection")
            .field("cause", &self.cause)
            .field("stamp", &self.parts.stamp)
            .field("target", &self.parts.target)
            .field("origin", &self.parts.origin)
            .field("operations", &self.parts.operations)
            .field("preconditions", &self.parts.preconditions)
            .finish()
    }
}

/// Structurally valid immutable input to a later private Buffer transition.
pub struct BufferTransactionPlan {
    parts: BufferPlanParts,
    owned_bytes: CheckedByteCount,
}

impl BufferTransactionPlan {
    /// Returns the exact plan identity and authority stamp.
    #[must_use]
    pub const fn stamp(&self) -> BufferPlanStamp {
        self.parts.stamp
    }

    /// Returns the exact Resource, Buffer, version, and History target.
    #[must_use]
    pub const fn target(&self) -> BufferStateStamp {
        self.parts.target
    }

    /// Borrows the self-contained origin carrier.
    #[must_use]
    pub const fn origin(&self) -> &BufferPlanCarrier {
        &self.parts.origin
    }

    /// Borrows the ordered Operation carrier.
    #[must_use]
    pub const fn operations(&self) -> &BufferPlanCarrier {
        &self.parts.operations
    }

    /// Borrows the optional Precondition carrier.
    #[must_use]
    pub const fn preconditions(&self) -> &BufferPlanCarrier {
        &self.parts.preconditions
    }

    /// Returns the checked complete owned carrier charge.
    #[must_use]
    pub const fn owned_bytes(&self) -> CheckedByteCount {
        self.owned_bytes
    }

    /// Releases the plan and returns all three carriers.
    #[must_use]
    pub fn into_carriers(
        self,
    ) -> (BufferPlanCarrier, BufferPlanCarrier, BufferPlanCarrier) {
        (self.parts.origin, self.parts.operations, self.parts.preconditions)
    }
}

impl fmt::Debug for BufferTransactionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BufferTransactionPlan")
            .field("stamp", &self.parts.stamp)
            .field("target", &self.parts.target)
            .field("origin", &self.parts.origin)
            .field("operations", &self.parts.operations)
            .field("preconditions", &self.parts.preconditions)
            .field("owned_bytes", &self.owned_bytes)
            .finish()
    }
}

const fn validate_plan_parts(
    descriptor: &BufferPlanDescriptor,
    stamp: BufferPlanStamp,
    origin: &BufferPlanCarrier,
    operations: &BufferPlanCarrier,
    preconditions: &BufferPlanCarrier,
) -> Result<CheckedByteCount, BufferPlanError> {
    if !descriptor.owner.same_generation(stamp.owner()) {
        return Err(BufferPlanError::OwnerGenerationMismatch);
    }
    match validate_role(descriptor, BufferCarrierRole::Origin, origin) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_role(descriptor, BufferCarrierRole::Operations, operations) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_role(
        descriptor,
        BufferCarrierRole::Preconditions,
        preconditions,
    ) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match descriptor.bounds.admit_plan_bytes(
        origin.size(),
        operations.size(),
        preconditions.size(),
    ) {
        Ok(value) => Ok(value),
        Err(error) => Err(BufferPlanError::Bound(error)),
    }
}

const fn validate_role(
    descriptor: &BufferPlanDescriptor,
    role: BufferCarrierRole,
    carrier: &BufferPlanCarrier,
) -> Result<(), BufferPlanError> {
    let result = super::carrier::validate_carrier(
        descriptor.carriers.contract(role),
        &descriptor.bounds,
        role,
        carrier,
    );
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(BufferPlanError::Carrier(error)),
    }
}
