use alloc::{boxed::Box, vec};

use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;

use super::native::models::{ByteModel, Key, KeyedModel, KeyedState};

pub struct NativeEdit {
    pub operations: Box<[u8]>,
    pub preconditions: Box<[u8]>,
}

/// Concrete fixture semantics; none of these meanings are Kernel types.
pub trait Family: ResourceStateModel {
    fn validate(
        &self,
        state: &Self::State,
        intent: &[u8],
    ) -> Result<(), SemanticPolicyFailure>;

    fn lower(
        &self,
        state: &Self::State,
        intent: &[u8],
    ) -> Result<NativeEdit, SemanticPolicyFailure>;

    fn preview(
        &self,
        state: &Self::State,
    ) -> Result<Box<[u8]>, SemanticPolicyFailure>;
}

impl Family for ByteModel {
    fn validate(
        &self,
        state: &Self::State,
        intent: &[u8],
    ) -> Result<(), SemanticPolicyFailure> {
        replacement(intent)?;
        heading(state)?;
        Ok(())
    }

    fn lower(
        &self,
        state: &Self::State,
        intent: &[u8],
    ) -> Result<NativeEdit, SemanticPolicyFailure> {
        let replacement = replacement(intent)?;
        let (_, offset, old) = heading(state)?;
        Ok(NativeEdit {
            operations: vec![offset, replacement].into_boxed_slice(),
            preconditions: vec![old].into_boxed_slice(),
        })
    }

    fn preview(
        &self,
        state: &Self::State,
    ) -> Result<Box<[u8]>, SemanticPolicyFailure> {
        let (level, _, label) = heading(state)?;
        Ok(vec![b'H', level, label].into_boxed_slice())
    }
}

const fn replacement(intent: &[u8]) -> Result<u8, SemanticPolicyFailure> {
    let [replacement] = intent else {
        return Err(SemanticPolicyFailure::InvalidIntent);
    };
    if !replacement.is_ascii_alphabetic() {
        return Err(SemanticPolicyFailure::Unrepresentable);
    }
    Ok(*replacement)
}

const fn heading(state: &[u8]) -> Result<(u8, u8, u8), SemanticPolicyFailure> {
    let result = match state {
        [b'#', b' ', label, b'\n', ..] => (1, 2, *label),
        [b'#', b'#', b' ', label, b'\n', ..] => (2, 3, *label),
        _ => return Err(SemanticPolicyFailure::NoCorrespondence),
    };
    if !result.2.is_ascii_alphabetic() {
        return Err(SemanticPolicyFailure::Unrepresentable);
    }
    Ok(result)
}

impl Family for KeyedModel {
    fn validate(
        &self,
        state: &Self::State,
        intent: &[u8],
    ) -> Result<(), SemanticPolicyFailure> {
        selected_key(state, intent)?;
        Ok(())
    }

    fn lower(
        &self,
        state: &Self::State,
        intent: &[u8],
    ) -> Result<NativeEdit, SemanticPolicyFailure> {
        let (tag, current) = selected_key(state, intent)?;
        Ok(NativeEdit {
            operations: vec![tag, u8::from(!current)].into_boxed_slice(),
            preconditions: vec![u8::from(current)].into_boxed_slice(),
        })
    }

    fn preview(
        &self,
        state: &Self::State,
    ) -> Result<Box<[u8]>, SemanticPolicyFailure> {
        let mut bytes = vec![b'K'];
        for (key, enabled) in state {
            bytes.push(match key {
                Key::Alpha => 17,
                Key::Omega => 93,
            });
            bytes.push(u8::from(*enabled));
        }
        Ok(bytes.into_boxed_slice())
    }
}

fn selected_key(
    state: &KeyedState,
    intent: &[u8],
) -> Result<(u8, bool), SemanticPolicyFailure> {
    let (key, tag) = match intent {
        [17] => (Key::Alpha, 17),
        [93] => (Key::Omega, 93),
        _ => return Err(SemanticPolicyFailure::InvalidIntent),
    };
    let value =
        state.get(&key).ok_or(SemanticPolicyFailure::NoCorrespondence)?;
    Ok((tag, *value))
}
