use reovim_kernel_interface::buffer::carrier::BufferCarrierRole;
use reovim_kernel_interface::buffer::plan::{
    BufferPlanBindingOutcome, BufferTransactionPlan,
};
use reovim_kernel_interface::semantic::descriptor::{
    DomainPolicyDescriptor, RepresentationPolicyDescriptor,
};
use reovim_kernel_interface::semantic::domain::{
    DomainPolicyPort, DomainValidationCandidate, DomainValidationRequest,
};
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::representation::{
    RepresentationLoweringCandidate, RepresentationLoweringRequest,
    RepresentationPolicyPort,
};

use super::families::{Family, NativeEdit};
use crate::semantic::support::SemanticFixture;

pub struct Policies<'a, Model> {
    pub fixture: &'a SemanticFixture,
    pub model: &'a Model,
}

impl<Model: Family> DomainPolicyPort for Policies<'_, Model> {
    type State = Model::State;

    fn descriptor(&self) -> &DomainPolicyDescriptor {
        &self.fixture.domain
    }

    fn validate(
        &self,
        request: DomainValidationRequest<'_, Self::State>,
    ) -> DomainValidationCandidate {
        match self
            .model
            .validate(request.state().state, request.intent().bytes())
        {
            Ok(()) => request.validated_candidate(),
            Err(cause) => DomainValidationCandidate::rejected(cause),
        }
    }
}

impl<Model: Family> RepresentationPolicyPort for Policies<'_, Model> {
    type State = Model::State;

    fn descriptor(&self) -> &RepresentationPolicyDescriptor {
        &self.fixture.representation
    }

    fn lower(
        &self,
        request: RepresentationLoweringRequest<'_, Self::State>,
    ) -> RepresentationLoweringCandidate {
        lower(self.model, request).map_or_else(
            RepresentationLoweringCandidate::rejected,
            RepresentationLoweringCandidate::from,
        )
    }
}

fn lower<Model: Family>(
    model: &Model,
    request: RepresentationLoweringRequest<'_, Model::State>,
) -> Result<BufferTransactionPlan, SemanticPolicyFailure> {
    let native =
        model.lower(request.state().state, request.intent().bytes())?;
    bind_plan(request, &native).ok_or(SemanticPolicyFailure::Pressure)
}

fn bind_plan<State>(
    request: RepresentationLoweringRequest<'_, State>,
    native: &NativeEdit,
) -> Option<BufferTransactionPlan> {
    let descriptor = request.plan();
    let origin = crate::buffer::carrier::make_carrier(
        descriptor,
        BufferCarrierRole::Origin,
        1,
        b"C2",
    )?;
    let operations = crate::buffer::carrier::make_carrier(
        descriptor,
        BufferCarrierRole::Operations,
        1,
        &native.operations,
    )?;
    let preconditions = crate::buffer::carrier::make_carrier(
        descriptor,
        BufferCarrierRole::Preconditions,
        1,
        &native.preconditions,
    )?;
    match descriptor.bind_plan(
        *request.intent().stamp().plan(),
        request.state().stamp,
        origin,
        operations,
        preconditions,
    ) {
        BufferPlanBindingOutcome::Accepted(plan) => Some(plan),
        BufferPlanBindingOutcome::Rejected(_) => None,
    }
}
