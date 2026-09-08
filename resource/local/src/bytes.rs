use alloc::vec::Vec;

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

/// Arbitrary bytes; invalid text units remain native source data.
#[derive(Debug, PartialEq, Eq)]
pub struct ByteState(Vec<u8>);

impl ByteState {
    /// Borrows exact native bytes without text normalization.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Byte-native replacement model; offsets never enter the neutral Kernel.
///
/// Version one encodes one little-endian `u64` offset followed by the new byte,
/// with exactly one old byte as its precondition. Initial source is arbitrary
/// bytes within [`crate::MAX_SOURCE_BYTES`]; no text validity is assumed.
pub struct ByteModel {
    binding: ResourceStateModelBinding,
    plan: BufferPlanDescriptor,
}

impl ByteModel {
    /// Selects the bounded byte family under assembly-owned lifecycles.
    ///
    /// # Panics
    /// Only if fixed reference-profile declarations become incoherent.
    #[must_use]
    pub fn new(
        owner: LifecycleIdentity,
        plan_owner: LifecycleIdentity,
    ) -> Self {
        Self {
            binding: native::binding(0x1000, owner),
            plan: native::plan(0x1000, plan_owner),
        }
    }
}

impl ResourceStateModel for ByteModel {
    type State = ByteState;

    fn binding(&self) -> ResourceStateModelBinding {
        self.binding
    }

    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        &self.plan
    }

    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, ByteState>,
    ) -> Result<ResourceStateOutput<ByteState>, ResourceStateError> {
        check_reservation(request)?;
        let bytes = match request.input() {
            ResourceStateInput::Initial(observed) => {
                native::copy_bytes(observed.payload().bytes())?
            }
            ResourceStateInput::Edit { observed, plan, .. } => {
                replace(observed.state.as_bytes(), plan)?
            }
            ResourceStateInput::Restore { retained, .. } => {
                native::copy_bytes(retained.state.as_bytes())?
            }
        };
        let resident = bytes
            .capacity()
            .checked_add(size_of::<ByteState>())
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(ResourceStateError::Pressure)?;
        let work = u64::try_from(bytes.len())
            .map_err(|_| ResourceStateError::Pressure)?;
        Ok(ResourceStateOutput {
            state: ByteState(bytes),
            usage: ResourceStateUsage {
                resident_bytes: profile::bytes(resident),
                work_units: profile::count(work.saturating_add(1)),
            },
        })
    }
}

fn check_reservation(
    request: &ResourceStatePreparation<'_, ByteState>,
) -> Result<(), ResourceStateError> {
    let source = match request.input() {
        ResourceStateInput::Initial(observed) => observed.payload().bytes(),
        ResourceStateInput::Edit { observed, .. } => observed.state.as_bytes(),
        ResourceStateInput::Restore { retained, .. } => {
            retained.state.as_bytes()
        }
    };
    let source_bytes = u64::try_from(source.len())
        .map_err(|_| ResourceStateError::Pressure)?;
    let resident = source_bytes
        .checked_add(size_of::<ByteState>() as u64)
        .ok_or(ResourceStateError::Pressure)?;
    let work =
        source_bytes.checked_add(1).ok_or(ResourceStateError::Pressure)?;
    if resident > request.limits().resident_bytes.value()
        || work > request.limits().work_units.value()
    {
        return Err(ResourceStateError::Pressure);
    }
    Ok(())
}

fn replace(
    source: &[u8],
    plan: &BufferTransactionPlan,
) -> Result<Vec<u8>, ResourceStateError> {
    if plan.preconditions().items().value() != 1 {
        return Err(ResourceStateError::MalformedOperation);
    }
    let encoded = plan.operations().bytes();
    if encoded.len() != 9 {
        return Err(ResourceStateError::MalformedOperation);
    }
    let offset = encoded[..8]
        .try_into()
        .map_err(|_| ResourceStateError::MalformedOperation)?;
    let offset = usize::try_from(u64::from_le_bytes(offset))
        .map_err(|_| ResourceStateError::MalformedOperation)?;
    let [expected] = plan.preconditions().bytes() else {
        return Err(ResourceStateError::MalformedOperation);
    };
    if source.get(offset) != Some(expected) {
        return Err(ResourceStateError::PreconditionFailed);
    }
    let mut next = native::copy_bytes(source)?;
    next[offset] = encoded[8];
    Ok(next)
}
