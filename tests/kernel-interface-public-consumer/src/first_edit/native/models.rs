use alloc::{collections::BTreeMap, vec::Vec};
use core::cell::Cell;

use reovim_kernel_interface::buffer::plan::{
    BufferPlanDescriptor, BufferTransactionPlan,
};
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStatePreparation,
    ResourceStateUsage,
};

use super::support;

/// Deterministic byte-addressed model, independent of semantic heading policy.
pub struct ByteModel {
    pub binding: ResourceStateModelBinding,
    pub entered: Cell<bool>,
    pub usage: ResourceStateUsage,
    pub descriptor: BufferPlanDescriptor,
}

impl ByteModel {
    /// Constructs the model with the declared fixture schemas and charges.
    #[must_use]
    pub fn new() -> Self {
        Self {
            binding: support::binding(901),
            entered: Cell::new(false),
            usage: support::usage(32, 8),
            descriptor: support::descriptors().1,
        }
    }
}

impl Default for ByteModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceStateModel for ByteModel {
    type State = Vec<u8>;

    fn binding(&self) -> ResourceStateModelBinding {
        self.binding
    }

    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        &self.descriptor
    }

    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, Self::State>,
    ) -> Result<ResourceStateOutput<Self::State>, ResourceStateError> {
        self.entered.set(true);
        let state = prepare_bytes(request.input())?;
        Ok(ResourceStateOutput { state, usage: self.usage })
    }
}

fn prepare_bytes(
    input: &ResourceStateInput<'_, Vec<u8>>,
) -> Result<Vec<u8>, ResourceStateError> {
    match input {
        ResourceStateInput::Initial(observed) => {
            Ok(observed.payload().bytes().to_vec())
        }
        ResourceStateInput::Edit { observed, plan, .. } => {
            replace_byte(observed.state, plan)
        }
        ResourceStateInput::Restore { retained, .. } => {
            Ok(retained.state.clone())
        }
    }
}

fn replace_byte(
    observed: &[u8],
    plan: &BufferTransactionPlan,
) -> Result<Vec<u8>, ResourceStateError> {
    let [offset, replacement] = plan.operations().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    let [expected] = plan.preconditions().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    if observed.get(usize::from(*offset)) != Some(expected) {
        return Err(ResourceStateError::PreconditionFailed);
    }
    let mut next = observed.to_vec();
    next[usize::from(*offset)] = *replacement;
    Ok(next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Alpha,
    Omega,
}

pub type KeyedState = BTreeMap<Key, bool>;

/// Deterministic non-positional model preserving independently keyed values.
pub struct KeyedModel {
    descriptor: BufferPlanDescriptor,
}

impl KeyedModel {
    /// Constructs a model using the selected native plan contract.
    #[must_use]
    pub fn new() -> Self {
        Self { descriptor: support::descriptors().1 }
    }
}

impl Default for KeyedModel {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceStateModel for KeyedModel {
    type State = KeyedState;

    fn binding(&self) -> ResourceStateModelBinding {
        support::binding(911)
    }

    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        &self.descriptor
    }

    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, Self::State>,
    ) -> Result<ResourceStateOutput<Self::State>, ResourceStateError> {
        let state = prepare_keys(request.input())?;
        Ok(ResourceStateOutput { state, usage: support::usage(128, 8) })
    }
}

fn prepare_keys(
    input: &ResourceStateInput<'_, KeyedState>,
) -> Result<KeyedState, ResourceStateError> {
    match input {
        ResourceStateInput::Initial(observed) => {
            initial_keys(observed.payload().bytes())
        }
        ResourceStateInput::Edit { observed, plan, .. } => {
            replace_key(observed.state, plan)
        }
        ResourceStateInput::Restore { retained, .. } => {
            Ok(retained.state.clone())
        }
    }
}

fn initial_keys(payload: &[u8]) -> Result<KeyedState, ResourceStateError> {
    // This fixture encoding admits an Omega-only resource too.
    match payload {
        [alpha, omega] => Ok(BTreeMap::from([
            (Key::Alpha, enabled(*alpha)?),
            (Key::Omega, enabled(*omega)?),
        ])),
        [omega] => Ok(BTreeMap::from([(Key::Omega, enabled(*omega)?)])),
        _ => Err(ResourceStateError::MalformedOperation),
    }
}

fn replace_key(
    observed: &KeyedState,
    plan: &BufferTransactionPlan,
) -> Result<KeyedState, ResourceStateError> {
    let [key, replacement] = plan.operations().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    let key = match key {
        17 => Key::Alpha,
        93 => Key::Omega,
        _ => return Err(ResourceStateError::MalformedOperation),
    };
    let [expected] = plan.preconditions().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    if observed.get(&key) != Some(&enabled(*expected)?) {
        return Err(ResourceStateError::PreconditionFailed);
    }
    let mut next = observed.clone();
    next.insert(key, enabled(*replacement)?);
    Ok(next)
}

const fn enabled(value: u8) -> Result<bool, ResourceStateError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ResourceStateError::MalformedOperation),
    }
}
