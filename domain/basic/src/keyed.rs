use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::descriptor::DomainPolicyDescriptor;
use reovim_kernel_interface::semantic::domain::{
    DomainPolicyPort, DomainValidationCandidate, DomainValidationRequest,
};
use reovim_kernel_interface::semantic::identity::DomainIdentity;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::stamp::DomainPolicyStamp;
use reovim_resource_local::{Key, KeyedModel, KeyedState, profile};

/// Meaning of a toggle on an existing named native record.
pub struct KeyedDomain {
    descriptor: DomainPolicyDescriptor,
}

impl KeyedDomain {
    /// Binds keyed meaning to the exact selected native model generation.
    #[must_use]
    pub fn new(model: &KeyedModel) -> Self {
        let binding = model.binding();
        let stamp = DomainPolicyStamp::new(
            DomainIdentity::new(0x5000),
            profile::schema(0x5001),
            binding.lifecycle,
        );
        Self {
            descriptor: DomainPolicyDescriptor::new(
                stamp,
                &crate::profile::call(binding.lifecycle.instance()),
                &binding,
            ),
        }
    }
}

impl DomainPolicyPort for KeyedDomain {
    type State = KeyedState;

    fn descriptor(&self) -> &DomainPolicyDescriptor {
        &self.descriptor
    }

    fn validate(
        &self,
        request: DomainValidationRequest<'_, KeyedState>,
    ) -> DomainValidationCandidate {
        match selected_key(request.state().state, request.intent().bytes()) {
            Ok(_) => request.validated_candidate(),
            Err(failure) => DomainValidationCandidate::rejected(failure),
        }
    }
}

/// Decodes a semantic record identity and reads its actual current value.
///
/// # Errors
/// Rejects unknown tags and instructions naming an absent record.
pub const fn selected_key(
    state: &KeyedState,
    intent: &[u8],
) -> Result<(Key, bool), SemanticPolicyFailure> {
    let [tag] = intent else {
        return Err(SemanticPolicyFailure::InvalidIntent);
    };
    let Some(key) = Key::from_tag(*tag) else {
        return Err(SemanticPolicyFailure::InvalidIntent);
    };
    let Some(current) = state.get(key) else {
        return Err(SemanticPolicyFailure::NoCorrespondence);
    };
    Ok((key, current))
}
