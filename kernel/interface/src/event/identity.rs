use reovim_foundation::contract::ContractId;

use crate::lifecycle::LifecycleIdentity;

impl crate::event::StartupEventIdentities {
    /// Constructs explicitly supplied startup Event identities.
    #[must_use]
    pub const fn new(
        contract: u128,
        family: u128,
        remediation_catalog: u128,
        canonical_reference: u128,
        stream: LifecycleIdentity,
        lifecycle_owner: LifecycleIdentity,
    ) -> Self {
        Self {
            contract: ContractId::new(contract),
            family: ContractId::new(family),
            remediation_catalog: ContractId::new(remediation_catalog),
            canonical_reference: ContractId::new(canonical_reference),
            stream,
            lifecycle_owner,
        }
    }

    /// Returns the startup Event contract identity scalar.
    #[must_use]
    pub const fn contract(self) -> u128 {
        self.contract.value()
    }

    /// Returns the `kernel.startup` family identity scalar.
    #[must_use]
    pub const fn family(self) -> u128 {
        self.family.value()
    }

    /// Returns the `kernel.bootstrap` remediation catalog identity scalar.
    #[must_use]
    pub const fn remediation_catalog(self) -> u128 {
        self.remediation_catalog.value()
    }

    /// Returns the borrowed canonical-reference carrier type identity.
    #[must_use]
    pub const fn canonical_reference(self) -> u128 {
        self.canonical_reference.value()
    }

    /// Returns the exact Event Stream lifecycle identity.
    #[must_use]
    pub const fn stream(self) -> LifecycleIdentity {
        self.stream
    }

    /// Returns the lifecycle authority admitted to propose startup state.
    #[must_use]
    pub const fn lifecycle_owner(self) -> LifecycleIdentity {
        self.lifecycle_owner
    }
}
