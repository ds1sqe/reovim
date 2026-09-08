use reovim_kernel_interface::buffer::plan::{
    BufferPlanDescriptor, BufferTransactionPlan,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStatePreparation,
    ResourceStateUsage,
};

use crate::{native, profile};

/// Non-positional native identities, independent of byte offsets or order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// First named record in the bounded reference family.
    Alpha,
    /// Independently named second record.
    Omega,
}

impl Key {
    /// Decodes the version-one record identity, not an ordinal index.
    #[must_use]
    pub const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            17 => Some(Self::Alpha),
            93 => Some(Self::Omega),
            _ => None,
        }
    }

    /// Encodes the stable native record identity.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Alpha => 17,
            Self::Omega => 93,
        }
    }
}

/// Sparse, keyed native state. Absent and disabled are different values.
#[derive(Debug, PartialEq, Eq)]
pub struct KeyedState {
    alpha: Option<bool>,
    omega: Option<bool>,
}

impl KeyedState {
    /// Reads a named record without implying any native byte address.
    #[must_use]
    pub const fn get(&self, key: Key) -> Option<bool> {
        match key {
            Key::Alpha => self.alpha,
            Key::Omega => self.omega,
        }
    }

    const fn reconstruct(&self) -> Self {
        Self { alpha: self.alpha, omega: self.omega }
    }

    const fn set(&mut self, key: Key, value: bool) {
        match key {
            Key::Alpha => self.alpha = Some(value),
            Key::Omega => self.omega = Some(value),
        }
    }
}

/// Native boolean records with checked conditional replacement.
///
/// Version one initial payload consists of unique `[key_tag, enabled]` pairs,
/// in any order; enabled is zero or one. Empty and sparse maps are valid.
/// A replacement operation is `[key_tag, enabled]`, with one old enabled byte
/// as its precondition. Record identity is never inferred from pair position.
pub struct KeyedModel {
    binding: ResourceStateModelBinding,
    plan: BufferPlanDescriptor,
}

impl KeyedModel {
    /// Selects the bounded keyed family under assembly-owned lifecycles.
    ///
    /// # Panics
    /// Only if fixed reference-profile declarations become incoherent.
    #[must_use]
    pub fn new(
        owner: LifecycleIdentity,
        plan_owner: LifecycleIdentity,
    ) -> Self {
        Self {
            binding: native::binding(0x2000, owner),
            plan: native::plan(0x2000, plan_owner),
        }
    }
}

impl ResourceStateModel for KeyedModel {
    type State = KeyedState;

    fn binding(&self) -> ResourceStateModelBinding {
        self.binding
    }

    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        &self.plan
    }

    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, KeyedState>,
    ) -> Result<ResourceStateOutput<KeyedState>, ResourceStateError> {
        if request.limits().resident_bytes.value()
            < size_of::<KeyedState>() as u64
            || request.limits().work_units.value() < 4
        {
            return Err(ResourceStateError::Pressure);
        }
        let state = match request.input() {
            ResourceStateInput::Initial(observed) => {
                decode(observed.payload().bytes())?
            }
            ResourceStateInput::Edit { observed, plan, .. } => {
                replace(observed.state, plan)?
            }
            ResourceStateInput::Restore { retained, .. } => {
                retained.state.reconstruct()
            }
        };
        Ok(ResourceStateOutput {
            state,
            usage: ResourceStateUsage {
                resident_bytes: profile::bytes(size_of::<KeyedState>() as u64),
                work_units: profile::count(4),
            },
        })
    }
}

fn decode(payload: &[u8]) -> Result<KeyedState, ResourceStateError> {
    if payload.len() > 4 || !payload.len().is_multiple_of(2) {
        return Err(ResourceStateError::MalformedOperation);
    }
    let mut state = KeyedState { alpha: None, omega: None };
    for pair in payload.as_chunks::<2>().0 {
        let key = Key::from_tag(pair[0])
            .ok_or(ResourceStateError::MalformedOperation)?;
        if state.get(key).is_some() {
            return Err(ResourceStateError::MalformedOperation);
        }
        state.set(key, enabled(pair[1])?);
    }
    Ok(state)
}

fn replace(
    state: &KeyedState,
    plan: &BufferTransactionPlan,
) -> Result<KeyedState, ResourceStateError> {
    if plan.preconditions().items().value() != 1 {
        return Err(ResourceStateError::MalformedOperation);
    }
    let [tag, replacement] = plan.operations().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    let key =
        Key::from_tag(*tag).ok_or(ResourceStateError::MalformedOperation)?;
    let [expected] = plan.preconditions().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    if state.get(key) != Some(enabled(*expected)?) {
        return Err(ResourceStateError::PreconditionFailed);
    }
    let mut next = state.reconstruct();
    next.set(key, enabled(*replacement)?);
    Ok(next)
}

const fn enabled(value: u8) -> Result<bool, ResourceStateError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ResourceStateError::MalformedOperation),
    }
}
