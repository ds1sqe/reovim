use crate::support::preparation;
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStatePreparation,
    ResourceWorkingState,
};
use reovim_resource_local::{ByteModel, ByteState, profile};
use std::{cell::Cell, rc::Rc};

pub struct Watch {
    pub inside: Cell<bool>,
    pub dropped: Cell<Option<u64>>,
}
pub struct ProbeState {
    pub native: ByteState,
    pub serial: u64,
    pub watch: Rc<Watch>,
}

impl Drop for ProbeState {
    fn drop(&mut self) {
        assert!(!self.watch.inside.get(), "native destructor under authority");
        self.watch.dropped.set(Some(self.serial));
    }
}

pub struct ProbeModel {
    pub base: ByteModel,
    pub next: Cell<u64>,
    pub watch: Rc<Watch>,
}

impl ResourceStateModel for ProbeModel {
    type State = ProbeState;
    fn binding(&self) -> ResourceStateModelBinding {
        self.base.binding()
    }
    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        self.base.plan_descriptor()
    }
    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, ProbeState>,
    ) -> Result<ResourceStateOutput<ProbeState>, ResourceStateError> {
        assert!(!self.watch.inside.get(), "native provider under authority");
        let output = self.native(request)?;
        let serial = self.next.get();
        self.next.set(serial + 1);
        let extra = profile::bytes(size_of::<ProbeState>() as u64);
        let mut usage = output.usage;
        usage.resident_bytes = usage
            .resident_bytes
            .checked_add(extra)
            .map_err(|_| ResourceStateError::Pressure)?;
        Ok(ResourceStateOutput {
            state: ProbeState {
                native: output.state,
                serial,
                watch: self.watch.clone(),
            },
            usage,
        })
    }
}

impl ProbeModel {
    fn native(
        &self,
        request: &ResourceStatePreparation<'_, ProbeState>,
    ) -> Result<ResourceStateOutput<ByteState>, ResourceStateError> {
        match request.input() {
            ResourceStateInput::Initial(observation) => {
                self.base.prepare(&preparation(
                    &self.base,
                    ResourceStateInput::Initial(observation),
                ))
            }
            ResourceStateInput::Edit {
                observed,
                descriptor,
                plan,
                observations,
            } => {
                let observed = unwrapped(observed);
                self.base.prepare(&preparation(
                    &self.base,
                    ResourceStateInput::Edit {
                        observed: &observed,
                        descriptor,
                        plan,
                        observations,
                    },
                ))
            }
            ResourceStateInput::Restore {
                current,
                retained,
                plan,
                direction,
                edge,
            } => {
                let current = unwrapped(current);
                let retained = unwrapped(retained);
                self.base.prepare(&preparation(
                    &self.base,
                    ResourceStateInput::Restore {
                        current: &current,
                        retained: &retained,
                        plan: *plan,
                        direction: *direction,
                        edge: *edge,
                    },
                ))
            }
        }
    }
}

const fn unwrapped<'a>(
    view: &ResourceWorkingState<'a, ProbeState>,
) -> ResourceWorkingState<'a, ByteState> {
    ResourceWorkingState {
        model: view.model,
        stamp: view.stamp,
        state: &view.state.native,
    }
}
