use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::descriptor::DomainPolicyDescriptor;
use reovim_kernel_interface::semantic::domain::{
    DomainPolicyPort, DomainValidationCandidate, DomainValidationRequest,
};
use reovim_kernel_interface::semantic::identity::DomainIdentity;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::stamp::DomainPolicyStamp;
use reovim_resource_local::{ByteModel, ByteState, profile};

/// Parsed single-letter heading in the deliberately constrained first profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heading {
    /// Heading level supported by this profile: one or two.
    pub level: u8,
    /// Native byte position of the one editable label unit.
    pub offset: u64,
    /// Exact current ASCII alphabetic label.
    pub label: u8,
}

/// Meaning/validation for a first heading without normalizing trailing bytes.
pub struct HeadingDomain {
    descriptor: DomainPolicyDescriptor,
}

impl HeadingDomain {
    /// Binds heading meaning to the exact selected native model generation.
    #[must_use]
    pub fn new(model: &ByteModel) -> Self {
        let binding = model.binding();
        let stamp = DomainPolicyStamp::new(
            DomainIdentity::new(0x4000),
            profile::schema(0x4001),
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

impl DomainPolicyPort for HeadingDomain {
    type State = ByteState;

    fn descriptor(&self) -> &DomainPolicyDescriptor {
        &self.descriptor
    }

    fn validate(
        &self,
        request: DomainValidationRequest<'_, ByteState>,
    ) -> DomainValidationCandidate {
        let result = replacement(request.intent().bytes())
            .and_then(|_| heading(request.state().state));
        match result {
            Ok(_) => request.validated_candidate(),
            Err(failure) => DomainValidationCandidate::rejected(failure),
        }
    }
}

/// Parses only this profile's first heading; unrelated bytes stay opaque.
///
/// # Errors
/// Returns no correspondence for absent headings, or unrepresentable for a
/// matching heading whose title cannot be expressed by this constrained view.
pub fn heading(state: &ByteState) -> Result<Heading, SemanticPolicyFailure> {
    let (level, offset, label) = match state.as_bytes() {
        [b'#', b' ', label, b'\n', ..] => (1, 2, *label),
        [b'#', b'#', b' ', label, b'\n', ..] => (2, 3, *label),
        _ => return Err(SemanticPolicyFailure::NoCorrespondence),
    };
    if !label.is_ascii_alphabetic() {
        return Err(SemanticPolicyFailure::Unrepresentable);
    }
    Ok(Heading { level, offset, label })
}

/// Decodes the version-one semantic instruction: one replacement title unit.
///
/// # Errors
/// Rejects malformed instructions and non-ASCII-alphabetic replacements.
pub const fn replacement(intent: &[u8]) -> Result<u8, SemanticPolicyFailure> {
    let [value] = intent else {
        return Err(SemanticPolicyFailure::InvalidIntent);
    };
    if !value.is_ascii_alphabetic() {
        return Err(SemanticPolicyFailure::Unrepresentable);
    }
    Ok(*value)
}
