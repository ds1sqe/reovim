use std::cell::Cell;
use std::rc::Rc;

use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::cancellation::{FenceCause, InstallationOutcome};
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStatePreparation,
};

use super::support;

struct RetirementProbe {
    authority: Rc<Cell<bool>>,
    released: Rc<Cell<bool>>,
}

impl Drop for RetirementProbe {
    fn drop(&mut self) {
        assert!(!self.authority.get(), "native destructor under authority");
        self.released.set(true);
    }
}

struct ProbeModel {
    authority: Rc<Cell<bool>>,
    released: Rc<Cell<bool>>,
    descriptor: BufferPlanDescriptor,
}

impl ResourceStateModel for ProbeModel {
    type State = RetirementProbe;

    fn binding(&self) -> ResourceStateModelBinding {
        support::binding(921)
    }

    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        &self.descriptor
    }

    fn prepare(
        &self,
        _: &ResourceStatePreparation<'_, Self::State>,
    ) -> Result<ResourceStateOutput<Self::State>, ResourceStateError> {
        Ok(ResourceStateOutput {
            state: RetirementProbe {
                authority: Rc::clone(&self.authority),
                released: Rc::clone(&self.released),
            },
            usage: support::usage(64, 1),
        })
    }
}

#[test]
fn scalar_admission_does_not_release_native_state_under_authority() {
    let authority = Rc::new(Cell::new(false));
    let released = Rc::new(Cell::new(false));
    let model = ProbeModel {
        authority: Rc::clone(&authority),
        released: Rc::clone(&released),
        descriptor: support::descriptors().1,
    };
    let observation = support::observation(b"probe");
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Initial(&observation),
    );
    let candidate = request.prepare_with(&model).expect("native probe state");
    let binding = model.binding();
    authority.set(true);
    assert_eq!(
        candidate.check_admission(
            binding,
            request.stamp(),
            request.stamp().target,
            request.limits(),
            InstallationOutcome::Fenced(FenceCause::Closing),
        ),
        Err(ResourceStateError::Fenced(FenceCause::Closing))
    );
    let output = candidate.into_output();
    assert!(!released.get());
    authority.set(false);
    drop(output);
    assert!(released.get());
}
