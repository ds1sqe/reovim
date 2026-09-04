//! Hard bounds for one public Buffer Transaction plan.

use core::fmt;

use crate::bound::{
    BoundDescriptor, BoundError, BoundUnit, BoundValue, CheckedByteCount,
    ZeroPolicy,
};
use crate::identity::InstanceId;

/// Named Buffer-plan bound whose unit, zero policy, or owner was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferPlanBoundField {
    /// Ordered Operations carried by one plan.
    Operations,
    /// Optional Preconditions carried by one plan.
    Preconditions,
    /// Bytes in the one origin/provenance record.
    OriginBytes,
    /// Bytes encoding ordered Operations.
    OperationBytes,
    /// Bytes encoding optional Preconditions.
    PreconditionBytes,
    /// Total owned carrier bytes retained by one or more plans.
    OwnedBytes,
}

/// Typed construction failure for a Buffer-plan bound set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferPlanBoundsError {
    /// A named bound uses a unit different from its contract.
    WrongUnit {
        /// Rejected bound field.
        field: BufferPlanBoundField,
        /// Unit required by the field.
        expected: BoundUnit,
        /// Unit supplied by the descriptor.
        actual: BoundUnit,
    },
    /// A named bound uses a different zero policy.
    WrongZeroPolicy {
        /// Rejected bound field.
        field: BufferPlanBoundField,
        /// Zero policy required by the field.
        expected: ZeroPolicy,
        /// Zero policy supplied by the descriptor.
        actual: ZeroPolicy,
    },
    /// A named bound belongs to another accounting owner.
    ChargeOwnerMismatch {
        /// Rejected bound field.
        field: BufferPlanBoundField,
        /// Charge owner selected for the complete plan bound set.
        expected: InstanceId,
        /// Charge owner supplied by the named descriptor.
        actual: InstanceId,
    },
}

impl fmt::Display for BufferPlanBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongUnit { .. } => "Buffer-plan bound uses the wrong unit",
            Self::WrongZeroPolicy { .. } => {
                "Buffer-plan bound uses the wrong zero policy"
            }
            Self::ChargeOwnerMismatch { .. } => {
                "Buffer-plan bound uses another charge owner"
            }
        })
    }
}

impl core::error::Error for BufferPlanBoundsError {}

/// Complete carrier and owned-state bounds for one Transaction plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferPlanBounds {
    charge_owner: InstanceId,
    operations: BoundDescriptor,
    preconditions: BoundDescriptor,
    origin_bytes: BoundDescriptor,
    operation_bytes: BoundDescriptor,
    precondition_bytes: BoundDescriptor,
    owned_bytes: BoundDescriptor,
}

impl BufferPlanBounds {
    /// Constructs a coherent plan bound set.
    ///
    /// # Errors
    ///
    /// Returns a typed unit, zero-policy, or charge-owner failure.
    pub const fn new(
        charge_owner: InstanceId,
        operations: BoundDescriptor,
        preconditions: BoundDescriptor,
        origin_bytes: BoundDescriptor,
        operation_bytes: BoundDescriptor,
        precondition_bytes: BoundDescriptor,
        owned_bytes: BoundDescriptor,
    ) -> Result<Self, BufferPlanBoundsError> {
        match require_bound(
            BufferPlanBoundField::Operations,
            operations,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            BufferPlanBoundField::Preconditions,
            preconditions,
            BoundUnit::Count,
            ZeroPolicy::Allow,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            BufferPlanBoundField::OriginBytes,
            origin_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            BufferPlanBoundField::OperationBytes,
            operation_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            BufferPlanBoundField::PreconditionBytes,
            precondition_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Allow,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            BufferPlanBoundField::OwnedBytes,
            owned_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self {
            charge_owner,
            operations,
            preconditions,
            origin_bytes,
            operation_bytes,
            precondition_bytes,
            owned_bytes,
        })
    }

    /// Returns the accounting owner shared by every bound in the set.
    #[must_use]
    pub const fn charge_owner(&self) -> InstanceId {
        self.charge_owner
    }

    /// Returns the ordered-Operation count bound.
    #[must_use]
    pub const fn operations(&self) -> BoundDescriptor {
        self.operations
    }

    /// Returns the optional-Precondition count bound.
    #[must_use]
    pub const fn preconditions(&self) -> BoundDescriptor {
        self.preconditions
    }

    /// Returns the origin-carrier byte bound.
    #[must_use]
    pub const fn origin_bytes(&self) -> BoundDescriptor {
        self.origin_bytes
    }

    /// Returns the Operation-carrier byte bound.
    #[must_use]
    pub const fn operation_bytes(&self) -> BoundDescriptor {
        self.operation_bytes
    }

    /// Returns the Precondition-carrier byte bound.
    #[must_use]
    pub const fn precondition_bytes(&self) -> BoundDescriptor {
        self.precondition_bytes
    }

    /// Returns the total owned plan-byte bound and aggregate scope.
    #[must_use]
    pub const fn owned_bytes(&self) -> BoundDescriptor {
        self.owned_bytes
    }

    /// Checks and admits one plan's complete owned carrier charge.
    ///
    /// # Errors
    ///
    /// Returns typed checked-arithmetic or hard-bound failure.
    pub const fn admit_plan_bytes(
        &self,
        origin: CheckedByteCount,
        operations: CheckedByteCount,
        preconditions: CheckedByteCount,
    ) -> Result<CheckedByteCount, BoundError> {
        let partial = match origin.checked_add(operations) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let total = match partial.checked_add(preconditions) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match self.owned_bytes.admit_per_object(BoundValue::Bytes(total)) {
            Ok(()) => Ok(total),
            Err(error) => Err(error),
        }
    }

    /// Charges another plan against the descriptor-wide aggregate byte bound.
    ///
    /// # Errors
    ///
    /// Returns typed checked-arithmetic or aggregate exhaustion.
    pub const fn admit_retained_bytes(
        &self,
        current: CheckedByteCount,
        additional: CheckedByteCount,
    ) -> Result<CheckedByteCount, BoundError> {
        let result = self.owned_bytes.admit_aggregate(
            BoundValue::Bytes(current),
            BoundValue::Bytes(additional),
        );
        match result {
            Ok(BoundValue::Bytes(value)) => Ok(value),
            Ok(value) => Err(BoundError::UnitMismatch {
                expected: BoundUnit::Bytes,
                actual: value.unit(),
            }),
            Err(error) => Err(error),
        }
    }
}

const fn require_bound(
    field: BufferPlanBoundField,
    descriptor: BoundDescriptor,
    unit: BoundUnit,
    zero: ZeroPolicy,
    charge_owner: InstanceId,
) -> Result<(), BufferPlanBoundsError> {
    if unit_code(descriptor.unit()) != unit_code(unit) {
        return Err(BufferPlanBoundsError::WrongUnit {
            field,
            expected: unit,
            actual: descriptor.unit(),
        });
    }
    if zero_code(descriptor.zero_policy()) != zero_code(zero) {
        return Err(BufferPlanBoundsError::WrongZeroPolicy {
            field,
            expected: zero,
            actual: descriptor.zero_policy(),
        });
    }
    if descriptor.charge_owner().value() != charge_owner.value() {
        return Err(BufferPlanBoundsError::ChargeOwnerMismatch {
            field,
            expected: charge_owner,
            actual: descriptor.charge_owner(),
        });
    }
    Ok(())
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
