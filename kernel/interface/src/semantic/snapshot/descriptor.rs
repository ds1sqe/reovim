use crate::operation::ContractSelection;
use crate::resource::state::ResourceStateModelBinding;

use super::{RepresentationSnapshotLimits, RepresentationSnapshotSchema};
use crate::semantic::descriptor::{
    RepresentationPolicyDescriptor, SemanticFidelity,
    SemanticPolicyCallDescriptor,
};
use crate::semantic::stamp::{CoordinateSpaceStamp, RepresentationPolicyStamp};

/// Read-only interpretation terms, without a Session or delivery contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationSnapshotDescriptor {
    representation: RepresentationPolicyStamp,
    coordinate: CoordinateSpaceStamp,
    state_model: ResourceStateModelBinding,
    fidelity: SemanticFidelity,
    output: ContractSelection,
    call: SemanticPolicyCallDescriptor,
    limits: RepresentationSnapshotLimits,
}

impl RepresentationSnapshotDescriptor {
    /// Selects snapshot terms without requiring transactional editability.
    ///
    /// The output schema defines a complete semantic snapshot encoding,
    /// including a genuine empty-state value. It is not native storage shape.
    #[must_use]
    pub fn new(
        representation: &RepresentationPolicyDescriptor,
        output: ContractSelection,
        call: &SemanticPolicyCallDescriptor,
        limits: RepresentationSnapshotLimits,
    ) -> Self {
        Self {
            representation: representation.stamp(),
            coordinate: representation.coordinate().stamp(),
            state_model: *representation.state_model(),
            fidelity: representation.fidelity(),
            output,
            call: call.clone(),
            limits,
        }
    }

    /// Returns the selected Representation generation.
    #[must_use]
    pub const fn representation(&self) -> RepresentationPolicyStamp {
        self.representation
    }

    /// Returns the selected semantic coordinate-space incarnation.
    #[must_use]
    pub const fn coordinate(&self) -> CoordinateSpaceStamp {
        self.coordinate
    }

    /// Returns the native model and schemas required before interpretation.
    #[must_use]
    pub const fn state_model(&self) -> &ResourceStateModelBinding {
        &self.state_model
    }

    /// Returns the Representation's declared source fidelity.
    #[must_use]
    pub const fn fidelity(&self) -> SemanticFidelity {
        self.fidelity
    }

    /// Returns the complete selected semantic snapshot schema.
    #[must_use]
    pub const fn output(&self) -> ContractSelection {
        self.output
    }

    /// Returns the exact type/version carried by produced payloads.
    #[must_use]
    pub const fn output_schema(&self) -> RepresentationSnapshotSchema {
        RepresentationSnapshotSchema::new(
            self.output.identity(),
            self.output.selected(),
        )
    }

    /// Returns bounded callback execution and reservation terms.
    #[must_use]
    pub const fn call(&self) -> &SemanticPolicyCallDescriptor {
        &self.call
    }

    /// Returns semantic result-shape limits.
    #[must_use]
    pub const fn limits(&self) -> RepresentationSnapshotLimits {
        self.limits
    }
}
