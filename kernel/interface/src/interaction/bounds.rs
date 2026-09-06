//! Hard bounds for Interaction Session retention and Update delivery.

use core::fmt;

use crate::bound::{
    BoundDescriptor, BoundError, BoundUnit, BoundValue, CheckedByteCount,
    CheckedCount, ZeroPolicy,
};
use crate::identity::InstanceId;

/// Named Interaction quantity governed by one exact bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionBoundField {
    /// Concurrent admitted semantic Intents.
    PendingIntents,
    /// Aggregate bytes retained by pending Intents.
    PendingIntentBytes,
    /// Terminal Outcomes retained for delivery.
    RetainedOutcomes,
    /// Aggregate bytes charged to retained Outcomes.
    RetainedOutcomeBytes,
    /// Semantic items carried by one Update.
    UpdateItems,
    /// Maximum nesting depth of one Update carrier.
    UpdateDepth,
    /// Encoded payload bytes in one Update.
    UpdateBytes,
    /// Update items retained by one stream.
    RetainedUpdates,
    /// Aggregate payload bytes retained by one stream.
    RetainedUpdateBytes,
    /// Live data-delivery credits.
    UpdateCredits,
    /// Reserved control-delivery capacity.
    ControlDeliveries,
    /// Concurrent resynchronization requests.
    Resynchronizations,
    /// Abstract work reserved for resynchronization callbacks.
    ResynchronizationWork,
    /// Bytes reserved for resynchronization work and results.
    ResynchronizationBytes,
    /// Abstract bounded work permitted during close.
    CloseWork,
}

/// Typed construction failure for Interaction bound sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionBoundsError {
    /// A named bound uses another unit.
    WrongUnit {
        /// Rejected Interaction quantity.
        field: InteractionBoundField,
        /// Unit required by the contract.
        expected: BoundUnit,
        /// Unit supplied by the descriptor.
        actual: BoundUnit,
    },
    /// A named bound uses another zero policy.
    WrongZeroPolicy {
        /// Rejected Interaction quantity.
        field: InteractionBoundField,
        /// Zero policy required by the contract.
        expected: ZeroPolicy,
        /// Zero policy supplied by the descriptor.
        actual: ZeroPolicy,
    },
    /// A named bound belongs to another accounting owner.
    ChargeOwnerMismatch {
        /// Rejected Interaction quantity.
        field: InteractionBoundField,
        /// Charge owner selected for the complete Session.
        expected: InstanceId,
        /// Charge owner supplied by the descriptor.
        actual: InstanceId,
    },
    /// Request and Update bound families have different owners.
    BoundFamilyOwnerMismatch,
}

impl fmt::Display for InteractionBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongUnit { .. } => "Interaction bound uses the wrong unit",
            Self::WrongZeroPolicy { .. } => {
                "Interaction bound uses the wrong zero policy"
            }
            Self::ChargeOwnerMismatch { .. } => {
                "Interaction bound uses another charge owner"
            }
            Self::BoundFamilyOwnerMismatch => {
                "Interaction bound families use different owners"
            }
        })
    }
}

impl core::error::Error for InteractionBoundsError {}

/// Typed per-object or aggregate Interaction bound failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionBoundViolation {
    /// One checked value failed its field's declared bound.
    Bound {
        /// Rejected Interaction quantity.
        field: InteractionBoundField,
        /// Exact Foundation bound failure.
        cause: BoundError,
    },
    /// A prepared reservation belongs to another Session owner.
    ChargeOwnerMismatch {
        /// Owner required by the selected Session bounds.
        expected: InstanceId,
        /// Owner carried by the supplied reservation.
        actual: InstanceId,
    },
    /// A fixed reservation differs from the selected descriptor's maximum.
    ReservationMismatch {
        /// Rejected Interaction quantity.
        field: InteractionBoundField,
        /// Full reservation required by the selected Session.
        expected: BoundValue,
        /// Reservation carried by the supplied charge.
        actual: BoundValue,
    },
}

impl InteractionBoundViolation {
    pub(crate) const fn new(
        field: InteractionBoundField,
        cause: BoundError,
    ) -> Self {
        Self::Bound { field, cause }
    }

    /// Returns the exact named field when the failure is field-specific.
    #[must_use]
    pub const fn field(self) -> Option<InteractionBoundField> {
        match self {
            Self::Bound { field, .. }
            | Self::ReservationMismatch { field, .. } => Some(field),
            Self::ChargeOwnerMismatch { .. } => None,
        }
    }

    /// Returns the underlying checked-bound failure when applicable.
    #[must_use]
    pub const fn cause(self) -> Option<BoundError> {
        match self {
            Self::Bound { cause, .. } => Some(cause),
            Self::ChargeOwnerMismatch { .. }
            | Self::ReservationMismatch { .. } => None,
        }
    }
}

impl fmt::Display for InteractionBoundViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bound { .. } => {
                "Interaction request exceeds a declared bound"
            }
            Self::ChargeOwnerMismatch { .. } => {
                "Interaction reservation belongs to another owner"
            }
            Self::ReservationMismatch { .. } => {
                "Interaction reservation differs from the selected maximum"
            }
        })
    }
}

impl core::error::Error for InteractionBoundViolation {}

/// Per-Intent reservation held after successful admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionIntentCharge {
    owner: InstanceId,
    intent_bytes: CheckedByteCount,
    outcome_bytes: CheckedByteCount,
}

impl InteractionIntentCharge {
    /// Returns the Session accounting owner.
    #[must_use]
    pub const fn charge_owner(self) -> InstanceId {
        self.owner
    }

    /// Returns retained Semantic Intent bytes.
    #[must_use]
    pub const fn intent_bytes(self) -> CheckedByteCount {
        self.intent_bytes
    }

    /// Returns reserved terminal Outcome bytes.
    #[must_use]
    pub const fn outcome_bytes(self) -> CheckedByteCount {
        self.outcome_bytes
    }
}

/// Aggregate pending state after one Intent reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionPendingTotals {
    intents: CheckedCount,
    intent_bytes: CheckedByteCount,
    outcomes: CheckedCount,
    outcome_bytes: CheckedByteCount,
}

impl InteractionPendingTotals {
    /// Returns the resulting pending Intent count.
    #[must_use]
    pub const fn intents(self) -> CheckedCount {
        self.intents
    }

    /// Returns the resulting retained Intent bytes.
    #[must_use]
    pub const fn intent_bytes(self) -> CheckedByteCount {
        self.intent_bytes
    }

    /// Returns the resulting reserved Outcome count.
    #[must_use]
    pub const fn outcomes(self) -> CheckedCount {
        self.outcomes
    }

    /// Returns the resulting reserved Outcome bytes.
    #[must_use]
    pub const fn outcome_bytes(self) -> CheckedByteCount {
        self.outcome_bytes
    }
}

/// Pending Intent and terminal Outcome reservation bounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionRequestBounds {
    owner: InstanceId,
    pending_intents: BoundDescriptor,
    pending_intent_bytes: BoundDescriptor,
    retained_outcomes: BoundDescriptor,
    retained_outcome_bytes: BoundDescriptor,
}

impl InteractionRequestBounds {
    /// Constructs one coherent request-bound family.
    ///
    /// # Errors
    ///
    /// Returns the first unit, zero-policy, or charge-owner mismatch.
    pub const fn new(
        owner: InstanceId,
        pending_intents: BoundDescriptor,
        pending_intent_bytes: BoundDescriptor,
        retained_outcomes: BoundDescriptor,
        retained_outcome_bytes: BoundDescriptor,
    ) -> Result<Self, InteractionBoundsError> {
        match require_bound(
            InteractionBoundField::PendingIntents,
            pending_intents,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            InteractionBoundField::PendingIntentBytes,
            pending_intent_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            InteractionBoundField::RetainedOutcomes,
            retained_outcomes,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            InteractionBoundField::RetainedOutcomeBytes,
            retained_outcome_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self {
            owner,
            pending_intents,
            pending_intent_bytes,
            retained_outcomes,
            retained_outcome_bytes,
        })
    }

    /// Returns the shared Session accounting owner.
    #[must_use]
    pub const fn charge_owner(&self) -> InstanceId {
        self.owner
    }

    /// Returns the pending-Intent count bound.
    #[must_use]
    pub const fn pending_intents(&self) -> BoundDescriptor {
        self.pending_intents
    }

    /// Returns the aggregate pending-Intent byte bound.
    #[must_use]
    pub const fn pending_intent_bytes(&self) -> BoundDescriptor {
        self.pending_intent_bytes
    }

    /// Returns the retained terminal-Outcome count bound.
    #[must_use]
    pub const fn retained_outcomes(&self) -> BoundDescriptor {
        self.retained_outcomes
    }

    /// Returns the retained terminal-Outcome byte bound.
    #[must_use]
    pub const fn retained_outcome_bytes(&self) -> BoundDescriptor {
        self.retained_outcome_bytes
    }

    /// Prepares one per-Intent and terminal reservation.
    ///
    /// # Errors
    ///
    /// Returns the first zero, unit, or per-object exhaustion.
    pub const fn prepare_intent(
        &self,
        intent_bytes: u64,
    ) -> Result<InteractionIntentCharge, InteractionBoundViolation> {
        let intent_bytes = match checked_bytes(
            InteractionBoundField::PendingIntentBytes,
            intent_bytes,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let outcome_bytes = match self.retained_outcome_bytes.per_object() {
            BoundValue::Bytes(value) => value,
            value => {
                return Err(InteractionBoundViolation::new(
                    InteractionBoundField::RetainedOutcomeBytes,
                    BoundError::UnitMismatch {
                        expected: BoundUnit::Bytes,
                        actual: value.unit(),
                    },
                ));
            }
        };
        let charge = InteractionIntentCharge {
            owner: self.owner,
            intent_bytes,
            outcome_bytes,
        };
        match self.validate_charge(charge) {
            Ok(()) => Ok(charge),
            Err(error) => Err(error),
        }
    }

    /// Revalidates one prepared per-Intent reservation.
    ///
    /// # Errors
    ///
    /// Returns owner or fixed-reservation mismatch, or per-object exhaustion.
    pub const fn validate_charge(
        &self,
        charge: InteractionIntentCharge,
    ) -> Result<(), InteractionBoundViolation> {
        if charge.charge_owner().value() != self.owner.value() {
            return Err(InteractionBoundViolation::ChargeOwnerMismatch {
                expected: self.owner,
                actual: charge.charge_owner(),
            });
        }
        let pending_one = match one_count(InteractionBoundField::PendingIntents)
        {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match admit(
            InteractionBoundField::PendingIntents,
            self.pending_intents,
            pending_one,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            InteractionBoundField::PendingIntentBytes,
            self.pending_intent_bytes,
            BoundValue::Bytes(charge.intent_bytes()),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let outcome_one =
            match one_count(InteractionBoundField::RetainedOutcomes) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
        match admit(
            InteractionBoundField::RetainedOutcomes,
            self.retained_outcomes,
            outcome_one,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        require_full_reservation(
            InteractionBoundField::RetainedOutcomeBytes,
            self.retained_outcome_bytes,
            BoundValue::Bytes(charge.outcome_bytes()),
        )
    }

    /// Validates aggregate pending and terminal-retention capacity.
    ///
    /// # Errors
    ///
    /// Returns checked overflow or the first aggregate exhaustion.
    pub const fn admit_pending(
        &self,
        current_intents: u64,
        current_intent_bytes: u64,
        current_outcomes: u64,
        current_outcome_bytes: u64,
        charge: InteractionIntentCharge,
    ) -> Result<InteractionPendingTotals, InteractionBoundViolation> {
        match self.validate_charge(charge) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let intents = match aggregate_count(
            InteractionBoundField::PendingIntents,
            self.pending_intents,
            current_intents,
            1,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let intent_bytes = match aggregate_bytes(
            InteractionBoundField::PendingIntentBytes,
            self.pending_intent_bytes,
            current_intent_bytes,
            charge.intent_bytes().value(),
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let outcomes = match aggregate_count(
            InteractionBoundField::RetainedOutcomes,
            self.retained_outcomes,
            current_outcomes,
            1,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let outcome_bytes = match aggregate_bytes(
            InteractionBoundField::RetainedOutcomeBytes,
            self.retained_outcome_bytes,
            current_outcome_bytes,
            charge.outcome_bytes().value(),
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(InteractionPendingTotals {
            intents,
            intent_bytes,
            outcomes,
            outcome_bytes,
        })
    }
}

/// Checked structural shape of one payload-bearing Update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateShape {
    items: CheckedCount,
    depth: CheckedCount,
    bytes: CheckedByteCount,
}

impl RepresentationUpdateShape {
    /// Returns the semantic item count.
    #[must_use]
    pub const fn items(self) -> CheckedCount {
        self.items
    }

    /// Returns the maximum nesting depth.
    #[must_use]
    pub const fn depth(self) -> CheckedCount {
        self.depth
    }

    /// Returns the exact payload byte count.
    #[must_use]
    pub const fn bytes(self) -> CheckedByteCount {
        self.bytes
    }
}

/// Aggregate retained stream state after one Update reservation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateTotals {
    updates: CheckedCount,
    bytes: CheckedByteCount,
}

impl RepresentationUpdateTotals {
    /// Returns the resulting retained Update count.
    #[must_use]
    pub const fn updates(self) -> CheckedCount {
        self.updates
    }

    /// Returns the resulting retained payload bytes.
    #[must_use]
    pub const fn bytes(self) -> CheckedByteCount {
        self.bytes
    }
}

/// Per-Update payload-shape bound descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateShapeBounds {
    items: BoundDescriptor,
    depth: BoundDescriptor,
    bytes: BoundDescriptor,
}

impl RepresentationUpdateShapeBounds {
    /// Groups the item, depth, and byte descriptors for validation.
    #[must_use]
    pub const fn new(
        items: BoundDescriptor,
        depth: BoundDescriptor,
        bytes: BoundDescriptor,
    ) -> Self {
        Self { items, depth, bytes }
    }

    /// Returns the per-Update item bound.
    #[must_use]
    pub const fn items(self) -> BoundDescriptor {
        self.items
    }

    /// Returns the per-Update nesting bound.
    #[must_use]
    pub const fn depth(self) -> BoundDescriptor {
        self.depth
    }

    /// Returns the per-Update payload byte bound.
    #[must_use]
    pub const fn bytes(self) -> BoundDescriptor {
        self.bytes
    }
}

/// Count, work, and byte bounds for deferred resynchronization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationResynchronizationBounds {
    requests: BoundDescriptor,
    work: BoundDescriptor,
    bytes: BoundDescriptor,
}

impl RepresentationResynchronizationBounds {
    /// Groups one exact resynchronization bound family.
    #[must_use]
    pub const fn new(
        requests: BoundDescriptor,
        work: BoundDescriptor,
        bytes: BoundDescriptor,
    ) -> Self {
        Self { requests, work, bytes }
    }

    /// Returns the concurrent request-count bound.
    #[must_use]
    pub const fn requests(self) -> BoundDescriptor {
        self.requests
    }

    /// Returns the abstract work-unit bound.
    #[must_use]
    pub const fn work(self) -> BoundDescriptor {
        self.work
    }

    /// Returns the reserved-byte bound.
    #[must_use]
    pub const fn bytes(self) -> BoundDescriptor {
        self.bytes
    }
}

/// Checked reservation for one deferred resynchronization request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationResynchronizationCharge {
    owner: InstanceId,
    requests: CheckedCount,
    work: CheckedCount,
    bytes: CheckedByteCount,
}

impl RepresentationResynchronizationCharge {
    /// Returns the Session accounting owner.
    #[must_use]
    pub const fn charge_owner(self) -> InstanceId {
        self.owner
    }

    /// Returns the one reserved request slot.
    #[must_use]
    pub const fn requests(self) -> CheckedCount {
        self.requests
    }

    /// Returns the reserved abstract work units.
    #[must_use]
    pub const fn work(self) -> CheckedCount {
        self.work
    }

    /// Returns the reserved work and result bytes.
    #[must_use]
    pub const fn bytes(self) -> CheckedByteCount {
        self.bytes
    }
}

/// Aggregate resynchronization state after one admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationResynchronizationTotals {
    owner: InstanceId,
    requests: CheckedCount,
    work: CheckedCount,
    bytes: CheckedByteCount,
}

impl RepresentationResynchronizationTotals {
    /// Returns the Session accounting owner.
    #[must_use]
    pub const fn charge_owner(self) -> InstanceId {
        self.owner
    }

    /// Returns the resulting admitted request count.
    #[must_use]
    pub const fn requests(self) -> CheckedCount {
        self.requests
    }

    /// Returns the resulting reserved work units.
    #[must_use]
    pub const fn work(self) -> CheckedCount {
        self.work
    }

    /// Returns the resulting reserved bytes.
    #[must_use]
    pub const fn bytes(self) -> CheckedByteCount {
        self.bytes
    }
}

/// Retention, delivery-credit, and resynchronization bound descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateFlowBounds {
    retained_updates: BoundDescriptor,
    retained_bytes: BoundDescriptor,
    credits: BoundDescriptor,
    control: BoundDescriptor,
    resynchronization: RepresentationResynchronizationBounds,
}

impl RepresentationUpdateFlowBounds {
    /// Groups the Update-stream flow-control descriptors.
    #[must_use]
    pub const fn new(
        retained_updates: BoundDescriptor,
        retained_bytes: BoundDescriptor,
        credits: BoundDescriptor,
        control: BoundDescriptor,
        resynchronization: RepresentationResynchronizationBounds,
    ) -> Self {
        Self {
            retained_updates,
            retained_bytes,
            credits,
            control,
            resynchronization,
        }
    }

    /// Returns the retained Update count bound.
    #[must_use]
    pub const fn retained_updates(self) -> BoundDescriptor {
        self.retained_updates
    }

    /// Returns the aggregate retained Update byte bound.
    #[must_use]
    pub const fn retained_bytes(self) -> BoundDescriptor {
        self.retained_bytes
    }

    /// Returns the live data-credit bound.
    #[must_use]
    pub const fn credits(self) -> BoundDescriptor {
        self.credits
    }

    /// Returns the reserved control-delivery bound.
    #[must_use]
    pub const fn control(self) -> BoundDescriptor {
        self.control
    }

    /// Returns the resynchronization bound family.
    #[must_use]
    pub const fn resynchronization(
        self,
    ) -> RepresentationResynchronizationBounds {
        self.resynchronization
    }
}

/// Update carrier, retention, credit, and resynchronization bounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateBounds {
    owner: InstanceId,
    shape: RepresentationUpdateShapeBounds,
    flow: RepresentationUpdateFlowBounds,
}

impl RepresentationUpdateBounds {
    /// Constructs one coherent Update-bound family.
    ///
    /// # Errors
    ///
    /// Returns the first unit, zero-policy, or charge-owner mismatch.
    pub const fn new(
        owner: InstanceId,
        shape: RepresentationUpdateShapeBounds,
        flow: RepresentationUpdateFlowBounds,
    ) -> Result<Self, InteractionBoundsError> {
        match validate_update_bounds(
            owner,
            &[
                (InteractionBoundField::UpdateItems, shape.items()),
                (InteractionBoundField::UpdateDepth, shape.depth()),
                (InteractionBoundField::UpdateBytes, shape.bytes()),
                (
                    InteractionBoundField::RetainedUpdates,
                    flow.retained_updates(),
                ),
                (
                    InteractionBoundField::RetainedUpdateBytes,
                    flow.retained_bytes(),
                ),
                (InteractionBoundField::UpdateCredits, flow.credits()),
                (InteractionBoundField::ControlDeliveries, flow.control()),
                (
                    InteractionBoundField::Resynchronizations,
                    flow.resynchronization().requests(),
                ),
                (
                    InteractionBoundField::ResynchronizationWork,
                    flow.resynchronization().work(),
                ),
                (
                    InteractionBoundField::ResynchronizationBytes,
                    flow.resynchronization().bytes(),
                ),
            ],
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self { owner, shape, flow })
    }

    /// Returns the shared Session accounting owner.
    #[must_use]
    pub const fn charge_owner(&self) -> InstanceId {
        self.owner
    }

    /// Returns the per-Update item bound.
    #[must_use]
    pub const fn items(&self) -> BoundDescriptor {
        self.shape.items()
    }

    /// Returns the per-Update nesting bound.
    #[must_use]
    pub const fn depth(&self) -> BoundDescriptor {
        self.shape.depth()
    }

    /// Returns the per-Update payload byte bound.
    #[must_use]
    pub const fn bytes(&self) -> BoundDescriptor {
        self.shape.bytes()
    }

    /// Returns the retained Update count bound.
    #[must_use]
    pub const fn retained_updates(&self) -> BoundDescriptor {
        self.flow.retained_updates()
    }

    /// Returns the aggregate retained Update byte bound.
    #[must_use]
    pub const fn retained_bytes(&self) -> BoundDescriptor {
        self.flow.retained_bytes()
    }

    /// Returns the live data-credit bound.
    #[must_use]
    pub const fn credits(&self) -> BoundDescriptor {
        self.flow.credits()
    }

    /// Returns the reserved control-delivery bound.
    #[must_use]
    pub const fn control(&self) -> BoundDescriptor {
        self.flow.control()
    }

    /// Returns the concurrent resynchronization bound.
    #[must_use]
    pub const fn resynchronizations(&self) -> BoundDescriptor {
        self.flow.resynchronization().requests()
    }

    /// Returns the resynchronization work-unit bound.
    #[must_use]
    pub const fn resynchronization_work(&self) -> BoundDescriptor {
        self.flow.resynchronization().work()
    }

    /// Returns the resynchronization byte bound.
    #[must_use]
    pub const fn resynchronization_bytes(&self) -> BoundDescriptor {
        self.flow.resynchronization().bytes()
    }

    /// Validates one payload shape before allocation.
    ///
    /// # Errors
    ///
    /// Returns the first zero, unit, or hard-limit failure.
    pub const fn prepare_shape(
        &self,
        items: u64,
        depth: u64,
        bytes: u64,
    ) -> Result<RepresentationUpdateShape, InteractionBoundViolation> {
        let items = match checked_count(
            InteractionBoundField::UpdateItems,
            items,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let depth = match checked_count(
            InteractionBoundField::UpdateDepth,
            depth,
            ZeroPolicy::Allow,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let bytes = match checked_bytes(
            InteractionBoundField::UpdateBytes,
            bytes,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match admit(
            InteractionBoundField::UpdateItems,
            self.items(),
            BoundValue::Count(items),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            InteractionBoundField::UpdateDepth,
            self.depth(),
            BoundValue::Count(depth),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            InteractionBoundField::UpdateBytes,
            self.bytes(),
            BoundValue::Bytes(bytes),
        ) {
            Ok(()) => Ok(RepresentationUpdateShape { items, depth, bytes }),
            Err(error) => Err(error),
        }
    }

    /// Validates an initial data-credit grant.
    ///
    /// # Errors
    ///
    /// Returns zero or credit-limit exhaustion.
    pub const fn prepare_credit(
        &self,
        credits: u64,
    ) -> Result<CheckedCount, InteractionBoundViolation> {
        let credits = match checked_count(
            InteractionBoundField::UpdateCredits,
            credits,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match admit(
            InteractionBoundField::UpdateCredits,
            self.credits(),
            BoundValue::Count(credits),
        ) {
            Ok(()) => Ok(credits),
            Err(error) => Err(error),
        }
    }

    /// Admits one credit grant against current Session-wide credit use.
    ///
    /// # Errors
    ///
    /// Returns checked overflow or aggregate credit exhaustion.
    pub const fn admit_credits(
        &self,
        current: u64,
        grant: CheckedCount,
    ) -> Result<CheckedCount, InteractionBoundViolation> {
        aggregate_count(
            InteractionBoundField::UpdateCredits,
            self.credits(),
            current,
            grant.value(),
        )
    }

    /// Validates reserved control-delivery capacity.
    ///
    /// # Errors
    ///
    /// Returns exhaustion when at least one control item cannot be retained.
    pub const fn require_control_reserve(
        &self,
    ) -> Result<(), InteractionBoundViolation> {
        let one = match one_count(InteractionBoundField::ControlDeliveries) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        admit(InteractionBoundField::ControlDeliveries, self.control(), one)
    }

    /// Prepares one deferred resynchronization reservation.
    ///
    /// # Errors
    ///
    /// Returns zero, unit, or per-request exhaustion.
    pub const fn prepare_resynchronization(
        &self,
    ) -> Result<
        self::RepresentationResynchronizationCharge,
        InteractionBoundViolation,
    > {
        let requests = match checked_count(
            InteractionBoundField::Resynchronizations,
            1,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let work = match self.resynchronization_work().per_object() {
            BoundValue::Count(value) => value,
            value => {
                return Err(InteractionBoundViolation::new(
                    InteractionBoundField::ResynchronizationWork,
                    BoundError::UnitMismatch {
                        expected: BoundUnit::Count,
                        actual: value.unit(),
                    },
                ));
            }
        };
        let bytes = match self.resynchronization_bytes().per_object() {
            BoundValue::Bytes(value) => value,
            value => {
                return Err(InteractionBoundViolation::new(
                    InteractionBoundField::ResynchronizationBytes,
                    BoundError::UnitMismatch {
                        expected: BoundUnit::Bytes,
                        actual: value.unit(),
                    },
                ));
            }
        };
        let charge = RepresentationResynchronizationCharge {
            owner: self.owner,
            requests,
            work,
            bytes,
        };
        match self.validate_resynchronization_charge(charge) {
            Ok(()) => Ok(charge),
            Err(error) => Err(error),
        }
    }

    /// Revalidates one provider-prepared resynchronization reservation.
    ///
    /// # Errors
    ///
    /// Returns owner or fixed-reservation mismatch, or per-request exhaustion.
    pub const fn validate_resynchronization_charge(
        &self,
        charge: RepresentationResynchronizationCharge,
    ) -> Result<(), InteractionBoundViolation> {
        if charge.charge_owner().value() != self.owner.value() {
            return Err(InteractionBoundViolation::ChargeOwnerMismatch {
                expected: self.owner,
                actual: charge.charge_owner(),
            });
        }
        match admit(
            InteractionBoundField::Resynchronizations,
            self.resynchronizations(),
            BoundValue::Count(charge.requests()),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_full_reservation(
            InteractionBoundField::ResynchronizationWork,
            self.resynchronization_work(),
            BoundValue::Count(charge.work()),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        require_full_reservation(
            InteractionBoundField::ResynchronizationBytes,
            self.resynchronization_bytes(),
            BoundValue::Bytes(charge.bytes()),
        )
    }

    /// Admits prepared resynchronization work against Session-wide use.
    ///
    /// # Errors
    ///
    /// Returns checked overflow or the first aggregate exhaustion.
    pub const fn admit_resynchronization(
        &self,
        current_requests: u64,
        current_work: u64,
        current_bytes: u64,
        charge: RepresentationResynchronizationCharge,
    ) -> Result<
        self::RepresentationResynchronizationTotals,
        InteractionBoundViolation,
    > {
        match self.validate_resynchronization_charge(charge) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let requests = match aggregate_count(
            InteractionBoundField::Resynchronizations,
            self.resynchronizations(),
            current_requests,
            charge.requests().value(),
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let work = match aggregate_count(
            InteractionBoundField::ResynchronizationWork,
            self.resynchronization_work(),
            current_work,
            charge.work().value(),
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let bytes = match aggregate_bytes(
            InteractionBoundField::ResynchronizationBytes,
            self.resynchronization_bytes(),
            current_bytes,
            charge.bytes().value(),
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(RepresentationResynchronizationTotals {
            owner: self.owner,
            requests,
            work,
            bytes,
        })
    }

    /// Validates aggregate retention for one payload-bearing Update.
    ///
    /// # Errors
    ///
    /// Returns checked overflow or aggregate exhaustion.
    pub const fn admit_retained(
        &self,
        current_updates: u64,
        current_bytes: u64,
        payload_bytes: u64,
    ) -> Result<RepresentationUpdateTotals, InteractionBoundViolation> {
        let updates = match aggregate_count(
            InteractionBoundField::RetainedUpdates,
            self.retained_updates(),
            current_updates,
            1,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let bytes = match aggregate_bytes_allow_zero(
            InteractionBoundField::RetainedUpdateBytes,
            self.retained_bytes(),
            current_bytes,
            payload_bytes,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(RepresentationUpdateTotals { updates, bytes })
    }
}

/// Complete Session request, Update, and close bound family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionSessionBounds {
    owner: InstanceId,
    requests: InteractionRequestBounds,
    updates: RepresentationUpdateBounds,
    close_work: BoundDescriptor,
}

impl InteractionSessionBounds {
    /// Constructs one Session-wide bound family.
    ///
    /// # Errors
    ///
    /// Returns owner, unit, or zero-policy incoherence.
    pub const fn new(
        owner: InstanceId,
        requests: InteractionRequestBounds,
        updates: RepresentationUpdateBounds,
        close_work: BoundDescriptor,
    ) -> Result<Self, InteractionBoundsError> {
        if requests.charge_owner().value() != owner.value()
            || updates.charge_owner().value() != owner.value()
        {
            return Err(InteractionBoundsError::BoundFamilyOwnerMismatch);
        }
        match require_bound(
            InteractionBoundField::CloseWork,
            close_work,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            owner,
        ) {
            Ok(()) => Ok(Self { owner, requests, updates, close_work }),
            Err(error) => Err(error),
        }
    }

    /// Returns the one Session accounting owner.
    #[must_use]
    pub const fn charge_owner(&self) -> InstanceId {
        self.owner
    }

    /// Returns pending Intent and terminal Outcome bounds.
    #[must_use]
    pub const fn requests(&self) -> &InteractionRequestBounds {
        &self.requests
    }

    /// Returns Update, credit, and resynchronization bounds.
    #[must_use]
    pub const fn updates(&self) -> &RepresentationUpdateBounds {
        &self.updates
    }

    /// Returns the bounded close-work descriptor.
    #[must_use]
    pub const fn close_work(&self) -> BoundDescriptor {
        self.close_work
    }

    /// Validates one bounded close-work reservation.
    ///
    /// # Errors
    ///
    /// Returns zero or close-work exhaustion.
    pub const fn prepare_close_work(
        &self,
        work: u64,
    ) -> Result<CheckedCount, InteractionBoundViolation> {
        let work = match checked_count(
            InteractionBoundField::CloseWork,
            work,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match admit(
            InteractionBoundField::CloseWork,
            self.close_work,
            BoundValue::Count(work),
        ) {
            Ok(()) => Ok(work),
            Err(error) => Err(error),
        }
    }
}

const fn validate_update_bounds(
    owner: InstanceId,
    bounds: &[(InteractionBoundField, BoundDescriptor); 10],
) -> Result<(), InteractionBoundsError> {
    let mut index = 0;
    while index < bounds.len() {
        let (field, descriptor) = bounds[index];
        let (unit, zero) = update_bound_shape(field);
        match require_bound(field, descriptor, unit, zero, owner) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        index += 1;
    }
    Ok(())
}

const fn update_bound_shape(
    field: InteractionBoundField,
) -> (BoundUnit, ZeroPolicy) {
    match field {
        InteractionBoundField::UpdateBytes
        | InteractionBoundField::RetainedUpdateBytes
        | InteractionBoundField::ResynchronizationBytes => {
            let zero = match field {
                InteractionBoundField::UpdateBytes
                | InteractionBoundField::ResynchronizationBytes => {
                    ZeroPolicy::Reject
                }
                _ => ZeroPolicy::Allow,
            };
            (BoundUnit::Bytes, zero)
        }
        InteractionBoundField::UpdateDepth => {
            (BoundUnit::Count, ZeroPolicy::Allow)
        }
        _ => (BoundUnit::Count, ZeroPolicy::Reject),
    }
}

const fn require_bound(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    unit: BoundUnit,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Result<(), InteractionBoundsError> {
    if unit_code(descriptor.unit()) != unit_code(unit) {
        return Err(InteractionBoundsError::WrongUnit {
            field,
            expected: unit,
            actual: descriptor.unit(),
        });
    }
    if zero_code(descriptor.zero_policy()) != zero_code(zero) {
        return Err(InteractionBoundsError::WrongZeroPolicy {
            field,
            expected: zero,
            actual: descriptor.zero_policy(),
        });
    }
    if descriptor.charge_owner().value() != owner.value() {
        return Err(InteractionBoundsError::ChargeOwnerMismatch {
            field,
            expected: owner,
            actual: descriptor.charge_owner(),
        });
    }
    Ok(())
}

const fn checked_count(
    field: InteractionBoundField,
    value: u64,
    zero: ZeroPolicy,
) -> Result<CheckedCount, InteractionBoundViolation> {
    match CheckedCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(cause) => Err(InteractionBoundViolation::new(field, cause)),
    }
}

const fn checked_bytes(
    field: InteractionBoundField,
    value: u64,
    zero: ZeroPolicy,
) -> Result<CheckedByteCount, InteractionBoundViolation> {
    match CheckedByteCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(cause) => Err(InteractionBoundViolation::new(field, cause)),
    }
}

const fn admit(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    value: BoundValue,
) -> Result<(), InteractionBoundViolation> {
    match descriptor.admit_per_object(value) {
        Ok(()) => Ok(()),
        Err(cause) => Err(InteractionBoundViolation::new(field, cause)),
    }
}

const fn require_full_reservation(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    actual: BoundValue,
) -> Result<(), InteractionBoundViolation> {
    match admit(field, descriptor, actual) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let expected = descriptor.per_object();
    if expected.value() != actual.value() {
        return Err(InteractionBoundViolation::ReservationMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

const fn aggregate_count(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    current: u64,
    additional: u64,
) -> Result<CheckedCount, InteractionBoundViolation> {
    let current = match CheckedCount::new(current, ZeroPolicy::Allow) {
        Ok(value) => value,
        Err(cause) => return Err(InteractionBoundViolation::new(field, cause)),
    };
    let additional = match CheckedCount::new(additional, ZeroPolicy::Reject) {
        Ok(value) => value,
        Err(cause) => return Err(InteractionBoundViolation::new(field, cause)),
    };
    match descriptor.admit_aggregate(
        BoundValue::Count(current),
        BoundValue::Count(additional),
    ) {
        Ok(BoundValue::Count(value)) => Ok(value),
        Ok(value) => Err(unit_mismatch(field, BoundUnit::Count, value.unit())),
        Err(cause) => Err(InteractionBoundViolation::new(field, cause)),
    }
}

const fn aggregate_bytes(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    current: u64,
    additional: u64,
) -> Result<CheckedByteCount, InteractionBoundViolation> {
    aggregate_bytes_with_zero(
        field,
        descriptor,
        current,
        additional,
        ZeroPolicy::Reject,
    )
}

const fn aggregate_bytes_allow_zero(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    current: u64,
    additional: u64,
) -> Result<CheckedByteCount, InteractionBoundViolation> {
    aggregate_bytes_with_zero(
        field,
        descriptor,
        current,
        additional,
        ZeroPolicy::Allow,
    )
}

const fn aggregate_bytes_with_zero(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    current: u64,
    additional: u64,
    zero: ZeroPolicy,
) -> Result<CheckedByteCount, InteractionBoundViolation> {
    let current = match CheckedByteCount::new(current, ZeroPolicy::Allow) {
        Ok(value) => value,
        Err(cause) => return Err(InteractionBoundViolation::new(field, cause)),
    };
    let additional = match CheckedByteCount::new(additional, zero) {
        Ok(value) => value,
        Err(cause) => return Err(InteractionBoundViolation::new(field, cause)),
    };
    match descriptor.admit_aggregate(
        BoundValue::Bytes(current),
        BoundValue::Bytes(additional),
    ) {
        Ok(BoundValue::Bytes(value)) => Ok(value),
        Ok(value) => Err(unit_mismatch(field, BoundUnit::Bytes, value.unit())),
        Err(cause) => Err(InteractionBoundViolation::new(field, cause)),
    }
}

const fn unit_mismatch(
    field: InteractionBoundField,
    expected: BoundUnit,
    actual: BoundUnit,
) -> InteractionBoundViolation {
    InteractionBoundViolation::new(
        field,
        BoundError::UnitMismatch { expected, actual },
    )
}

const fn one_count(
    field: InteractionBoundField,
) -> Result<BoundValue, InteractionBoundViolation> {
    match CheckedCount::new(1, ZeroPolicy::Reject) {
        Ok(value) => Ok(BoundValue::Count(value)),
        Err(cause) => Err(InteractionBoundViolation::new(field, cause)),
    }
}

const fn unit_code(unit: BoundUnit) -> u8 {
    match unit {
        BoundUnit::Count => 0,
        BoundUnit::Bytes => 1,
        BoundUnit::DurationNanoseconds => 2,
    }
}

const fn zero_code(zero: ZeroPolicy) -> u8 {
    match zero {
        ZeroPolicy::Allow => 0,
        ZeroPolicy::Reject => 1,
    }
}
