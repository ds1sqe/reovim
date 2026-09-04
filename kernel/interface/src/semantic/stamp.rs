//! Exact stamps for semantic policy work.

use core::fmt;

use crate::buffer::BufferStateStamp;
use crate::buffer::plan::BufferPlanStamp;
use crate::identity::IncarnationId;
use crate::lifecycle::LifecycleIdentity;
use crate::operation::ContractSelection;

use super::descriptor::RepresentationPolicyDescriptor;
use super::identity::{
    CoordinateSpaceIdentity, DomainIdentity, ProjectionGeneration,
    ProjectionIdentity, RepresentationIdentity, SemanticIntentIdentity,
    SemanticRequestIdentity,
};
use super::intent::{
    SemanticIntentCarrierStamp, SemanticIntentError, SemanticIntentPreparation,
};

/// Exact selected Domain policy generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainPolicyStamp {
    domain: DomainIdentity,
    contract: ContractSelection,
    lifecycle: LifecycleIdentity,
}

impl DomainPolicyStamp {
    /// Constructs one selected Domain policy stamp.
    #[must_use]
    pub const fn new(
        domain: DomainIdentity,
        contract: ContractSelection,
        lifecycle: LifecycleIdentity,
    ) -> Self {
        Self { domain, contract, lifecycle }
    }

    /// Returns the semantic Domain identity.
    #[must_use]
    pub const fn domain(self) -> DomainIdentity {
        self.domain
    }

    /// Returns the selected Domain contract.
    #[must_use]
    pub const fn contract(self) -> ContractSelection {
        self.contract
    }

    /// Returns the exact policy lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Reports exact Domain, contract, incarnation, and generation equality.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.domain.value() == other.domain.value()
            && same_contract(self.contract, other.contract)
            && self.lifecycle.same_generation(other.lifecycle)
    }
}

/// Exact selected Representation policy generation and owning Domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationPolicyStamp {
    representation: RepresentationIdentity,
    contract: ContractSelection,
    lifecycle: LifecycleIdentity,
    domain: DomainPolicyStamp,
}

impl RepresentationPolicyStamp {
    /// Constructs one selected Representation policy stamp.
    #[must_use]
    pub const fn new(
        representation: RepresentationIdentity,
        contract: ContractSelection,
        lifecycle: LifecycleIdentity,
        domain: DomainPolicyStamp,
    ) -> Self {
        Self { representation, contract, lifecycle, domain }
    }

    /// Returns the Representation identity.
    #[must_use]
    pub const fn representation(self) -> RepresentationIdentity {
        self.representation
    }

    /// Returns the selected Representation contract.
    #[must_use]
    pub const fn contract(self) -> ContractSelection {
        self.contract
    }

    /// Returns the exact Representation lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the exact owning Domain policy generation.
    #[must_use]
    pub const fn domain(self) -> DomainPolicyStamp {
        self.domain
    }

    /// Reports exact Representation, contract, and owner equality.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.representation.value() == other.representation.value()
            && same_contract(self.contract, other.contract)
            && self.lifecycle.same_generation(other.lifecycle)
            && self.domain.same_generation(other.domain)
    }
}

/// Exact maintained Projection and predecessor generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectionStamp {
    projection: ProjectionIdentity,
    lifecycle: LifecycleIdentity,
    representation: RepresentationPolicyStamp,
    predecessor: ProjectionGeneration,
}

impl ProjectionStamp {
    /// Constructs one observed Projection predecessor stamp.
    #[must_use]
    pub const fn new(
        projection: ProjectionIdentity,
        lifecycle: LifecycleIdentity,
        representation: RepresentationPolicyStamp,
        predecessor: ProjectionGeneration,
    ) -> Self {
        Self { projection, lifecycle, representation, predecessor }
    }

    /// Returns the maintained Projection identity.
    #[must_use]
    pub const fn projection(self) -> ProjectionIdentity {
        self.projection
    }

    /// Returns the Projection lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the selected Representation policy generation.
    #[must_use]
    pub const fn representation(self) -> RepresentationPolicyStamp {
        self.representation
    }

    /// Returns the predecessor Projection generation observed by the caller.
    #[must_use]
    pub const fn predecessor(self) -> ProjectionGeneration {
        self.predecessor
    }

    /// Reports equality of the complete observed Projection state.
    #[must_use]
    pub const fn same_state(self, other: Self) -> bool {
        self.projection.value() == other.projection.value()
            && self.lifecycle.same_generation(other.lifecycle)
            && self.representation.same_generation(other.representation)
            && self.predecessor.value() == other.predecessor.value()
    }
}

/// Exact selected coordinate-space contract and incarnation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoordinateSpaceStamp {
    space: CoordinateSpaceIdentity,
    contract: ContractSelection,
    incarnation: IncarnationId,
}

impl CoordinateSpaceStamp {
    /// Constructs one live coordinate-space stamp.
    #[must_use]
    pub const fn new(
        space: CoordinateSpaceIdentity,
        contract: ContractSelection,
        incarnation: IncarnationId,
    ) -> Self {
        Self { space, contract, incarnation }
    }

    /// Returns the coordinate-space identity.
    #[must_use]
    pub const fn space(self) -> CoordinateSpaceIdentity {
        self.space
    }

    /// Returns the selected coordinate-space contract.
    #[must_use]
    pub const fn contract(self) -> ContractSelection {
        self.contract
    }

    /// Returns the live coordinate-space incarnation.
    #[must_use]
    pub const fn incarnation(self) -> IncarnationId {
        self.incarnation
    }

    /// Reports exact identity, contract, and incarnation equality.
    #[must_use]
    pub const fn same_incarnation(self, other: Self) -> bool {
        self.space.value() == other.space.value()
            && same_contract(self.contract, other.contract)
            && self.incarnation.value() == other.incarnation.value()
    }
}

/// Exact caller authority and selected semantic caller role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticCallerStamp {
    authority: LifecycleIdentity,
    role: ContractSelection,
}

impl SemanticCallerStamp {
    /// Constructs one caller authority and role stamp.
    #[must_use]
    pub const fn new(
        authority: LifecycleIdentity,
        role: ContractSelection,
    ) -> Self {
        Self { authority, role }
    }

    /// Returns the exact caller authority generation.
    #[must_use]
    pub const fn authority(self) -> LifecycleIdentity {
        self.authority
    }

    /// Returns the selected semantic caller-role contract.
    #[must_use]
    pub const fn role(self) -> ContractSelection {
        self.role
    }

    /// Reports equality of caller authority and selected role.
    #[must_use]
    pub const fn same_authority(self, other: Self) -> bool {
        self.authority.same_generation(other.authority)
            && same_contract(self.role, other.role)
    }
}

/// Complete move-owned semantic request and future Buffer-plan stamp.
pub struct SemanticIntentStamp {
    intent: SemanticIntentIdentity,
    caller: SemanticCallerStamp,
    request: SemanticRequestIdentity,
    target: BufferStateStamp,
    plan: BufferPlanStamp,
    projection: ProjectionStamp,
    coordinate: CoordinateSpaceStamp,
}

impl fmt::Debug for SemanticIntentStamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SemanticIntentStamp")
            .field("intent", &self.intent)
            .field("caller", &self.caller)
            .field("request", &self.request)
            .field("target", &self.target)
            .field("plan", &self.plan)
            .field("projection", &self.projection)
            .field("coordinate", &self.coordinate)
            .finish()
    }
}

impl SemanticIntentStamp {
    /// Constructs one complete semantic Intent stamp.
    #[must_use]
    pub const fn new(
        intent: SemanticIntentIdentity,
        caller: SemanticCallerStamp,
        request: SemanticRequestIdentity,
        target: BufferStateStamp,
        plan: BufferPlanStamp,
        projection: ProjectionStamp,
        coordinate: CoordinateSpaceStamp,
    ) -> Self {
        Self { intent, caller, request, target, plan, projection, coordinate }
    }

    /// Validates this semantic Intent stamp before caller allocation.
    ///
    /// # Errors
    ///
    /// Returns carrier type, schema, or structural bound failure.
    pub const fn prepare_intent(
        self,
        descriptor: &RepresentationPolicyDescriptor,
        carrier: SemanticIntentCarrierStamp,
        items: u64,
        depth: u64,
        bytes: u64,
    ) -> Result<SemanticIntentPreparation, SemanticIntentError> {
        match super::intent::validate_outer(descriptor, &self) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match super::intent::validate_carrier(descriptor, carrier) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let shape =
            match super::intent::prepare_shape(descriptor, items, depth, bytes)
            {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
        Ok(SemanticIntentPreparation { stamp: self, carrier, shape })
    }

    /// Returns the unique immutable Intent identity.
    #[must_use]
    pub const fn intent(&self) -> SemanticIntentIdentity {
        self.intent
    }

    /// Returns the exact caller authority and role.
    #[must_use]
    pub const fn caller(&self) -> SemanticCallerStamp {
        self.caller
    }

    /// Returns the semantic request identity.
    #[must_use]
    pub const fn request(&self) -> SemanticRequestIdentity {
        self.request
    }

    /// Returns the exact Resource, Buffer, version, and History target.
    #[must_use]
    pub const fn target(&self) -> &BufferStateStamp {
        &self.target
    }

    /// Returns the future Buffer Transaction-plan stamp.
    #[must_use]
    pub const fn plan(&self) -> &BufferPlanStamp {
        &self.plan
    }

    /// Returns the exact observed Projection predecessor.
    #[must_use]
    pub const fn projection(&self) -> &ProjectionStamp {
        &self.projection
    }

    /// Returns the selected coordinate-space incarnation.
    #[must_use]
    pub const fn coordinate(&self) -> CoordinateSpaceStamp {
        self.coordinate
    }

    /// Reports equality of every authority and observed state stamp.
    #[must_use]
    pub const fn same_request(&self, other: &Self) -> bool {
        self.intent.value() == other.intent.value()
            && self.caller.same_authority(other.caller)
            && self.request.value() == other.request.value()
            && self.target.same_state(other.target)
            && self.plan.same_plan(other.plan)
            && self.projection.same_state(other.projection)
            && self.coordinate.same_incarnation(other.coordinate)
    }
}

const fn same_contract(
    left: ContractSelection,
    right: ContractSelection,
) -> bool {
    left.identity() == right.identity()
        && left.selected() == right.selected()
        && left.minimum() == right.minimum()
        && left.maximum() == right.maximum()
}
