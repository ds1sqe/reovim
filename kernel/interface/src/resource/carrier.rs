//! Qualified move-only carriers for Resource-native regions and payloads.

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

use super::bounds::ResourceTransferBounds;

/// Semantic role of one Resource-owned byte carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCarrierRole {
    /// Encoded Resource-native Address, Extent, and Unit regions.
    Regions,
    /// Encoded Resource-native payload corresponding to requested regions.
    Payload,
}

/// Type identity and selected schema version stamped on one carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceCarrierStamp {
    type_identity: u128,
    schema_version: u64,
}

impl ResourceCarrierStamp {
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

/// Selected schema and enforced safety terms for one carrier role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceCarrierContract {
    schema: ContractSelection,
    safety: CarrierSafetyDescriptor,
}

impl ResourceCarrierContract {
    /// Constructs one exact move-only immutable byte-carrier contract.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceCarrierError::IncoherentSafety`] when any safety
    /// term differs from the public carrier's actual behavior.
    pub const fn new(
        schema: ContractSelection,
        safety: CarrierSafetyDescriptor,
    ) -> Result<Self, ResourceCarrierError> {
        if !safety_is_exact(schema, safety) {
            return Err(ResourceCarrierError::IncoherentSafety);
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
    pub const fn selected_stamp(&self) -> ResourceCarrierStamp {
        ResourceCarrierStamp::new(
            self.schema.identity(),
            self.schema.selected(),
        )
    }

    /// Validates one claimed carrier type and schema version.
    ///
    /// # Errors
    ///
    /// Returns a typed type-family or incompatible-version failure.
    pub const fn validate_stamp(
        &self,
        role: ResourceCarrierRole,
        stamp: ResourceCarrierStamp,
    ) -> Result<(), ResourceCarrierError> {
        if stamp.type_identity() != self.schema.identity() {
            return Err(ResourceCarrierError::TypeMismatch { role });
        }
        if stamp.schema_version() != self.schema.selected() {
            return Err(ResourceCarrierError::IncompatibleSchema { role });
        }
        Ok(())
    }
}

/// Independent region-request and Snapshot-payload carrier contracts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceCarrierContracts {
    regions: ResourceCarrierContract,
    payload: ResourceCarrierContract,
}

impl ResourceCarrierContracts {
    /// Groups two independently selected carrier contracts.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceCarrierError::DuplicateTypeIdentity`] when both
    /// semantic roles use one type identity.
    pub fn new(
        regions: &ResourceCarrierContract,
        payload: &ResourceCarrierContract,
    ) -> Result<Self, ResourceCarrierError> {
        if regions.schema().identity() == payload.schema().identity() {
            return Err(ResourceCarrierError::DuplicateTypeIdentity);
        }
        Ok(Self { regions: regions.clone(), payload: payload.clone() })
    }

    /// Returns the encoded region-set carrier contract.
    #[must_use]
    pub const fn regions(&self) -> &ResourceCarrierContract {
        &self.regions
    }

    /// Returns the Snapshot payload carrier contract.
    #[must_use]
    pub const fn payload(&self) -> &ResourceCarrierContract {
        &self.payload
    }
}

/// Typed carrier-contract, schema, arithmetic, and bound failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceCarrierError {
    /// The declared safety terms differ from actual carrier behavior.
    IncoherentSafety,
    /// Region and payload roles reused one carrier type identity.
    DuplicateTypeIdentity,
    /// A carrier names another type family.
    TypeMismatch {
        /// Carrier role whose type did not match.
        role: ResourceCarrierRole,
    },
    /// A carrier selects a schema version outside the compatible range.
    IncompatibleSchema {
        /// Carrier role whose schema version was rejected.
        role: ResourceCarrierRole,
    },
    /// A checked count or byte quantity violated its declared bound.
    Bound {
        /// Carrier role governed by the bound.
        role: ResourceCarrierRole,
        /// Exact Foundation bound failure.
        cause: BoundError,
    },
    /// An allocated carrier length cannot fit the portable byte count.
    LengthOverflow {
        /// Carrier role whose allocation length overflowed.
        role: ResourceCarrierRole,
    },
    /// Allocated length differs from the prevalidated byte count.
    LengthMismatch {
        /// Carrier role whose allocation length differed.
        role: ResourceCarrierRole,
        /// Byte count admitted before allocation.
        prepared: u64,
        /// Actual allocated byte count when representable.
        actual: u64,
    },
}

impl fmt::Display for ResourceCarrierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IncoherentSafety => {
                "Resource carrier safety terms are incoherent"
            }
            Self::DuplicateTypeIdentity => {
                "Resource carrier roles share one type identity"
            }
            Self::TypeMismatch { .. } => {
                "Resource carrier type identity differs"
            }
            Self::IncompatibleSchema { .. } => {
                "Resource carrier schema version is incompatible"
            }
            Self::Bound { .. } => "Resource carrier exceeds its bound",
            Self::LengthOverflow { .. } => {
                "Resource carrier length exceeds its portable count"
            }
            Self::LengthMismatch { .. } => {
                "Resource carrier length differs from its preparation"
            }
        })
    }
}

impl core::error::Error for ResourceCarrierError {}

/// Pre-allocation proof for one encoded native region set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceRegionPreparation {
    stamp: ResourceCarrierStamp,
    regions: CheckedCount,
    size: CheckedByteCount,
}

impl ResourceRegionPreparation {
    /// Returns the validated carrier stamp.
    #[must_use]
    pub const fn stamp(self) -> ResourceCarrierStamp {
        self.stamp
    }

    /// Returns the validated native-region count.
    #[must_use]
    pub const fn regions(self) -> CheckedCount {
        self.regions
    }

    /// Returns the exact byte allocation size.
    #[must_use]
    pub const fn size(self) -> CheckedByteCount {
        self.size
    }
}

/// Pre-allocation proof for one encoded Snapshot payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourcePayloadPreparation {
    stamp: ResourceCarrierStamp,
    regions: CheckedCount,
    size: CheckedByteCount,
}

impl ResourcePayloadPreparation {
    /// Returns the validated carrier stamp.
    #[must_use]
    pub const fn stamp(self) -> ResourceCarrierStamp {
        self.stamp
    }

    /// Returns the native-region count represented by the payload.
    #[must_use]
    pub const fn regions(self) -> CheckedCount {
        self.regions
    }

    /// Returns the exact byte allocation size.
    #[must_use]
    pub const fn size(self) -> CheckedByteCount {
        self.size
    }
}

/// Move-only encoded Resource-native Address, Extent, and Unit regions.
pub struct ResourceRegionCarrier {
    preparation: ResourceRegionPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl ResourceRegionCarrier {
    /// Returns the qualified carrier stamp.
    #[must_use]
    pub const fn stamp(&self) -> ResourceCarrierStamp {
        self.preparation.stamp()
    }

    /// Returns the encoded Resource-native region count.
    #[must_use]
    pub const fn regions(&self) -> CheckedCount {
        self.preparation.regions()
    }

    /// Returns the exact carried byte count.
    #[must_use]
    pub const fn size(&self) -> CheckedByteCount {
        self.preparation.size()
    }

    /// Borrows the immutable encoded region bytes.
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

impl fmt::Debug for ResourceRegionCarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceRegionCarrier")
            .field("stamp", &self.stamp())
            .field("regions", &self.regions())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

/// Move-only encoded Resource-native Snapshot payload.
pub struct ResourcePayloadCarrier {
    preparation: ResourcePayloadPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl ResourcePayloadCarrier {
    /// Returns the qualified carrier stamp.
    #[must_use]
    pub const fn stamp(&self) -> ResourceCarrierStamp {
        self.preparation.stamp()
    }

    /// Returns the native-region count represented by this payload.
    #[must_use]
    pub const fn regions(&self) -> CheckedCount {
        self.preparation.regions()
    }

    /// Returns the exact carried byte count.
    #[must_use]
    pub const fn size(&self) -> CheckedByteCount {
        self.preparation.size()
    }

    /// Borrows the immutable encoded payload bytes.
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

impl fmt::Debug for ResourcePayloadCarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourcePayloadCarrier")
            .field("stamp", &self.stamp())
            .field("regions", &self.regions())
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

/// Failed binding that preserves ownership of the allocated bytes.
pub struct ResourceCarrierRejection {
    cause: ResourceCarrierError,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl ResourceCarrierRejection {
    /// Returns the exact rejection cause.
    #[must_use]
    pub const fn cause(&self) -> ResourceCarrierError {
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

impl fmt::Debug for ResourceCarrierRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceCarrierRejection")
            .field("cause", &self.cause)
            .field("byte_count", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

pub(super) const fn prepare_regions(
    contract: &ResourceCarrierContract,
    bounds: &ResourceTransferBounds,
    stamp: ResourceCarrierStamp,
    regions: u64,
    bytes: u64,
) -> Result<ResourceRegionPreparation, ResourceCarrierError> {
    match contract.validate_stamp(ResourceCarrierRole::Regions, stamp) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let regions = match checked_count(ResourceCarrierRole::Regions, regions) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match admit_count(ResourceCarrierRole::Regions, bounds.regions(), regions) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let size = match checked_bytes(ResourceCarrierRole::Regions, bytes) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match admit_bytes(
        ResourceCarrierRole::Regions,
        bounds.carrier_bytes(),
        size,
    ) {
        Ok(()) => Ok(ResourceRegionPreparation { stamp, regions, size }),
        Err(error) => Err(error),
    }
}

pub(super) const fn prepare_payload(
    contract: &ResourceCarrierContract,
    bounds: &ResourceTransferBounds,
    stamp: ResourceCarrierStamp,
    regions: u64,
    bytes: u64,
) -> Result<ResourcePayloadPreparation, ResourceCarrierError> {
    match contract.validate_stamp(ResourceCarrierRole::Payload, stamp) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let regions = match checked_count(ResourceCarrierRole::Payload, regions) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match admit_count(ResourceCarrierRole::Payload, bounds.regions(), regions) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let size = match checked_bytes(ResourceCarrierRole::Payload, bytes) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    match admit_bytes(
        ResourceCarrierRole::Payload,
        bounds.payload_bytes(),
        size,
    ) {
        Ok(()) => Ok(ResourcePayloadPreparation { stamp, regions, size }),
        Err(error) => Err(error),
    }
}

pub(super) fn bind_regions(
    contract: &ResourceCarrierContract,
    bounds: &ResourceTransferBounds,
    preparation: ResourceRegionPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
) -> Result<ResourceRegionCarrier, ResourceCarrierRejection> {
    if let Err(cause) = prepare_regions(
        contract,
        bounds,
        preparation.stamp(),
        preparation.regions().value(),
        preparation.size().value(),
    ) {
        return Err(reject(cause, bytes));
    }
    match actual_length(ResourceCarrierRole::Regions, &bytes) {
        Ok(actual) if actual == preparation.size().value() => {
            Ok(ResourceRegionCarrier { preparation, bytes })
        }
        Ok(actual) => Err(reject(
            ResourceCarrierError::LengthMismatch {
                role: ResourceCarrierRole::Regions,
                prepared: preparation.size().value(),
                actual,
            },
            bytes,
        )),
        Err(cause) => Err(reject(cause, bytes)),
    }
}

pub(super) fn bind_payload(
    contract: &ResourceCarrierContract,
    bounds: &ResourceTransferBounds,
    preparation: ResourcePayloadPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
) -> Result<ResourcePayloadCarrier, ResourceCarrierRejection> {
    if let Err(cause) = prepare_payload(
        contract,
        bounds,
        preparation.stamp(),
        preparation.regions().value(),
        preparation.size().value(),
    ) {
        return Err(reject(cause, bytes));
    }
    match actual_length(ResourceCarrierRole::Payload, &bytes) {
        Ok(actual) if actual == preparation.size().value() => {
            Ok(ResourcePayloadCarrier { preparation, bytes })
        }
        Ok(actual) => Err(reject(
            ResourceCarrierError::LengthMismatch {
                role: ResourceCarrierRole::Payload,
                prepared: preparation.size().value(),
                actual,
            },
            bytes,
        )),
        Err(cause) => Err(reject(cause, bytes)),
    }
}

const fn checked_count(
    role: ResourceCarrierRole,
    value: u64,
) -> Result<CheckedCount, ResourceCarrierError> {
    match CheckedCount::new(value, ZeroPolicy::Allow) {
        Ok(value) => Ok(value),
        Err(cause) => Err(ResourceCarrierError::Bound { role, cause }),
    }
}

const fn checked_bytes(
    role: ResourceCarrierRole,
    value: u64,
) -> Result<CheckedByteCount, ResourceCarrierError> {
    match CheckedByteCount::new(value, ZeroPolicy::Allow) {
        Ok(value) => Ok(value),
        Err(cause) => Err(ResourceCarrierError::Bound { role, cause }),
    }
}

const fn admit_count(
    role: ResourceCarrierRole,
    bound: crate::bound::BoundDescriptor,
    count: CheckedCount,
) -> Result<(), ResourceCarrierError> {
    match bound.admit_per_object(BoundValue::Count(count)) {
        Ok(()) => Ok(()),
        Err(cause) => Err(ResourceCarrierError::Bound { role, cause }),
    }
}

const fn admit_bytes(
    role: ResourceCarrierRole,
    bound: crate::bound::BoundDescriptor,
    size: CheckedByteCount,
) -> Result<(), ResourceCarrierError> {
    match bound.admit_per_object(BoundValue::Bytes(size)) {
        Ok(()) => Ok(()),
        Err(cause) => Err(ResourceCarrierError::Bound { role, cause }),
    }
}

fn actual_length(
    role: ResourceCarrierRole,
    bytes: &[u8],
) -> Result<u64, ResourceCarrierError> {
    u64::try_from(bytes.len())
        .map_err(|_| ResourceCarrierError::LengthOverflow { role })
}

const fn reject(
    cause: ResourceCarrierError,
    bytes: ::alloc::boxed::Box<[u8]>,
) -> ResourceCarrierRejection {
    ResourceCarrierRejection { cause, bytes }
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
