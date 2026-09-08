//! Immutable actual state accompanying a semantic policy intent.

use crate::resource::state::{ResourceStateModelBinding, ResourceWorkingState};

use super::intent::SemanticIntent;
use super::policy::SemanticPolicyContractError;

/// Runtime-retained semantic input; neither reference grants mutation access.
///
/// State may be a sparse/persistent native root or a public read interface.
/// No concrete storage, text position, contiguous region or cloning contract
/// is prescribed. Both policy requests validate this pair before invocation.
pub struct SemanticPolicyInput<'a, State> {
    /// Exact retained intent and its observed target.
    pub intent: &'a SemanticIntent,
    /// Actual immutable data matching that target, not another revision token.
    pub state: &'a ResourceWorkingState<'a, State>,
}

impl<State> Copy for SemanticPolicyInput<'_, State> {}

impl<State> Clone for SemanticPolicyInput<'_, State> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<State> SemanticPolicyInput<'_, State> {
    pub(crate) fn validate(
        self,
        expected: &ResourceStateModelBinding,
    ) -> Result<(), SemanticPolicyContractError> {
        validate_working_state(expected, self.state)?;
        if self.state.stamp != *self.intent.stamp().target() {
            return Err(
                SemanticPolicyContractError::WorkingStateTargetMismatch,
            );
        }
        Ok(())
    }
}

pub(crate) fn validate_working_state<State>(
    expected: &ResourceStateModelBinding,
    state: &ResourceWorkingState<'_, State>,
) -> Result<(), SemanticPolicyContractError> {
    if state.model != *expected
        || state.stamp.source().revision().schema()
            != expected.schemas.revision()
    {
        return Err(SemanticPolicyContractError::WorkingStateModelMismatch);
    }
    Ok(())
}
