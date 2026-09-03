use crate::carrier::{
    BorrowModel, CopyModel, DropModel, InvocationModel, LifetimeModel,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use crate::clock::ClockDescriptor;
use crate::operation::{ContractSelection, OperationDescriptor};

use super::{EventError, StartupEventIdentities, StartupEventLimits};

impl crate::event::StartupEventDescriptor {
    /// Constructs one contract-only startup Event descriptor.
    ///
    /// # Errors
    ///
    /// Returns a typed contract-identity, bound-owner, or reference-carrier
    /// coherence failure.
    pub const fn new(
        contract: ContractSelection,
        identities: StartupEventIdentities,
        clock: ClockDescriptor,
        limits: StartupEventLimits,
        canonical_reference_safety: super::CarrierSafetyDescriptor,
    ) -> Result<Self, EventError> {
        if contract.identity() != identities.contract() {
            return Err(EventError::DescriptorContractMismatch);
        }
        if let Err(error) =
            limits.validate_owner(identities.stream().instance())
        {
            return Err(error);
        }
        if !canonical_reference_safety_is_exact(
            contract,
            identities,
            canonical_reference_safety,
        ) {
            return Err(EventError::IncoherentCarrier);
        }
        Ok(Self {
            contract,
            identities,
            clock,
            limits,
            canonical_reference_safety,
        })
    }

    /// Returns the selected startup Event contract.
    #[must_use]
    pub const fn contract(&self) -> ContractSelection {
        self.contract
    }

    /// Returns all authority-assigned identities.
    #[must_use]
    pub const fn identities(&self) -> StartupEventIdentities {
        self.identities
    }

    /// Returns the exact clock descriptor bound to this Stream incarnation.
    #[must_use]
    pub const fn clock(&self) -> ClockDescriptor {
        self.clock
    }

    /// Returns every startup Event hard bound.
    #[must_use]
    pub const fn limits(&self) -> &StartupEventLimits {
        &self.limits
    }

    /// Returns safety terms for the borrowed canonical-record reference.
    #[must_use]
    pub const fn canonical_reference_safety(
        &self,
    ) -> super::CarrierSafetyDescriptor {
        self.canonical_reference_safety
    }

    /// Returns the fixed `kernel.startup` schema version.
    #[must_use]
    pub const fn family_schema_version(&self) -> u64 {
        1
    }

    /// Returns the fixed `kernel.bootstrap` catalog version.
    #[must_use]
    pub const fn remediation_catalog_version(&self) -> u64 {
        1
    }

    /// Returns the exact producer-admission operation classification.
    #[must_use]
    pub const fn admission_operation(&self) -> OperationDescriptor {
        OperationDescriptor::external_observation_inline()
    }

    /// Returns the exact snapshot operation classification.
    #[must_use]
    pub const fn snapshot_operation(&self) -> OperationDescriptor {
        OperationDescriptor::local_inline()
    }

    /// Validates one requested startup Event contract identity and version.
    ///
    /// # Errors
    ///
    /// Returns a typed unsupported-family or incompatible-version error.
    pub const fn negotiate(
        &self,
        requested_contract: u128,
        requested_version: u64,
    ) -> Result<(), EventError> {
        if requested_contract != self.identities.contract() {
            return Err(EventError::UnsupportedContract);
        }
        if !self.contract.supports(requested_version) {
            return Err(EventError::IncompatibleContract);
        }
        Ok(())
    }
}

const fn canonical_reference_safety_is_exact(
    contract: ContractSelection,
    identities: StartupEventIdentities,
    safety: super::CarrierSafetyDescriptor,
) -> bool {
    let versions = safety.schema_versions();
    let memory = safety.memory();
    let concurrency = safety.concurrency();
    let invocation = safety.invocation();
    safety.type_id().value() == identities.canonical_reference()
        && versions.minimum().value() == contract.minimum()
        && versions.maximum().value() == contract.maximum()
        && canonical_memory_code(memory) == 0
        && canonical_concurrency_code(concurrency) == 0
        && canonical_invocation_code(invocation) == 0
        && canonical_mismatch_code(safety.mismatch()) == 0
        && canonical_boundary_code(safety.unsafe_boundary()) == 0
}

const fn canonical_memory_code(memory: crate::carrier::MemorySafety) -> u8 {
    match (
        memory.ownership(),
        memory.borrowing(),
        memory.copying(),
        memory.dropping(),
        memory.lifetime(),
    ) {
        (
            OwnershipModel::Borrowed,
            BorrowModel::Shared,
            CopyModel::Copy,
            DropModel::Trivial,
            LifetimeModel::IncarnationBound,
        ) => 0,
        _ => 1,
    }
}

const fn canonical_concurrency_code(
    concurrency: crate::carrier::ConcurrencySafety,
) -> u8 {
    match (concurrency.thread_transfer(), concurrency.sharing()) {
        (ThreadTransfer::Transferable, SharingModel::Immutable) => 0,
        _ => 1,
    }
}

const fn canonical_invocation_code(
    invocation: crate::carrier::InvocationSafety,
) -> u8 {
    match (invocation.invocation(), invocation.unwind()) {
        (InvocationModel::DataOnly, UnwindModel::NotInvocable) => 0,
        _ => 1,
    }
}

const fn canonical_mismatch_code(mismatch: TypeMismatch) -> u8 {
    match mismatch {
        TypeMismatch::Reject => 0,
    }
}

const fn canonical_boundary_code(boundary: UnsafeBoundary) -> u8 {
    match boundary {
        UnsafeBoundary::None => 0,
        UnsafeBoundary::Reviewed(_) => 1,
    }
}
