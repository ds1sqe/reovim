//! Hard bounds for semantic Intent and policy calls.

use core::fmt;

use crate::bound::{
    BoundDescriptor, BoundError, BoundUnit, BoundValue, CheckedByteCount,
    CheckedCount, ZeroPolicy,
};
use crate::identity::InstanceId;

/// Named semantic quantity governed by one exact bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticBoundField {
    /// Claimed structural items in one semantic Intent.
    IntentItems,
    /// Claimed maximum nesting depth in one semantic Intent.
    IntentDepth,
    /// Encoded bytes owned by one semantic Intent.
    IntentBytes,
    /// Bounded abstract work units reserved for one policy call.
    PolicyWork,
    /// Temporary bytes reserved for one policy call.
    TemporaryBytes,
    /// Result bytes reserved for one policy call.
    ResultBytes,
    /// Concurrent calls admitted for one policy instance.
    ConcurrentCalls,
}

/// Typed construction failure for semantic bound sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticBoundsError {
    /// A named bound uses another unit.
    WrongUnit {
        /// Rejected semantic quantity.
        field: SemanticBoundField,
        /// Unit required by the contract.
        expected: BoundUnit,
        /// Unit supplied by the descriptor.
        actual: BoundUnit,
    },
    /// A named bound uses another zero policy.
    WrongZeroPolicy {
        /// Rejected semantic quantity.
        field: SemanticBoundField,
        /// Zero policy required by the contract.
        expected: ZeroPolicy,
        /// Zero policy supplied by the descriptor.
        actual: ZeroPolicy,
    },
    /// A named bound belongs to another accounting owner.
    ChargeOwnerMismatch {
        /// Rejected semantic quantity.
        field: SemanticBoundField,
        /// Charge owner selected for the complete bound set.
        expected: InstanceId,
        /// Charge owner supplied by the descriptor.
        actual: InstanceId,
    },
}

impl fmt::Display for SemanticBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WrongUnit { .. } => "semantic bound uses the wrong unit",
            Self::WrongZeroPolicy { .. } => {
                "semantic bound uses the wrong zero policy"
            }
            Self::ChargeOwnerMismatch { .. } => {
                "semantic bound uses another charge owner"
            }
        })
    }
}

impl core::error::Error for SemanticBoundsError {}

/// Typed request-shape or policy-charge bound failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticBoundViolation {
    /// One checked value exceeded its field's declared rule or limit.
    Bound {
        /// Rejected semantic quantity.
        field: SemanticBoundField,
        /// Exact Foundation bound failure.
        cause: BoundError,
    },
    /// A prepared call charge belongs to another accounting owner.
    ChargeOwnerMismatch {
        /// Owner required by the selected call bounds.
        expected: InstanceId,
        /// Owner carried by the supplied charge.
        actual: InstanceId,
    },
}

impl SemanticBoundViolation {
    const fn new(field: SemanticBoundField, cause: BoundError) -> Self {
        Self::Bound { field, cause }
    }

    /// Returns the semantic quantity that failed, when field-specific.
    #[must_use]
    pub const fn field(self) -> Option<SemanticBoundField> {
        match self {
            Self::Bound { field, .. } => Some(field),
            Self::ChargeOwnerMismatch { .. } => None,
        }
    }

    /// Returns the exact checked-bound failure.
    #[must_use]
    pub const fn cause(self) -> Option<BoundError> {
        match self {
            Self::Bound { cause, .. } => Some(cause),
            Self::ChargeOwnerMismatch { .. } => None,
        }
    }
}

impl fmt::Display for SemanticBoundViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bound { .. } => "semantic request exceeds a declared bound",
            Self::ChargeOwnerMismatch { .. } => {
                "semantic call charge belongs to another owner"
            }
        })
    }
}

impl core::error::Error for SemanticBoundViolation {}

/// Checked structural shape of one semantic Intent carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticIntentShape {
    items: CheckedCount,
    depth: CheckedCount,
    bytes: CheckedByteCount,
}

impl SemanticIntentShape {
    /// Returns the claimed structural item count.
    #[must_use]
    pub const fn items(self) -> CheckedCount {
        self.items
    }

    /// Returns the claimed maximum nesting depth.
    #[must_use]
    pub const fn depth(self) -> CheckedCount {
        self.depth
    }

    /// Returns the exact encoded byte count.
    #[must_use]
    pub const fn bytes(self) -> CheckedByteCount {
        self.bytes
    }
}

/// Item, nesting, and byte bounds for one semantic Intent family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticIntentBounds {
    charge_owner: InstanceId,
    items: BoundDescriptor,
    depth: BoundDescriptor,
    bytes: BoundDescriptor,
}

impl SemanticIntentBounds {
    /// Constructs a coherent semantic Intent bound set.
    ///
    /// # Errors
    ///
    /// Returns a typed unit, zero-policy, or charge-owner mismatch.
    pub const fn new(
        charge_owner: InstanceId,
        items: BoundDescriptor,
        depth: BoundDescriptor,
        bytes: BoundDescriptor,
    ) -> Result<Self, SemanticBoundsError> {
        match require_bound(
            SemanticBoundField::IntentItems,
            items,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            SemanticBoundField::IntentDepth,
            depth,
            BoundUnit::Count,
            ZeroPolicy::Allow,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            SemanticBoundField::IntentBytes,
            bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self { charge_owner, items, depth, bytes })
    }

    /// Returns the shared accounting owner.
    #[must_use]
    pub const fn charge_owner(&self) -> InstanceId {
        self.charge_owner
    }

    /// Returns the semantic item-count bound.
    #[must_use]
    pub const fn items(&self) -> BoundDescriptor {
        self.items
    }

    /// Returns the semantic nesting-depth bound.
    #[must_use]
    pub const fn depth(&self) -> BoundDescriptor {
        self.depth
    }

    /// Returns the encoded Intent byte bound and aggregate retention scope.
    #[must_use]
    pub const fn bytes(&self) -> BoundDescriptor {
        self.bytes
    }

    /// Validates one claimed Intent shape before allocation or traversal.
    ///
    /// # Errors
    ///
    /// Returns the first typed zero, unit, arithmetic, or hard-bound failure.
    pub const fn prepare(
        &self,
        items: u64,
        depth: u64,
        bytes: u64,
    ) -> Result<SemanticIntentShape, SemanticBoundViolation> {
        let items = match checked_count(
            SemanticBoundField::IntentItems,
            items,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let depth = match checked_count(
            SemanticBoundField::IntentDepth,
            depth,
            ZeroPolicy::Allow,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let bytes = match checked_bytes(
            SemanticBoundField::IntentBytes,
            bytes,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match admit(
            SemanticBoundField::IntentItems,
            self.items,
            BoundValue::Count(items),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            SemanticBoundField::IntentDepth,
            self.depth,
            BoundValue::Count(depth),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            SemanticBoundField::IntentBytes,
            self.bytes,
            BoundValue::Bytes(bytes),
        ) {
            Ok(()) => Ok(SemanticIntentShape { items, depth, bytes }),
            Err(error) => Err(error),
        }
    }

    /// Charges another retained Intent against aggregate encoded bytes.
    ///
    /// # Errors
    ///
    /// Returns checked arithmetic or aggregate byte exhaustion.
    pub const fn admit_retained_bytes(
        &self,
        current: CheckedByteCount,
        additional: CheckedByteCount,
    ) -> Result<CheckedByteCount, SemanticBoundViolation> {
        let result = self.bytes.admit_aggregate(
            BoundValue::Bytes(current),
            BoundValue::Bytes(additional),
        );
        match result {
            Ok(BoundValue::Bytes(value)) => Ok(value),
            Ok(value) => Err(SemanticBoundViolation::new(
                SemanticBoundField::IntentBytes,
                BoundError::UnitMismatch {
                    expected: BoundUnit::Bytes,
                    actual: value.unit(),
                },
            )),
            Err(cause) => Err(SemanticBoundViolation::new(
                SemanticBoundField::IntentBytes,
                cause,
            )),
        }
    }
}

/// Checked reservation for one semantic policy callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticPolicyCallCharge {
    charge_owner: InstanceId,
    work: CheckedCount,
    temporary_bytes: CheckedByteCount,
    result_bytes: CheckedByteCount,
    concurrent_calls: CheckedCount,
}

impl SemanticPolicyCallCharge {
    /// Returns the accounting owner of the reservation.
    #[must_use]
    pub const fn charge_owner(self) -> InstanceId {
        self.charge_owner
    }

    /// Returns reserved abstract work units.
    #[must_use]
    pub const fn work(self) -> CheckedCount {
        self.work
    }

    /// Returns reserved temporary bytes.
    #[must_use]
    pub const fn temporary_bytes(self) -> CheckedByteCount {
        self.temporary_bytes
    }

    /// Returns reserved result bytes.
    #[must_use]
    pub const fn result_bytes(self) -> CheckedByteCount {
        self.result_bytes
    }

    /// Returns reserved concurrent-call capacity.
    #[must_use]
    pub const fn concurrent_calls(self) -> CheckedCount {
        self.concurrent_calls
    }
}

/// Work, memory, result, and concurrency bounds for one policy call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticPolicyCallBounds {
    charge_owner: InstanceId,
    work: BoundDescriptor,
    temporary_bytes: BoundDescriptor,
    result_bytes: BoundDescriptor,
    concurrent_calls: BoundDescriptor,
}

impl SemanticPolicyCallBounds {
    /// Constructs a coherent semantic policy call bound set.
    ///
    /// # Errors
    ///
    /// Returns a typed unit, zero-policy, or charge-owner mismatch.
    pub const fn new(
        charge_owner: InstanceId,
        work: BoundDescriptor,
        temporary_bytes: BoundDescriptor,
        result_bytes: BoundDescriptor,
        concurrent_calls: BoundDescriptor,
    ) -> Result<Self, SemanticBoundsError> {
        match require_bound(
            SemanticBoundField::PolicyWork,
            work,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            SemanticBoundField::TemporaryBytes,
            temporary_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Allow,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            SemanticBoundField::ResultBytes,
            result_bytes,
            BoundUnit::Bytes,
            ZeroPolicy::Allow,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_bound(
            SemanticBoundField::ConcurrentCalls,
            concurrent_calls,
            BoundUnit::Count,
            ZeroPolicy::Reject,
            charge_owner,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self {
            charge_owner,
            work,
            temporary_bytes,
            result_bytes,
            concurrent_calls,
        })
    }

    /// Returns the shared accounting owner.
    #[must_use]
    pub const fn charge_owner(&self) -> InstanceId {
        self.charge_owner
    }

    /// Returns the abstract work-unit bound.
    #[must_use]
    pub const fn work(&self) -> BoundDescriptor {
        self.work
    }

    /// Returns the temporary-byte bound.
    #[must_use]
    pub const fn temporary_bytes(&self) -> BoundDescriptor {
        self.temporary_bytes
    }

    /// Returns the result-byte bound.
    #[must_use]
    pub const fn result_bytes(&self) -> BoundDescriptor {
        self.result_bytes
    }

    /// Returns the concurrent-call bound.
    #[must_use]
    pub const fn concurrent_calls(&self) -> BoundDescriptor {
        self.concurrent_calls
    }

    /// Validates one complete policy call reservation.
    ///
    /// # Errors
    ///
    /// Returns the first typed zero, unit, arithmetic, or hard-bound failure.
    pub const fn prepare(
        &self,
        work: u64,
        temporary_bytes: u64,
        result_bytes: u64,
        concurrent_calls: u64,
    ) -> Result<SemanticPolicyCallCharge, SemanticBoundViolation> {
        let work = match checked_count(
            SemanticBoundField::PolicyWork,
            work,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let temporary_bytes = match checked_bytes(
            SemanticBoundField::TemporaryBytes,
            temporary_bytes,
            ZeroPolicy::Allow,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let result_bytes = match checked_bytes(
            SemanticBoundField::ResultBytes,
            result_bytes,
            ZeroPolicy::Allow,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let concurrent_calls = match checked_count(
            SemanticBoundField::ConcurrentCalls,
            concurrent_calls,
            ZeroPolicy::Reject,
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let charge = SemanticPolicyCallCharge {
            charge_owner: self.charge_owner,
            work,
            temporary_bytes,
            result_bytes,
            concurrent_calls,
        };
        match self.validate_charge(charge) {
            Ok(()) => Ok(charge),
            Err(error) => Err(error),
        }
    }

    /// Revalidates a reservation against this exact bound set.
    ///
    /// # Errors
    ///
    /// Returns owner mismatch or the first hard-bound failure.
    pub const fn validate_charge(
        &self,
        charge: SemanticPolicyCallCharge,
    ) -> Result<(), SemanticBoundViolation> {
        if charge.charge_owner().value() != self.charge_owner.value() {
            return Err(SemanticBoundViolation::ChargeOwnerMismatch {
                expected: self.charge_owner,
                actual: charge.charge_owner(),
            });
        }
        match admit(
            SemanticBoundField::PolicyWork,
            self.work,
            BoundValue::Count(charge.work()),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            SemanticBoundField::TemporaryBytes,
            self.temporary_bytes,
            BoundValue::Bytes(charge.temporary_bytes()),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match admit(
            SemanticBoundField::ResultBytes,
            self.result_bytes,
            BoundValue::Bytes(charge.result_bytes()),
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        admit(
            SemanticBoundField::ConcurrentCalls,
            self.concurrent_calls,
            BoundValue::Count(charge.concurrent_calls()),
        )
    }
}

const fn require_bound(
    field: SemanticBoundField,
    descriptor: BoundDescriptor,
    unit: BoundUnit,
    zero: ZeroPolicy,
    charge_owner: InstanceId,
) -> Result<(), SemanticBoundsError> {
    if unit_code(descriptor.unit()) != unit_code(unit) {
        return Err(SemanticBoundsError::WrongUnit {
            field,
            expected: unit,
            actual: descriptor.unit(),
        });
    }
    if zero_code(descriptor.zero_policy()) != zero_code(zero) {
        return Err(SemanticBoundsError::WrongZeroPolicy {
            field,
            expected: zero,
            actual: descriptor.zero_policy(),
        });
    }
    if descriptor.charge_owner().value() != charge_owner.value() {
        return Err(SemanticBoundsError::ChargeOwnerMismatch {
            field,
            expected: charge_owner,
            actual: descriptor.charge_owner(),
        });
    }
    Ok(())
}

const fn checked_count(
    field: SemanticBoundField,
    value: u64,
    zero: ZeroPolicy,
) -> Result<CheckedCount, SemanticBoundViolation> {
    match CheckedCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(cause) => Err(SemanticBoundViolation::new(field, cause)),
    }
}

const fn checked_bytes(
    field: SemanticBoundField,
    value: u64,
    zero: ZeroPolicy,
) -> Result<CheckedByteCount, SemanticBoundViolation> {
    match CheckedByteCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(cause) => Err(SemanticBoundViolation::new(field, cause)),
    }
}

const fn admit(
    field: SemanticBoundField,
    descriptor: BoundDescriptor,
    value: BoundValue,
) -> Result<(), SemanticBoundViolation> {
    match descriptor.admit_per_object(value) {
        Ok(()) => Ok(()),
        Err(cause) => Err(SemanticBoundViolation::new(field, cause)),
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
