//! Checked quantities and immutable bound declarations.

use core::fmt;

use crate::identity::InstanceId;

/// Whether zero is valid for one checked value or declared limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroPolicy {
    /// Zero is a meaningful value.
    Allow,
    /// Zero is invalid and produces a typed error.
    Reject,
}

/// Unit carried by a checked quantity and its bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundUnit {
    /// Number of domain-neutral items.
    Count,
    /// Number of owned or carried bytes.
    Bytes,
    /// Duration represented explicitly in nanoseconds.
    DurationNanoseconds,
}

/// Typed zero, overflow, unit, coherence, and exhaustion failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundError {
    /// Zero was rejected before traversal, allocation, or work.
    ZeroRejected { unit: BoundUnit },
    /// Machine arithmetic overflowed before a bound comparison.
    ArithmeticOverflow { unit: BoundUnit, left: u64, right: u64 },
    /// A value used a different unit from its descriptor.
    UnitMismatch { expected: BoundUnit, actual: BoundUnit },
    /// A per-object maximum exceeded its aggregate maximum.
    IncoherentDescriptor { per_object: u64, aggregate: u64 },
    /// A checked request exceeded its applicable hard maximum.
    Exhausted { unit: BoundUnit, requested: u64, maximum: u64 },
}

impl fmt::Display for BoundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroRejected { .. } => "zero is rejected by this bound",
            Self::ArithmeticOverflow { .. } => {
                "checked bound arithmetic overflowed"
            }
            Self::UnitMismatch { .. } => "bound units do not match",
            Self::IncoherentDescriptor { .. } => {
                "per-object maximum exceeds aggregate maximum"
            }
            Self::Exhausted { .. } => "requested value exceeds the bound",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for BoundError {}

/// Checked domain-neutral item count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedCount(u64);

impl CheckedCount {
    /// Constructs a count under an explicit zero policy.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ZeroRejected`] when `value` is zero and the
    /// selected policy rejects zero.
    pub const fn new(value: u64, zero: ZeroPolicy) -> Result<Self, BoundError> {
        match validate_zero(BoundUnit::Count, value, zero) {
            Ok(()) => Ok(Self(value)),
            Err(error) => Err(error),
        }
    }

    /// Returns the checked scalar count.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Adds two counts or returns typed machine overflow.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ArithmeticOverflow`] when the sum exceeds
    /// [`u64::MAX`].
    pub const fn checked_add(self, other: Self) -> Result<Self, BoundError> {
        match add(BoundUnit::Count, self.0, other.0) {
            Ok(value) => Ok(Self(value)),
            Err(error) => Err(error),
        }
    }
}

/// Checked owned or carried byte count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedByteCount(u64);

impl CheckedByteCount {
    /// Constructs a byte count under an explicit zero policy.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ZeroRejected`] when `value` is zero and the
    /// selected policy rejects zero.
    pub const fn new(value: u64, zero: ZeroPolicy) -> Result<Self, BoundError> {
        match validate_zero(BoundUnit::Bytes, value, zero) {
            Ok(()) => Ok(Self(value)),
            Err(error) => Err(error),
        }
    }

    /// Returns the checked scalar byte count.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Adds two byte counts or returns typed machine overflow.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ArithmeticOverflow`] when the sum exceeds
    /// [`u64::MAX`].
    pub const fn checked_add(self, other: Self) -> Result<Self, BoundError> {
        match add(BoundUnit::Bytes, self.0, other.0) {
            Ok(value) => Ok(Self(value)),
            Err(error) => Err(error),
        }
    }
}

/// Checked duration represented canonically in nanoseconds.
///
/// This value has no wall-time meaning and does not identify a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedDuration(u64);

impl CheckedDuration {
    /// Constructs a nanosecond duration under an explicit zero policy.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ZeroRejected`] when `nanoseconds` is zero and
    /// the selected policy rejects zero.
    pub const fn from_nanoseconds(
        nanoseconds: u64,
        zero: ZeroPolicy,
    ) -> Result<Self, BoundError> {
        let unit = BoundUnit::DurationNanoseconds;
        match validate_zero(unit, nanoseconds, zero) {
            Ok(()) => Ok(Self(nanoseconds)),
            Err(error) => Err(error),
        }
    }

    /// Returns the canonical number of nanoseconds.
    #[must_use]
    pub const fn nanoseconds(self) -> u64 {
        self.0
    }

    /// Adds two durations or returns typed machine overflow.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::ArithmeticOverflow`] when the sum exceeds
    /// [`u64::MAX`] nanoseconds.
    pub const fn checked_add(self, other: Self) -> Result<Self, BoundError> {
        let unit = BoundUnit::DurationNanoseconds;
        match add(unit, self.0, other.0) {
            Ok(value) => Ok(Self(value)),
            Err(error) => Err(error),
        }
    }
}

/// One checked scalar tagged with its exact bound unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundValue {
    /// Domain-neutral count value.
    Count(CheckedCount),
    /// Owned or carried byte value.
    Bytes(CheckedByteCount),
    /// Nanosecond duration value.
    Duration(CheckedDuration),
}

impl BoundValue {
    /// Returns the exact unit of this value.
    #[must_use]
    pub const fn unit(self) -> BoundUnit {
        match self {
            Self::Count(_) => BoundUnit::Count,
            Self::Bytes(_) => BoundUnit::Bytes,
            Self::Duration(_) => BoundUnit::DurationNanoseconds,
        }
    }

    /// Returns the checked scalar carried by this value.
    #[must_use]
    pub const fn value(self) -> u64 {
        match self {
            Self::Count(value) => value.value(),
            Self::Bytes(value) => value.value(),
            Self::Duration(value) => value.nanoseconds(),
        }
    }

    /// Adds values with equal units or returns a typed failure.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::UnitMismatch`] for different units or
    /// [`BoundError::ArithmeticOverflow`] when the sum exceeds [`u64::MAX`].
    pub const fn checked_add(self, other: Self) -> Result<Self, BoundError> {
        match (self, other) {
            (Self::Count(left), Self::Count(right)) => {
                map_count(left.checked_add(right))
            }
            (Self::Bytes(left), Self::Bytes(right)) => {
                map_bytes(left.checked_add(right))
            }
            (Self::Duration(left), Self::Duration(right)) => {
                map_duration(left.checked_add(right))
            }
            (left, right) => Err(BoundError::UnitMismatch {
                expected: left.unit(),
                actual: right.unit(),
            }),
        }
    }
}

/// Arithmetic rule applied before every bound comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithmeticRule {
    /// All arithmetic is checked and overflow is a typed failure.
    Checked,
}

/// Required order for validating a bounded request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationOrder {
    /// Validate identity, unit, arithmetic, and limit before work.
    BeforeTraversalAndAllocation,
}

/// Immutable declaration of one per-object and aggregate hard bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundDescriptor {
    per_object: BoundValue,
    aggregate: BoundValue,
    zero: ZeroPolicy,
    charge_owner: InstanceId,
}

impl BoundDescriptor {
    /// Constructs a coherent descriptor without allocating or doing work.
    ///
    /// # Errors
    ///
    /// Returns a typed zero, unit-mismatch, or incoherent-descriptor error
    /// when the declaration cannot govern one exact bounded quantity.
    pub const fn new(
        per_object: BoundValue,
        aggregate: BoundValue,
        zero: ZeroPolicy,
        charge_owner: InstanceId,
    ) -> Result<Self, BoundError> {
        if !same_unit(per_object.unit(), aggregate.unit()) {
            return Err(BoundError::UnitMismatch {
                expected: per_object.unit(),
                actual: aggregate.unit(),
            });
        }
        if per_object.value() > aggregate.value() {
            return Err(BoundError::IncoherentDescriptor {
                per_object: per_object.value(),
                aggregate: aggregate.value(),
            });
        }
        match validate_zero(per_object.unit(), per_object.value(), zero) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_zero(aggregate.unit(), aggregate.value(), zero) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self { per_object, aggregate, zero, charge_owner })
    }

    /// Returns the exact unit governed by this descriptor.
    #[must_use]
    pub const fn unit(self) -> BoundUnit {
        self.per_object.unit()
    }

    /// Returns the per-object maximum.
    #[must_use]
    pub const fn per_object(self) -> BoundValue {
        self.per_object
    }

    /// Returns the aggregate maximum.
    #[must_use]
    pub const fn aggregate(self) -> BoundValue {
        self.aggregate
    }

    /// Returns the zero-limit behavior.
    #[must_use]
    pub const fn zero_policy(self) -> ZeroPolicy {
        self.zero
    }

    /// Returns the one owner of accounting for this bound.
    #[must_use]
    pub const fn charge_owner(self) -> InstanceId {
        self.charge_owner
    }

    /// Returns the structurally fixed checked-arithmetic rule.
    #[must_use]
    pub const fn arithmetic_rule(self) -> ArithmeticRule {
        ArithmeticRule::Checked
    }

    /// Returns the structurally fixed pre-work validation rule.
    #[must_use]
    pub const fn validation_order(self) -> ValidationOrder {
        ValidationOrder::BeforeTraversalAndAllocation
    }

    /// Validates one object against its exact maximum before work.
    ///
    /// # Errors
    ///
    /// Returns [`BoundError::UnitMismatch`] for another unit or
    /// [`BoundError::Exhausted`] when `requested` exceeds the hard maximum.
    pub const fn admit_per_object(
        self,
        requested: BoundValue,
    ) -> Result<(), BoundError> {
        admit(self.per_object, requested, self.zero)
    }

    /// Checks an aggregate addition, then validates the resulting charge.
    ///
    /// # Errors
    ///
    /// Returns a typed unit, arithmetic-overflow, or exhaustion error before
    /// any caller performs traversal, allocation, or other work.
    pub const fn admit_aggregate(
        self,
        current: BoundValue,
        additional: BoundValue,
    ) -> Result<BoundValue, BoundError> {
        match require_unit(self.aggregate, current) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match require_unit(self.aggregate, additional) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_zero(additional.unit(), additional.value(), self.zero) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let total = match current.checked_add(additional) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        match admit(self.aggregate, total, self.zero) {
            Ok(()) => Ok(total),
            Err(error) => Err(error),
        }
    }
}

const fn validate_zero(
    unit: BoundUnit,
    value: u64,
    policy: ZeroPolicy,
) -> Result<(), BoundError> {
    if value == 0 && matches_zero_reject(policy) {
        return Err(BoundError::ZeroRejected { unit });
    }
    Ok(())
}

const fn matches_zero_reject(policy: ZeroPolicy) -> bool {
    match policy {
        ZeroPolicy::Allow => false,
        ZeroPolicy::Reject => true,
    }
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

const fn add(
    unit: BoundUnit,
    left: u64,
    right: u64,
) -> Result<u64, BoundError> {
    match left.checked_add(right) {
        Some(value) => Ok(value),
        None => Err(BoundError::ArithmeticOverflow { unit, left, right }),
    }
}

const fn map_count(
    result: Result<CheckedCount, BoundError>,
) -> Result<BoundValue, BoundError> {
    match result {
        Ok(value) => Ok(BoundValue::Count(value)),
        Err(error) => Err(error),
    }
}

const fn map_bytes(
    result: Result<CheckedByteCount, BoundError>,
) -> Result<BoundValue, BoundError> {
    match result {
        Ok(value) => Ok(BoundValue::Bytes(value)),
        Err(error) => Err(error),
    }
}

const fn map_duration(
    result: Result<CheckedDuration, BoundError>,
) -> Result<BoundValue, BoundError> {
    match result {
        Ok(value) => Ok(BoundValue::Duration(value)),
        Err(error) => Err(error),
    }
}

const fn admit(
    maximum: BoundValue,
    requested: BoundValue,
    zero: ZeroPolicy,
) -> Result<(), BoundError> {
    match require_unit(maximum, requested) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_zero(requested.unit(), requested.value(), zero) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    if requested.value() > maximum.value() {
        return Err(BoundError::Exhausted {
            unit: maximum.unit(),
            requested: requested.value(),
            maximum: maximum.value(),
        });
    }
    Ok(())
}

const fn require_unit(
    expected: BoundValue,
    actual: BoundValue,
) -> Result<(), BoundError> {
    if same_unit(expected.unit(), actual.unit()) {
        return Ok(());
    }
    Err(BoundError::UnitMismatch {
        expected: expected.unit(),
        actual: actual.unit(),
    })
}
