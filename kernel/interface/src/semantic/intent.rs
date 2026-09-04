//! Qualified move-only semantic Intent carrier.

use core::fmt;

use crate::carrier::{
    BorrowModel, CarrierSafetyDescriptor, CopyModel, DropModel,
    InvocationModel, LifetimeModel, OwnershipModel, SharingModel,
    ThreadTransfer, TypeMismatch, UnsafeBoundary, UnwindModel,
};
use crate::operation::ContractSelection;

use super::bounds::{SemanticBoundViolation, SemanticIntentShape};
use super::descriptor::RepresentationPolicyDescriptor;
use super::stamp::SemanticIntentStamp;

/// Type identity and selected schema version of a semantic Intent carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticIntentCarrierStamp {
    type_identity: u128,
    schema_version: u64,
}

impl SemanticIntentCarrierStamp {
    /// Constructs a carrier stamp from authority-supplied scalars.
    #[must_use]
    pub const fn new(type_identity: u128, schema_version: u64) -> Self {
        Self { type_identity, schema_version }
    }

    /// Returns the semantic Intent carrier-family identity.
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

/// Selected schema and exact safety terms of one semantic Intent family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticIntentContract {
    schema: ContractSelection,
    safety: CarrierSafetyDescriptor,
}

impl SemanticIntentContract {
    /// Constructs the exact owned immutable semantic Intent contract.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticIntentError::IncoherentSafety`] when declared terms
    /// differ from the public carrier's actual behavior.
    pub const fn new(
        schema: ContractSelection,
        safety: CarrierSafetyDescriptor,
    ) -> Result<Self, SemanticIntentError> {
        if !safety_is_exact(schema, safety) {
            return Err(SemanticIntentError::IncoherentSafety);
        }
        Ok(Self { schema, safety })
    }

    /// Returns the selected carrier schema and compatible range.
    #[must_use]
    pub const fn schema(&self) -> ContractSelection {
        self.schema
    }

    /// Returns the enforced carrier safety terms.
    #[must_use]
    pub const fn safety(&self) -> CarrierSafetyDescriptor {
        self.safety
    }

    /// Returns the selected type and schema version for a new Intent.
    #[must_use]
    pub const fn selected_stamp(&self) -> SemanticIntentCarrierStamp {
        SemanticIntentCarrierStamp::new(
            self.schema.identity(),
            self.schema.selected(),
        )
    }

    /// Validates a claimed semantic Intent carrier stamp.
    ///
    /// # Errors
    ///
    /// Returns a typed carrier-family or selected-version mismatch.
    pub const fn validate_stamp(
        &self,
        stamp: SemanticIntentCarrierStamp,
    ) -> Result<(), SemanticIntentError> {
        if stamp.type_identity() != self.schema.identity() {
            return Err(SemanticIntentError::TypeMismatch);
        }
        if stamp.schema_version() != self.schema.selected() {
            return Err(SemanticIntentError::IncompatibleSchema);
        }
        Ok(())
    }
}

/// Typed semantic Intent contract, stamp, shape, bound, or length failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticIntentError {
    /// Declared carrier safety differs from actual carrier behavior.
    IncoherentSafety,
    /// The carrier names another semantic Intent family.
    TypeMismatch,
    /// The carrier selects another semantic Intent schema version.
    IncompatibleSchema,
    /// The outer Intent names another Representation policy generation.
    RepresentationGenerationMismatch,
    /// The outer Intent names another coordinate-space incarnation.
    CoordinateSpaceMismatch,
    /// A semantic item, depth, or byte bound rejected the claimed shape.
    Bound(SemanticBoundViolation),
    /// The allocation length cannot fit the portable byte count.
    LengthOverflow,
    /// The allocation length differs from its prevalidated byte count.
    LengthMismatch {
        /// Byte count validated before allocation.
        prepared: u64,
        /// Actual allocated byte count when representable.
        actual: u64,
    },
}

impl fmt::Display for SemanticIntentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IncoherentSafety => {
                "semantic Intent carrier safety terms are incoherent"
            }
            Self::TypeMismatch => {
                "semantic Intent carrier type identity differs"
            }
            Self::IncompatibleSchema => {
                "semantic Intent carrier schema version is incompatible"
            }
            Self::RepresentationGenerationMismatch => {
                "semantic Intent names another Representation generation"
            }
            Self::CoordinateSpaceMismatch => {
                "semantic Intent names another coordinate-space incarnation"
            }
            Self::Bound(_) => "semantic Intent exceeds its declared bounds",
            Self::LengthOverflow => {
                "semantic Intent allocation exceeds its portable count"
            }
            Self::LengthMismatch { .. } => {
                "semantic Intent allocation differs from its preparation"
            }
        })
    }
}

impl core::error::Error for SemanticIntentError {}

/// Move-only pre-allocation proof for one semantic Intent carrier.
pub struct SemanticIntentPreparation {
    pub(super) stamp: SemanticIntentStamp,
    pub(super) carrier: SemanticIntentCarrierStamp,
    pub(super) shape: SemanticIntentShape,
}

impl SemanticIntentPreparation {
    /// Returns the validated outer semantic Intent stamp.
    #[must_use]
    pub const fn stamp(&self) -> &SemanticIntentStamp {
        &self.stamp
    }

    /// Returns the validated carrier type and selected schema.
    #[must_use]
    pub const fn carrier(&self) -> SemanticIntentCarrierStamp {
        self.carrier
    }

    /// Returns the validated structural and byte shape.
    #[must_use]
    pub const fn shape(&self) -> SemanticIntentShape {
        self.shape
    }

    /// Binds an exact allocation and outer stamp to the selected descriptor.
    ///
    /// Rejection retains the unchanged proof and allocation with a typed cause.
    #[must_use]
    pub fn bind(
        self,
        descriptor: &RepresentationPolicyDescriptor,
        bytes: ::alloc::boxed::Box<[u8]>,
    ) -> SemanticIntentBindingOutcome {
        if let Err(cause) = validate_preparation(descriptor, &self) {
            return SemanticIntentBindingOutcome::Rejected(
                self.reject(cause, bytes),
            );
        }
        let actual = match actual_length(&bytes) {
            Ok(value) => value,
            Err(cause) => {
                return SemanticIntentBindingOutcome::Rejected(
                    self.reject(cause, bytes),
                );
            }
        };
        let prepared = self.shape().bytes().value();
        if actual != prepared {
            let cause =
                SemanticIntentError::LengthMismatch { prepared, actual };
            return SemanticIntentBindingOutcome::Rejected(
                self.reject(cause, bytes),
            );
        }
        SemanticIntentBindingOutcome::Bound(SemanticIntent {
            preparation: self,
            bytes,
        })
    }

    const fn reject(
        self,
        cause: SemanticIntentError,
        bytes: ::alloc::boxed::Box<[u8]>,
    ) -> SemanticIntentRejection {
        SemanticIntentRejection { cause, preparation: self, bytes }
    }
}

/// Exact owning result of binding one semantic Intent allocation.
pub enum SemanticIntentBindingOutcome {
    /// Structural validation succeeded and produced a retained Intent.
    Bound(SemanticIntent),
    /// Binding failed without losing the preparation or allocation.
    Rejected(SemanticIntentRejection),
}

impl fmt::Debug for SemanticIntentBindingOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bound(intent) => {
                formatter.debug_tuple("Bound").field(intent).finish()
            }
            Self::Rejected(rejection) => {
                formatter.debug_tuple("Rejected").field(rejection).finish()
            }
        }
    }
}

impl fmt::Debug for SemanticIntentPreparation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SemanticIntentPreparation")
            .field("stamp", &self.stamp)
            .field("carrier", &self.carrier)
            .field("shape", &self.shape)
            .finish()
    }
}

/// Move-only immutable schema-qualified semantic Intent.
pub struct SemanticIntent {
    preparation: SemanticIntentPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl SemanticIntent {
    /// Returns every authority and observed-state stamp on this Intent.
    #[must_use]
    pub const fn stamp(&self) -> &SemanticIntentStamp {
        self.preparation.stamp()
    }

    /// Returns the carrier type and selected schema version.
    #[must_use]
    pub const fn carrier(&self) -> SemanticIntentCarrierStamp {
        self.preparation.carrier()
    }

    /// Returns the checked item, nesting, and byte shape.
    #[must_use]
    pub const fn shape(&self) -> SemanticIntentShape {
        self.preparation.shape()
    }

    /// Borrows the immutable provider-defined encoded bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Releases the Intent and returns its exact allocation.
    #[must_use]
    pub fn into_bytes(self) -> ::alloc::boxed::Box<[u8]> {
        self.bytes
    }
}

impl fmt::Debug for SemanticIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SemanticIntent")
            .field("stamp", &self.stamp())
            .field("carrier", &self.carrier())
            .field("shape", &self.shape())
            .finish_non_exhaustive()
    }
}

/// Failed Intent binding that preserves the exact allocated bytes.
pub struct SemanticIntentRejection {
    cause: SemanticIntentError,
    preparation: SemanticIntentPreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl SemanticIntentRejection {
    /// Returns the exact Intent-binding failure.
    #[must_use]
    pub const fn cause(&self) -> SemanticIntentError {
        self.cause
    }

    /// Borrows the unchanged pre-allocation proof.
    #[must_use]
    pub const fn preparation(&self) -> &SemanticIntentPreparation {
        &self.preparation
    }

    /// Borrows the unchanged rejected allocation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns ownership of the unchanged proof and rejected allocation.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (SemanticIntentPreparation, ::alloc::boxed::Box<[u8]>) {
        (self.preparation, self.bytes)
    }
}

impl fmt::Debug for SemanticIntentRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SemanticIntentRejection")
            .field("cause", &self.cause)
            .field("preparation", &self.preparation)
            .field("byte_count", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

pub(super) const fn validate_outer(
    descriptor: &RepresentationPolicyDescriptor,
    stamp: &SemanticIntentStamp,
) -> Result<(), SemanticIntentError> {
    if !descriptor.stamp().same_generation(stamp.projection().representation())
    {
        return Err(SemanticIntentError::RepresentationGenerationMismatch);
    }
    if !descriptor.coordinate().stamp().same_incarnation(stamp.coordinate()) {
        return Err(SemanticIntentError::CoordinateSpaceMismatch);
    }
    Ok(())
}

pub(super) const fn validate_carrier(
    descriptor: &RepresentationPolicyDescriptor,
    carrier: SemanticIntentCarrierStamp,
) -> Result<(), SemanticIntentError> {
    descriptor.intent().validate_stamp(carrier)
}

pub(super) const fn prepare_shape(
    descriptor: &RepresentationPolicyDescriptor,
    items: u64,
    depth: u64,
    bytes: u64,
) -> Result<SemanticIntentShape, SemanticIntentError> {
    match descriptor.coordinate().intent_bounds().prepare(items, depth, bytes) {
        Ok(value) => Ok(value),
        Err(error) => Err(SemanticIntentError::Bound(error)),
    }
}

const fn validate_preparation(
    descriptor: &RepresentationPolicyDescriptor,
    preparation: &SemanticIntentPreparation,
) -> Result<(), SemanticIntentError> {
    match validate_outer(descriptor, preparation.stamp()) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_carrier(descriptor, preparation.carrier()) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let shape = preparation.shape();
    match prepare_shape(
        descriptor,
        shape.items().value(),
        shape.depth().value(),
        shape.bytes().value(),
    ) {
        Ok(_) => Ok(()),
        Err(error) => Err(error),
    }
}

fn actual_length(bytes: &[u8]) -> Result<u64, SemanticIntentError> {
    u64::try_from(bytes.len()).map_err(|_| SemanticIntentError::LengthOverflow)
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
