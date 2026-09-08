use alloc::boxed::Box;

use reovim_domain_basic::{
    HeadingDomain, KeyedDomain, heading, replacement, selected_key,
};
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierRole, BufferPlanCarrier,
};
use reovim_kernel_interface::buffer::plan::{
    BufferPlanBindingOutcome, BufferPlanDescriptor, BufferTransactionPlan,
};
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateModel,
};
use reovim_kernel_interface::semantic::{
    descriptor::RepresentationPolicyDescriptor,
    domain::DomainPolicyPort,
    policy::SemanticPolicyFailure,
    representation::{
        RepresentationLoweringCandidate, RepresentationLoweringRequest,
        RepresentationPolicyPort,
    },
};
use reovim_resource_local::{ByteModel, ByteState, KeyedModel, KeyedState};

/// Interprets a heading label instruction as a conditional native replacement.
pub struct HeadingRepresentation {
    descriptor: RepresentationPolicyDescriptor,
}

impl HeadingRepresentation {
    /// Selects heading lowering with the supplied Domain and native model.
    ///
    /// # Errors
    /// Rejects a Domain bound to another native model generation.
    ///
    /// # Panics
    /// Only if fixed reference-profile declarations become incoherent.
    pub fn new(
        domain: &HeadingDomain,
        model: &ByteModel,
    ) -> Result<Self, ResourceStateError> {
        if *domain.descriptor().state_model() != model.binding() {
            return Err(ResourceStateError::ModelMismatch);
        }
        Ok(Self {
            descriptor: super::descriptor::build(
                0x6000,
                domain.descriptor(),
                model.plan_descriptor(),
            ),
        })
    }
}

impl RepresentationPolicyPort for HeadingRepresentation {
    type State = ByteState;

    fn descriptor(&self) -> &RepresentationPolicyDescriptor {
        &self.descriptor
    }

    fn lower(
        &self,
        request: RepresentationLoweringRequest<'_, ByteState>,
    ) -> RepresentationLoweringCandidate {
        lower_heading(request).map_or_else(
            RepresentationLoweringCandidate::rejected,
            RepresentationLoweringCandidate::from,
        )
    }
}

fn lower_heading(
    request: RepresentationLoweringRequest<'_, ByteState>,
) -> Result<BufferTransactionPlan, SemanticPolicyFailure> {
    let replacement = replacement(request.intent().bytes())?;
    let current = heading(request.state().state)?;
    let mut operation = [0_u8; 9];
    operation[..8].copy_from_slice(&current.offset.to_le_bytes());
    operation[8] = replacement;
    bind(request, &operation, current.label)
}

/// Interprets a semantic toggle using the current value of a named record.
pub struct KeyedRepresentation {
    descriptor: RepresentationPolicyDescriptor,
}

impl KeyedRepresentation {
    /// Selects keyed lowering with the supplied Domain and native model.
    ///
    /// # Errors
    /// Rejects a Domain bound to another native model generation.
    ///
    /// # Panics
    /// Only if fixed reference-profile declarations become incoherent.
    pub fn new(
        domain: &KeyedDomain,
        model: &KeyedModel,
    ) -> Result<Self, ResourceStateError> {
        if *domain.descriptor().state_model() != model.binding() {
            return Err(ResourceStateError::ModelMismatch);
        }
        Ok(Self {
            descriptor: super::descriptor::build(
                0x7000,
                domain.descriptor(),
                model.plan_descriptor(),
            ),
        })
    }
}

impl RepresentationPolicyPort for KeyedRepresentation {
    type State = KeyedState;

    fn descriptor(&self) -> &RepresentationPolicyDescriptor {
        &self.descriptor
    }

    fn lower(
        &self,
        request: RepresentationLoweringRequest<'_, KeyedState>,
    ) -> RepresentationLoweringCandidate {
        lower_keyed(request).map_or_else(
            RepresentationLoweringCandidate::rejected,
            RepresentationLoweringCandidate::from,
        )
    }
}

fn lower_keyed(
    request: RepresentationLoweringRequest<'_, KeyedState>,
) -> Result<BufferTransactionPlan, SemanticPolicyFailure> {
    let (key, current) =
        selected_key(request.state().state, request.intent().bytes())?;
    bind(request, &[key.tag(), u8::from(!current)], u8::from(current))
}

fn bind<State>(
    request: RepresentationLoweringRequest<'_, State>,
    operations: &[u8],
    expected: u8,
) -> Result<BufferTransactionPlan, SemanticPolicyFailure> {
    let descriptor = request.plan();
    let origin = carrier(descriptor, BufferCarrierRole::Origin, &[1])?;
    let operations =
        carrier(descriptor, BufferCarrierRole::Operations, operations)?;
    let preconditions =
        carrier(descriptor, BufferCarrierRole::Preconditions, &[expected])?;
    match descriptor.bind_plan(
        *request.intent().stamp().plan(),
        request.state().stamp,
        origin,
        operations,
        preconditions,
    ) {
        BufferPlanBindingOutcome::Accepted(plan) => Ok(plan),
        BufferPlanBindingOutcome::Rejected(_) => {
            Err(SemanticPolicyFailure::ProviderFailure)
        }
    }
}

fn carrier(
    descriptor: &BufferPlanDescriptor,
    role: BufferCarrierRole,
    bytes: &[u8],
) -> Result<BufferPlanCarrier, SemanticPolicyFailure> {
    let contract = match role {
        BufferCarrierRole::Origin => descriptor.carriers().origin(),
        BufferCarrierRole::Operations => descriptor.carriers().operations(),
        BufferCarrierRole::Preconditions => {
            descriptor.carriers().preconditions()
        }
    };
    let size = u64::try_from(bytes.len())
        .map_err(|_| SemanticPolicyFailure::Pressure)?;
    let prepared = descriptor
        .prepare_carrier(role, contract.selected_stamp(), 1, size)
        .map_err(|_| SemanticPolicyFailure::Pressure)?;
    prepared
        .bind(descriptor, Box::from(bytes))
        .map_err(|_| SemanticPolicyFailure::ProviderFailure)
}
