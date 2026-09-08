use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStateModelBinding, ResourceStateOutput, ResourceStatePreparation,
};

use super::models::ByteModel;
use super::support;

struct ChangingDescriptor {
    native: ByteModel,
    foreign: BufferPlanDescriptor,
}

impl ResourceStateModel for ChangingDescriptor {
    type State = Vec<u8>;

    fn binding(&self) -> ResourceStateModelBinding {
        self.native.binding()
    }

    fn plan_descriptor(&self) -> &BufferPlanDescriptor {
        if self.native.entered.get() {
            &self.foreign
        } else {
            self.native.plan_descriptor()
        }
    }

    fn prepare(
        &self,
        request: &ResourceStatePreparation<'_, Self::State>,
    ) -> Result<ResourceStateOutput<Self::State>, ResourceStateError> {
        self.native.prepare(request)
    }
}

#[test]
fn changed_descriptor_after_callback_returns_prepared_output() {
    let native = ByteModel::new();
    let original = native.plan_descriptor();
    let foreign = BufferPlanDescriptor::new(
        support::selection(999),
        original.owner(),
        original.carriers(),
        original.bounds(),
    );
    let model = ChangingDescriptor { native, foreign };
    let state = b"a\xffc".to_vec();
    let observed =
        support::view(&model.binding(), &support::state_stamp(0, 900), &state);
    let plan = support::plan(&observed.stamp, &[0, b'b'], b"a");
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Edit {
            observed: &observed,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
    );
    let rejected = request
        .prepare_with(&model)
        .expect_err("provider changed native semantics during callback");
    assert_eq!(rejected.cause, ResourceStateError::PlanDescriptorMismatch);
    assert!(model.native.entered.get());
    assert_eq!(
        rejected.output.expect("produced state retained").state,
        b"b\xffc"
    );
    assert_eq!(state, b"a\xffc");
}
