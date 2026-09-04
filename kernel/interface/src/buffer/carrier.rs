//! Qualified move-only carriers for Buffer Transaction plans.

use core::fmt;

use crate::bound::{
    BoundError, BoundValue, CheckedByteCount, CheckedCount, ZeroPolicy,
};
use crate::carrier::{
    BorrowModel, CarrierSafetyDescriptor, CopyModel, DropModel,
    InvocationModel, LifetimeModel, OwnershipModel, SharingModel,
    ThreadTransfer, TypeMismatch, UnsafeBoundary, UnwindModel,
};
use crate::operation::ContractSelection;

use super::bounds::BufferPlanBounds;

/// Semantic role of one Buffer-plan byte carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferCarrierRole {
    /// One self-contained origin and provenance record.
    Origin,
    /// Ordered Buffer-family Operations.
    Operations,
    /// Optional Buffer-family Preconditions.
    Preconditions,
}

/// Type identity and selected schema version stamped on one plan carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCarrierStamp {
    type_identity: u128,
    schema_version: u64,
}

impl BufferCarrierStamp {
    /// Constructs a stamp from authority-supplied scalar values.
    #[must_use]
    pub const fn new(type_identity: u128, schema_version: u64) -> Self {
        Self { type_identity, schema_version }
    }

    /// Returns the carrier-family type identity.
    #[must_use]
    pub const fn type_identity(self) -> u128 {
        self.type_identity
    }

    /// Returns the selected carrier schema version.
    #[must_use]
    pub const fn schema_version(self) -> u64 {
        self.schema_version
    }
}

/// Selected schema and safety terms for one Buffer-plan carrier role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferCarrierContract {
    schema: ContractSelection,
    safety: CarrierSafetyDescriptor,
}

impl BufferCarrierContract {
    /// Constructs one exact move-only immutable byte-carrier contract.
    ///
    /// # Errors
    ///
    /// Returns [`BufferCarrierError::IncoherentSafety`] when the declared
    /// terms differ from the public carrier's actual behavior.
    pub const fn new(
        schema: ContractSelection,
        safety: CarrierSafetyDescriptor,
    ) -> Result<Self, BufferCarrierError> {
        if !safety_is_exact(schema, safety) {
            return Err(BufferCarrierError::IncoherentSafety);
        }
        Ok(Self { schema, safety })
    }

    /// Returns the selected carrier schema and compatible range.
    #[must_use]
    pub const fn schema(&self) -> ContractSelection {
        self.schema
    }

    /// Returns the enforced carrier-safety terms.
    #[must_use]
    pub const fn safety(&self) -> CarrierSafetyDescriptor {
        self.safety
    }

    /// Returns the exact type and selected version for a new carrier.
    #[must_use]
    pub const fn selected_stamp(&self) -> BufferCarrierStamp {
        BufferCarrierStamp::new(self.schema.identity(), self.schema.selected())
    }

    /// Validates one claimed carrier type and schema version.
    ///
    /// # Errors
    ///
    /// Returns a typed type-family or incompatible-version failure.
    pub const fn validate_stamp(
        &self,
        role: BufferCarrierRole,
        stamp: BufferCarrierStamp,
    ) -> Result<(), BufferCarrierError> {
        if stamp.type_identity() != self.schema.identity() {
            return Err(BufferCarrierError::TypeMismatch { role });
        }
        if stamp.schema_version() != self.schema.selected() {
            return Err(BufferCarrierError::IncompatibleSchema { role });
        }
        Ok(())
    }
}

/// Independent origin, Operation, and Precondition carrier contracts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferCarrierContracts {
    origin: BufferCarrierContract,
    operations: BufferCarrierContract,
    preconditions: BufferCarrierContract,
}

impl BufferCarrierContracts {
    /// Groups three independently selected carrier contracts.
    ///
    /// # Errors
    ///
    /// Returns a typed duplicate when two roles reuse one type identity.
    pub fn new(
        origin: &BufferCarrierContract,
        operations: &BufferCarrierContract,
        preconditions: &BufferCarrierContract,
    ) -> Result<Self, BufferCarrierError> {
        if origin.schema().identity() == operations.schema().identity() {
            return Err(BufferCarrierError::DuplicateTypeIdentity {
                first: BufferCarrierRole::Origin,
                second: BufferCarrierRole::Operations,
            });
        }
        if origin.schema().identity() == preconditions.schema().identity() {
            return Err(BufferCarrierError::DuplicateTypeIdentity {
                first: BufferCarrierRole::Origin,
                second: BufferCarrierRole::Preconditions,
            });
        }
        if operations.schema().identity() == preconditions.schema().identity() {
            return Err(BufferCarrierError::DuplicateTypeIdentity {
                first: BufferCarrierRole::Operations,
                second: BufferCarrierRole::Preconditions,
            });
        }
        Ok(Self {
            origin: origin.clone(),
            operations: operations.clone(),
            preconditions: preconditions.clone(),
        })
    }

    /// Returns the origin/provenance carrier contract.
    #[must_use]
    pub const fn origin(&self) -> &BufferCarrierContract {
        &self.origin
    }

    /// Returns the ordered-Operation carrier contract.
    #[must_use]
    pub const fn operations(&self) -> &BufferCarrierContract {
        &self.operations
    }

    /// Returns the optional-Precondition carrier contract.
    #[must_use]
    pub const fn preconditions(&self) -> &BufferCarrierContract {
        &self.preconditions
    }

    pub(super) const fn contract(
        &self,
        role: BufferCarrierRole,
    ) -> &BufferCarrierContract {
        match role {
            BufferCarrierRole::Origin => &self.origin,
            BufferCarrierRole::Operations => &self.operations,
            BufferCarrierRole::Preconditions => &self.preconditions,
        }
    }
}

/// Typed carrier-contract, shape, arithmetic, and bound failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferCarrierError {
    /// The declared safety terms differ from actual carrier behavior.
    IncoherentSafety,
    /// Two semantic roles reused one carrier type identity.
    DuplicateTypeIdentity {
        /// First conflicting role.
        first: BufferCarrierRole,
        /// Second conflicting role.
        second: BufferCarrierRole,
    },
    /// A carrier names another type family.
    TypeMismatch {
        /// Carrier role whose type did not match.
        role: BufferCarrierRole,
    },
    /// A carrier selects another schema version.
    IncompatibleSchema {
        /// Carrier role whose version was rejected.
        role: BufferCarrierRole,
    },
    /// A checked item or byte quantity violated its declared bound.
    Bound {
        /// Carrier role governed by the bound.
        role: BufferCarrierRole,
        /// Exact Foundation bound failure.
        cause: BoundError,
    },
    /// Origin carriers must contain exactly one record.
    OriginRecordCount {
        /// Rejected record count.
        actual: u64,
    },
    /// Precondition item and byte emptiness disagree.
    PreconditionShape {
        /// Claimed Precondition count.
        items: u64,
        /// Claimed encoded byte count.
        bytes: u64,
    },
    /// A carrier was supplied in another semantic role.
    RoleMismatch {
        /// Role required by the plan field.
        expected: BufferCarrierRole,
        /// Role carried by the value.
        actual: BufferCarrierRole,
    },
    /// An allocated carrier length cannot fit the portable byte count.
    LengthOverflow {
        /// Carrier role whose allocation length overflowed.
        role: BufferCarrierRole,
    },
    /// Allocated length differs from the prevalidated byte count.
    LengthMismatch {
        /// Carrier role whose allocation length differed.
        role: BufferCarrierRole,
        /// Byte count admitted before allocation.
        prepared: u64,
        /// Actual allocated byte count when representable.
        actual: u64,
    },
}

impl fmt::Display for BufferCarrierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IncoherentSafety => {
                "Buffer-plan carrier safety terms are incoherent"
            }
            Self::DuplicateTypeIdentity { .. } => {
                "Buffer-plan carrier roles share one type identity"
            }
            Self::TypeMismatch { .. } => {
                "Buffer-plan carrier type identity differs"
            }
            Self::IncompatibleSchema { .. } => {
                "Buffer-plan carrier schema version is incompatible"
            }
            Self::Bound { .. } => "Buffer-plan carrier exceeds its bound",
            Self::OriginRecordCount { .. } => {
                "Buffer-plan origin must contain exactly one record"
            }
            Self::PreconditionShape { .. } => {
                "Buffer-plan Precondition count and bytes disagree"
            }
            Self::RoleMismatch { .. } => {
                "Buffer-plan carrier occupies another semantic role"
            }
            Self::LengthOverflow { .. } => {
                "Buffer-plan carrier length exceeds its portable count"
            }
            Self::LengthMismatch { .. } => {
                "Buffer-plan carrier length differs from its preparation"
            }
        })
    }
}

impl core::error::Error for BufferCarrierError {}

/// Pre-allocation proof for one Buffer-plan carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferCarrierPreparation {
    role: BufferCarrierRole,
    stamp: BufferCarrierStamp,
    items: CheckedCount,
    size: CheckedByteCount,
}

impl BufferCarrierPreparation {
    /// Returns the carrier's semantic role.
    #[must_use]
    pub const fn role(self) -> BufferCarrierRole {
        self.role
    }

    /// Returns the validated carrier stamp.
    #[must_use]
    pub const fn stamp(self) -> BufferCarrierStamp {
        self.stamp
    }

    /// Returns the validated encoded item count.
    #[must_use]
    pub const fn items(self) -> CheckedCount {
        self.items
    }

    /// Returns the exact byte allocation size.
    #[must_use]
    pub const fn size(self) -> CheckedByteCount {
        self.size
    }

    /// Binds an exact allocation against the governing plan descriptor.
    ///
    /// # Errors
    ///
    /// Returns the unchanged allocation when descriptor terms or length differ.
    pub fn bind(
        self,
        descriptor: &super::plan::BufferPlanDescriptor,
        bytes: ::alloc::boxed::Box<[u8]>,
    ) -> Result<BufferPlanCarrier, BufferCarrierRejection> {
        bind(
            descriptor.carriers().contract(self.role()),
            descriptor.bounds(),
            self,
            bytes,
        )
    }
}

/// Move-only encoded origin, Operation, or Precondition carrier.
pub struct BufferPlanCarrier {
    preparation: BufferCarrierPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl BufferPlanCarrier {
    /// Returns the carrier's semantic role.
    #[must_use]
    pub const fn role(&self) -> BufferCarrierRole {
        self.preparation.role()
    }

    /// Returns the qualified carrier stamp.
    #[must_use]
    pub const fn stamp(&self) -> BufferCarrierStamp {
        self.preparation.stamp()
    }

    /// Returns the encoded item count.
    #[must_use]
    pub const fn items(&self) -> CheckedCount {
        self.preparation.items()
    }

    /// Returns the exact carried byte count.
    #[must_use]
    pub const fn size(&self) -> CheckedByteCount {
        self.preparation.size()
    }

    /// Borrows the immutable encoded bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Releases the carrier and returns its exact allocation.
    #[must_use]
    pub fn into_bytes(self) -> ::alloc::boxed::Box<[u8]> {
        self.bytes
    }
}

impl fmt::Debug for BufferPlanCarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BufferPlanCarrier")
            .field("role", &self.role())
            .field("stamp", &self.stamp())
            .field("items", &self.items())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

/// Failed carrier binding that preserves the allocated bytes.
pub struct BufferCarrierRejection {
    cause: BufferCarrierError,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl BufferCarrierRejection {
    /// Returns the exact rejection cause.
    #[must_use]
    pub const fn cause(&self) -> BufferCarrierError {
        self.cause
    }

    /// Borrows the unchanged rejected allocation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns ownership of the unchanged rejected allocation.
    #[must_use]
    pub fn into_bytes(self) -> ::alloc::boxed::Box<[u8]> {
        self.bytes
    }
}

impl fmt::Debug for BufferCarrierRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BufferCarrierRejection")
            .field("cause", &self.cause)
            .field("byte_count", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

pub(super) const fn prepare(
    contract: &BufferCarrierContract,
    bounds: &BufferPlanBounds,
    role: BufferCarrierRole,
    stamp: BufferCarrierStamp,
    items: u64,
    bytes: u64,
) -> Result<BufferCarrierPreparation, BufferCarrierError> {
    match contract.validate_stamp(role, stamp) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_shape(role, items, bytes) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let items = match checked_items(role, items) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match admit_items(bounds, role, items) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let size = match checked_bytes(role, bytes) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match byte_bound(bounds, role).admit_per_object(BoundValue::Bytes(size)) {
        Ok(()) => Ok(BufferCarrierPreparation { role, stamp, items, size }),
        Err(cause) => Err(BufferCarrierError::Bound { role, cause }),
    }
}

pub(super) fn bind(
    contract: &BufferCarrierContract,
    bounds: &BufferPlanBounds,
    preparation: BufferCarrierPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
) -> Result<BufferPlanCarrier, BufferCarrierRejection> {
    if let Err(cause) = prepare(
        contract,
        bounds,
        preparation.role(),
        preparation.stamp(),
        preparation.items().value(),
        preparation.size().value(),
    ) {
        return Err(reject(cause, bytes));
    }
    match actual_length(preparation.role(), &bytes) {
        Ok(actual) if actual == preparation.size().value() => {
            Ok(BufferPlanCarrier { preparation, bytes })
        }
        Ok(actual) => Err(reject(
            BufferCarrierError::LengthMismatch {
                role: preparation.role(),
                prepared: preparation.size().value(),
                actual,
            },
            bytes,
        )),
        Err(cause) => Err(reject(cause, bytes)),
    }
}

pub(super) const fn validate_carrier(
    contract: &BufferCarrierContract,
    bounds: &BufferPlanBounds,
    expected: BufferCarrierRole,
    carrier: &BufferPlanCarrier,
) -> Result<(), BufferCarrierError> {
    if role_code(carrier.role()) != role_code(expected) {
        return Err(BufferCarrierError::RoleMismatch {
            expected,
            actual: carrier.role(),
        });
    }
    match prepare(
        contract,
        bounds,
        expected,
        carrier.stamp(),
        carrier.items().value(),
        carrier.size().value(),
    ) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}

const fn validate_shape(
    role: BufferCarrierRole,
    items: u64,
    bytes: u64,
) -> Result<(), BufferCarrierError> {
    if role_code(role) == 0 && items != 1 {
        return Err(BufferCarrierError::OriginRecordCount { actual: items });
    }
    if role_code(role) == 2 && ((items == 0) != (bytes == 0)) {
        return Err(BufferCarrierError::PreconditionShape { items, bytes });
    }
    Ok(())
}

const fn checked_items(
    role: BufferCarrierRole,
    value: u64,
) -> Result<CheckedCount, BufferCarrierError> {
    let zero = if role_code(role) == 2 {
        ZeroPolicy::Allow
    } else {
        ZeroPolicy::Reject
    };
    match CheckedCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(cause) => Err(BufferCarrierError::Bound { role, cause }),
    }
}

const fn checked_bytes(
    role: BufferCarrierRole,
    value: u64,
) -> Result<CheckedByteCount, BufferCarrierError> {
    let zero = if role_code(role) == 2 {
        ZeroPolicy::Allow
    } else {
        ZeroPolicy::Reject
    };
    match CheckedByteCount::new(value, zero) {
        Ok(value) => Ok(value),
        Err(cause) => Err(BufferCarrierError::Bound { role, cause }),
    }
}

const fn admit_items(
    bounds: &BufferPlanBounds,
    role: BufferCarrierRole,
    items: CheckedCount,
) -> Result<(), BufferCarrierError> {
    let bound = match role {
        BufferCarrierRole::Origin => return Ok(()),
        BufferCarrierRole::Operations => bounds.operations(),
        BufferCarrierRole::Preconditions => bounds.preconditions(),
    };
    match bound.admit_per_object(BoundValue::Count(items)) {
        Ok(()) => Ok(()),
        Err(cause) => Err(BufferCarrierError::Bound { role, cause }),
    }
}

const fn byte_bound(
    bounds: &BufferPlanBounds,
    role: BufferCarrierRole,
) -> crate::bound::BoundDescriptor {
    match role {
        BufferCarrierRole::Origin => bounds.origin_bytes(),
        BufferCarrierRole::Operations => bounds.operation_bytes(),
        BufferCarrierRole::Preconditions => bounds.precondition_bytes(),
    }
}

fn actual_length(
    role: BufferCarrierRole,
    bytes: &[u8],
) -> Result<u64, BufferCarrierError> {
    u64::try_from(bytes.len())
        .map_err(|_| BufferCarrierError::LengthOverflow { role })
}

const fn reject(
    cause: BufferCarrierError,
    bytes: ::alloc::boxed::Box<[u8]>,
) -> BufferCarrierRejection {
    BufferCarrierRejection { cause, bytes }
}

const fn role_code(role: BufferCarrierRole) -> u8 {
    match role {
        BufferCarrierRole::Origin => 0,
        BufferCarrierRole::Operations => 1,
        BufferCarrierRole::Preconditions => 2,
    }
}

const fn safety_is_exact(
    schema: ContractSelection,
    safety: CarrierSafetyDescriptor,
) -> bool {
    let versions = safety.schema_versions();
    safety.type_id().value() == schema.identity()
        && versions.minimum().value() == schema.minimum()
        && versions.maximum().value() == schema.maximum()
        && memory_code(safety) == 0
        && concurrency_code(safety) == 0
        && invocation_code(safety) == 0
        && mismatch_code(safety.mismatch()) == 0
        && boundary_code(safety.unsafe_boundary()) == 0
}

const fn memory_code(safety: CarrierSafetyDescriptor) -> u8 {
    let memory = safety.memory();
    match (
        memory.ownership(),
        memory.borrowing(),
        memory.copying(),
        memory.dropping(),
        memory.lifetime(),
    ) {
        (
            OwnershipModel::Owned,
            BorrowModel::Shared,
            CopyModel::MoveOnly,
            DropModel::ReleasesOwnedState,
            LifetimeModel::Value,
        ) => 0,
        _ => 1,
    }
}

const fn concurrency_code(safety: CarrierSafetyDescriptor) -> u8 {
    let concurrency = safety.concurrency();
    match (concurrency.thread_transfer(), concurrency.sharing()) {
        (ThreadTransfer::Transferable, SharingModel::Immutable) => 0,
        _ => 1,
    }
}

const fn invocation_code(safety: CarrierSafetyDescriptor) -> u8 {
    let invocation = safety.invocation();
    match (invocation.invocation(), invocation.unwind()) {
        (InvocationModel::DataOnly, UnwindModel::NotInvocable) => 0,
        _ => 1,
    }
}

const fn mismatch_code(mismatch: TypeMismatch) -> u8 {
    match mismatch {
        TypeMismatch::Reject => 0,
    }
}

const fn boundary_code(boundary: UnsafeBoundary) -> u8 {
    match boundary {
        UnsafeBoundary::None => 0,
        UnsafeBoundary::Reviewed(_) => 1,
    }
}
