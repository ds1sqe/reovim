use crate::clock::ClockDescriptor;
use crate::operation::{ContractSelection, OperationDescriptor};

use super::{EventError, StartupEventIdentities, StartupEventLimits};

impl crate::event::StartupEventDescriptor {
    /// Constructs one contract-only startup Event descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::ChargeOwnerMismatch`] when an admission bound is
    /// charged to an instance other than the exact Event Stream instance.
    pub const fn new(
        contract: ContractSelection,
        identities: StartupEventIdentities,
        clock: ClockDescriptor,
        limits: StartupEventLimits,
    ) -> Result<Self, EventError> {
        if contract.identity() != identities.contract() {
            return Err(EventError::DescriptorContractMismatch);
        }
        match limits.validate_owner(identities.stream().instance()) {
            Ok(()) => Ok(Self { contract, identities, clock, limits }),
            Err(error) => Err(error),
        }
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
