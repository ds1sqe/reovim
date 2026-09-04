//! Selected semantic policy and coordinate-space descriptors.

use core::fmt;

use crate::buffer::carrier::BufferCarrierStamp;
use crate::buffer::plan::BufferPlanDescriptor;
use crate::carrier::{InvocationModel, InvocationSafety, UnwindModel};
use crate::operation::{ContractSelection, OperationDescriptor};

use super::bounds::{SemanticIntentBounds, SemanticPolicyCallBounds};
use super::intent::SemanticIntentContract;
use super::stamp::{
    CoordinateSpaceStamp, DomainPolicyStamp, RepresentationPolicyStamp,
};

/// Preservation guarantee declared by one Representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticFidelity {
    /// Every source Unit in scope is represented or losslessly recoverable.
    Faithful,
    /// Accepted edits preserve every unexpressed source Unit.
    ConstrainedEditable,
    /// Derived state may be lossy and cannot directly create edits.
    DerivedReadOnly,
}

/// Whether one Representation may produce Buffer Transaction plans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticEditability {
    /// No semantic Intent may lower through this Representation.
    ReadOnly,
    /// Valid semantic Intent may lower into a complete Transaction plan.
    Transactional,
}

/// Source correspondence supplied by one Representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticCorrespondence {
    /// Source state maps forward and semantic locations map back to edits.
    Bidirectional,
    /// Source state maps into Representation state without reverse mapping.
    SourceOnly,
    /// No stable item-level source correspondence is declared.
    None,
}

/// Coherent fidelity, editability, and correspondence declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationBehavior {
    fidelity: SemanticFidelity,
    editability: SemanticEditability,
    correspondence: SemanticCorrespondence,
}

impl RepresentationBehavior {
    /// Constructs one coherent Representation behavior declaration.
    ///
    /// # Errors
    ///
    /// Returns a typed contradiction between the three behavior axes.
    pub fn new(
        fidelity: SemanticFidelity,
        editability: SemanticEditability,
        correspondence: SemanticCorrespondence,
    ) -> Result<Self, SemanticDescriptorError> {
        validate_representation(fidelity, editability, correspondence)?;
        Ok(Self { fidelity, editability, correspondence })
    }

    /// Returns the declared fidelity class.
    #[must_use]
    pub const fn fidelity(self) -> SemanticFidelity {
        self.fidelity
    }

    /// Returns whether edit lowering is available.
    #[must_use]
    pub const fn editability(self) -> SemanticEditability {
        self.editability
    }

    /// Returns the declared source-correspondence class.
    #[must_use]
    pub const fn correspondence(self) -> SemanticCorrespondence {
        self.correspondence
    }
}

/// Progress model selected for one semantic policy callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyProgress {
    /// One invocation returns within its declared work bound.
    BoundedToCompletion,
}

/// Concurrent invocation relation selected for one policy instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyConcurrency {
    /// At most one callback runs for one policy instance.
    Serialized,
    /// Calls may run concurrently with independent immutable inputs.
    ParallelIndependent,
}

/// Runtime context selected for one in-process semantic callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyExecutionContext {
    /// Kernel Runtime owns dispatch on one of its admitted workers.
    RuntimeWorker,
}

/// Worker-affinity requirement of one in-process semantic callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyAffinity {
    /// Any Runtime worker may invoke the selected thread-safe provider.
    AnyRuntimeWorker,
}

/// Outward service calls available during one semantic callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyOutwardCalls {
    /// The callback receives no Runtime, I/O, queue, or service handle.
    None,
}

/// Cancellation and deadline observation during one bounded callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPolicyInterruptionObservation {
    /// Runtime observes interruption before entry and after callback return.
    RuntimeBoundaries,
}

/// Typed semantic descriptor coherence failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticDescriptorError {
    /// A Representation stamp names another Domain generation.
    DomainGenerationMismatch,
    /// Derived read-only fidelity cannot be Transactional.
    DerivedEditable,
    /// Constrained editable fidelity must be Transactional.
    ConstrainedReadOnly,
    /// Transactional editability requires reverse correspondence.
    TransactionWithoutReverseCorrespondence,
    /// Faithful fidelity requires at least source correspondence.
    FaithfulWithoutCorrespondence,
    /// Serialized policy allows more than one call per policy instance.
    SerializedConcurrencyExceeded {
        /// Rejected per-instance concurrent-call maximum.
        actual: u64,
    },
}

impl fmt::Display for SemanticDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DomainGenerationMismatch => {
                "Representation belongs to another Domain generation"
            }
            Self::DerivedEditable => {
                "derived read-only Representation cannot be Transactional"
            }
            Self::ConstrainedReadOnly => {
                "constrained editable Representation must be Transactional"
            }
            Self::TransactionWithoutReverseCorrespondence => {
                "Transactional Representation requires reverse correspondence"
            }
            Self::FaithfulWithoutCorrespondence => {
                "faithful Representation requires source correspondence"
            }
            Self::SerializedConcurrencyExceeded { .. } => {
                "serialized semantic policy admits concurrent callbacks"
            }
        })
    }
}

impl core::error::Error for SemanticDescriptorError {}

/// Complete bounded invocation contract for one semantic policy callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticPolicyCallDescriptor {
    operation: OperationDescriptor,
    safety: InvocationSafety,
    progress: SemanticPolicyProgress,
    concurrency: SemanticPolicyConcurrency,
    context: SemanticPolicyExecutionContext,
    affinity: SemanticPolicyAffinity,
    outward_calls: SemanticPolicyOutwardCalls,
    interruption: SemanticPolicyInterruptionObservation,
    bounds: SemanticPolicyCallBounds,
}

impl SemanticPolicyCallDescriptor {
    /// Constructs the fixed deferred, contained semantic callback contract.
    ///
    /// # Errors
    ///
    /// Returns a typed concurrency mismatch for a serialized policy.
    pub fn new(
        concurrency: SemanticPolicyConcurrency,
        bounds: &SemanticPolicyCallBounds,
    ) -> Result<Self, SemanticDescriptorError> {
        let maximum = bounds.concurrent_calls().per_object().value();
        if concurrency == SemanticPolicyConcurrency::Serialized && maximum != 1
        {
            return Err(
                SemanticDescriptorError::SerializedConcurrencyExceeded {
                    actual: maximum,
                },
            );
        }
        Ok(Self {
            operation: OperationDescriptor::local_deferred_callback_admission(),
            safety: InvocationSafety::new(
                InvocationModel::UserCallback,
                UnwindModel::Contained,
            ),
            progress: SemanticPolicyProgress::BoundedToCompletion,
            concurrency,
            context: SemanticPolicyExecutionContext::RuntimeWorker,
            affinity: SemanticPolicyAffinity::AnyRuntimeWorker,
            outward_calls: SemanticPolicyOutwardCalls::None,
            interruption:
                SemanticPolicyInterruptionObservation::RuntimeBoundaries,
            bounds: bounds.clone(),
        })
    }

    /// Returns the Runtime-owned deferred operation shape.
    #[must_use]
    pub const fn operation(&self) -> OperationDescriptor {
        self.operation
    }

    /// Returns the callback and contained-unwind safety declaration.
    #[must_use]
    pub const fn safety(&self) -> InvocationSafety {
        self.safety
    }

    /// Returns the callback progress model.
    #[must_use]
    pub const fn progress(&self) -> SemanticPolicyProgress {
        self.progress
    }

    /// Returns the policy-instance concurrency relation.
    #[must_use]
    pub const fn concurrency(&self) -> SemanticPolicyConcurrency {
        self.concurrency
    }

    /// Returns the Runtime-owned callback execution context.
    #[must_use]
    pub const fn context(&self) -> SemanticPolicyExecutionContext {
        self.context
    }

    /// Returns the selected provider's worker-affinity requirement.
    #[must_use]
    pub const fn affinity(&self) -> SemanticPolicyAffinity {
        self.affinity
    }

    /// Returns the outward-call authority available to the callback.
    #[must_use]
    pub const fn outward_calls(&self) -> SemanticPolicyOutwardCalls {
        self.outward_calls
    }

    /// Returns where Runtime observes cancellation and deadline state.
    #[must_use]
    pub const fn interruption(&self) -> SemanticPolicyInterruptionObservation {
        self.interruption
    }

    /// Returns work, memory, result, and concurrent-call bounds.
    #[must_use]
    pub const fn bounds(&self) -> &SemanticPolicyCallBounds {
        &self.bounds
    }
}

/// Selected coordinate, region, and value schemas for one semantic space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticCoordinateSpaceDescriptor {
    stamp: CoordinateSpaceStamp,
    coordinate_schema: ContractSelection,
    region_schema: ContractSelection,
    value_schema: ContractSelection,
    intent_bounds: SemanticIntentBounds,
}

impl SemanticCoordinateSpaceDescriptor {
    /// Constructs one selected coordinate-space declaration.
    #[must_use]
    pub fn new(
        stamp: CoordinateSpaceStamp,
        coordinate_schema: ContractSelection,
        region_schema: ContractSelection,
        value_schema: ContractSelection,
        intent_bounds: &SemanticIntentBounds,
    ) -> Self {
        Self {
            stamp,
            coordinate_schema,
            region_schema,
            value_schema,
            intent_bounds: intent_bounds.clone(),
        }
    }

    /// Returns the selected coordinate-space incarnation.
    #[must_use]
    pub const fn stamp(&self) -> CoordinateSpaceStamp {
        self.stamp
    }

    /// Returns the selected coordinate carrier schema.
    #[must_use]
    pub const fn coordinate_schema(&self) -> ContractSelection {
        self.coordinate_schema
    }

    /// Returns the selected semantic-region carrier schema.
    #[must_use]
    pub const fn region_schema(&self) -> ContractSelection {
        self.region_schema
    }

    /// Returns the selected semantic-value carrier schema.
    #[must_use]
    pub const fn value_schema(&self) -> ContractSelection {
        self.value_schema
    }

    /// Returns structural and byte bounds for an Intent in this space.
    #[must_use]
    pub const fn intent_bounds(&self) -> &SemanticIntentBounds {
        &self.intent_bounds
    }
}

/// Selected Domain validation provider contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainPolicyDescriptor {
    stamp: DomainPolicyStamp,
    validation: SemanticPolicyCallDescriptor,
}

impl DomainPolicyDescriptor {
    /// Constructs one selected Domain validation descriptor.
    #[must_use]
    pub fn new(
        stamp: DomainPolicyStamp,
        validation: &SemanticPolicyCallDescriptor,
    ) -> Self {
        Self { stamp, validation: validation.clone() }
    }

    /// Returns the exact selected Domain policy generation.
    #[must_use]
    pub const fn stamp(&self) -> DomainPolicyStamp {
        self.stamp
    }

    /// Returns the Domain validation call contract.
    #[must_use]
    pub const fn validation(&self) -> &SemanticPolicyCallDescriptor {
        &self.validation
    }
}

/// Exact Buffer-plan contract and carrier selections produced by policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticBufferPlanContract {
    contract: ContractSelection,
    origin: BufferCarrierStamp,
    operations: BufferCarrierStamp,
    preconditions: BufferCarrierStamp,
}

impl SemanticBufferPlanContract {
    /// Captures the public selections of one Buffer-plan descriptor.
    #[must_use]
    pub const fn from_descriptor(descriptor: &BufferPlanDescriptor) -> Self {
        Self {
            contract: descriptor.contract(),
            origin: descriptor.carriers().origin().selected_stamp(),
            operations: descriptor.carriers().operations().selected_stamp(),
            preconditions: descriptor
                .carriers()
                .preconditions()
                .selected_stamp(),
        }
    }

    /// Returns the selected Buffer-plan contract.
    #[must_use]
    pub const fn contract(self) -> ContractSelection {
        self.contract
    }

    /// Returns the selected origin carrier type and schema.
    #[must_use]
    pub const fn origin(self) -> BufferCarrierStamp {
        self.origin
    }

    /// Returns the selected Operation carrier type and schema.
    #[must_use]
    pub const fn operations(self) -> BufferCarrierStamp {
        self.operations
    }

    /// Returns the selected Precondition carrier type and schema.
    #[must_use]
    pub const fn preconditions(self) -> BufferCarrierStamp {
        self.preconditions
    }

    /// Reports whether a Buffer-plan descriptor has every exact selection.
    #[must_use]
    pub const fn matches_descriptor(
        self,
        descriptor: &BufferPlanDescriptor,
    ) -> bool {
        same_contract(self.contract, descriptor.contract())
            && same_carrier_stamp(
                self.origin,
                descriptor.carriers().origin().selected_stamp(),
            )
            && same_carrier_stamp(
                self.operations,
                descriptor.carriers().operations().selected_stamp(),
            )
            && same_carrier_stamp(
                self.preconditions,
                descriptor.carriers().preconditions().selected_stamp(),
            )
    }
}

/// Selected Representation edit-lowering provider contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationPolicyDescriptor {
    stamp: RepresentationPolicyStamp,
    coordinate: SemanticCoordinateSpaceDescriptor,
    intent: SemanticIntentContract,
    behavior: RepresentationBehavior,
    plan: SemanticBufferPlanContract,
    lowering: SemanticPolicyCallDescriptor,
}

impl RepresentationPolicyDescriptor {
    /// Constructs and validates one complete edit-lowering descriptor.
    ///
    /// # Errors
    ///
    /// Returns a typed Domain, fidelity, editability, or correspondence error.
    pub fn new(
        domain: &DomainPolicyDescriptor,
        stamp: RepresentationPolicyStamp,
        coordinate: &SemanticCoordinateSpaceDescriptor,
        intent: &SemanticIntentContract,
        behavior: RepresentationBehavior,
        plan: SemanticBufferPlanContract,
        lowering: &SemanticPolicyCallDescriptor,
    ) -> Result<Self, SemanticDescriptorError> {
        if !domain.stamp().same_generation(stamp.domain()) {
            return Err(SemanticDescriptorError::DomainGenerationMismatch);
        }
        Ok(Self {
            stamp,
            coordinate: coordinate.clone(),
            intent: intent.clone(),
            behavior,
            plan,
            lowering: lowering.clone(),
        })
    }

    /// Returns the exact selected Representation policy generation.
    #[must_use]
    pub const fn stamp(&self) -> RepresentationPolicyStamp {
        self.stamp
    }

    /// Returns the selected coordinate-space declaration.
    #[must_use]
    pub const fn coordinate(&self) -> &SemanticCoordinateSpaceDescriptor {
        &self.coordinate
    }

    /// Returns the selected semantic Intent carrier contract.
    #[must_use]
    pub const fn intent(&self) -> &SemanticIntentContract {
        &self.intent
    }

    /// Returns the declared fidelity class.
    #[must_use]
    pub const fn fidelity(&self) -> SemanticFidelity {
        self.behavior.fidelity()
    }

    /// Returns whether edit lowering is available.
    #[must_use]
    pub const fn editability(&self) -> SemanticEditability {
        self.behavior.editability()
    }

    /// Returns the declared source-correspondence class.
    #[must_use]
    pub const fn correspondence(&self) -> SemanticCorrespondence {
        self.behavior.correspondence()
    }

    /// Returns the exact Buffer-plan selections produced by this policy.
    #[must_use]
    pub const fn plan(&self) -> SemanticBufferPlanContract {
        self.plan
    }

    /// Returns the Representation lowering call contract.
    #[must_use]
    pub const fn lowering(&self) -> &SemanticPolicyCallDescriptor {
        &self.lowering
    }
}

fn validate_representation(
    fidelity: SemanticFidelity,
    editability: SemanticEditability,
    correspondence: SemanticCorrespondence,
) -> Result<(), SemanticDescriptorError> {
    if fidelity == SemanticFidelity::DerivedReadOnly
        && editability == SemanticEditability::Transactional
    {
        return Err(SemanticDescriptorError::DerivedEditable);
    }
    if fidelity == SemanticFidelity::ConstrainedEditable
        && editability == SemanticEditability::ReadOnly
    {
        return Err(SemanticDescriptorError::ConstrainedReadOnly);
    }
    if editability == SemanticEditability::Transactional
        && correspondence != SemanticCorrespondence::Bidirectional
    {
        return Err(
            SemanticDescriptorError::TransactionWithoutReverseCorrespondence,
        );
    }
    if fidelity == SemanticFidelity::Faithful
        && correspondence == SemanticCorrespondence::None
    {
        return Err(SemanticDescriptorError::FaithfulWithoutCorrespondence);
    }
    Ok(())
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

const fn same_carrier_stamp(
    left: BufferCarrierStamp,
    right: BufferCarrierStamp,
) -> bool {
    left.type_identity() == right.type_identity()
        && left.schema_version() == right.schema_version()
}
