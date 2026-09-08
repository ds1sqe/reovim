use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::buffer::{
    BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationState, DeadlineState, FenceCause,
    InstallationOutcome,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::resource::carrier::ResourceCarrierStamp;
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStatePreparation, ResourceStateTarget,
};

use super::models::ByteModel;
use super::support;

#[test]
fn changed_model_identity_schema_or_generation_never_invokes_provider() {
    let observation = support::observation(b"a\xffc");
    let model = ByteModel::new();
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Initial(&observation),
    );
    let mut identity = model.binding();
    identity.contract = support::selection(999);
    let mut schema = model.binding();
    schema.contract =
        reovim_kernel_interface::operation::ContractSelection::new(
            schema.contract.identity(),
            2,
            2,
            2,
        )
        .expect("new schema");
    let mut generation = model.binding();
    generation.lifecycle = LifecycleIdentity::new(901, 1, Generation::new(2));
    for binding in [identity, schema, generation] {
        let provider = ByteModel { binding, ..ByteModel::new() };
        let rejected =
            request.prepare_with(&provider).expect_err("model fence");
        assert_eq!(rejected.cause, ResourceStateError::ModelMismatch);
        assert!(rejected.output.is_none());
        assert!(!provider.entered.get());
    }
}

#[test]
fn wrong_observation_payload_schema_rejects_before_native_decoding() {
    let observation = support::observation(b"a\xffc");
    let mut model = ByteModel::new();
    model.binding.payload = ResourceCarrierStamp::new(999, 1);
    let result = ResourceStatePreparation::<Vec<u8>>::new(
        model.binding(),
        ResourceStateInput::Initial(&observation),
        support::limits(128, 8),
        CancellationState::Open,
        DeadlineState::Pending,
    );
    assert_eq!(result.err(), Some(ResourceStateError::PayloadSchemaMismatch));
    assert!(!model.entered.get());
}

#[test]
fn caller_cannot_select_a_foreign_native_plan_descriptor() {
    let model = ByteModel::new();
    let data = b"a\xffc".to_vec();
    let view =
        support::view(&model.binding(), &support::state_stamp(0, 900), &data);
    let plan = support::plan(&view.stamp, &[0, b'b'], b"a");
    let own = model.plan_descriptor();
    let foreign = BufferPlanDescriptor::new(
        support::selection(999),
        own.owner(),
        own.carriers(),
        own.bounds(),
    );
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Edit {
            observed: &view,
            descriptor: &foreign,
            plan: &plan,
            observations: &[],
        },
    );
    let rejected = request.prepare_with(&model).expect_err("descriptor fence");
    assert_eq!(rejected.cause, ResourceStateError::PlanDescriptorMismatch);
    assert!(rejected.output.is_none());
    assert!(!model.entered.get());
    assert_eq!(data, b"a\xffc");
}

#[test]
fn stale_buffer_version_and_reopened_generation_retain_candidate() {
    let model = ByteModel::new();
    let data = b"a\xffc".to_vec();
    let original = support::state_stamp(0, 900);
    let view = support::view(&model.binding(), &original, &data);
    let plan = support::plan(&original, &[0, b'b'], b"a");
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Edit {
            observed: &view,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
    );
    let candidate = request.prepare_with(&model).expect("prepared edit");
    let advanced = support::state_stamp(1, 7);
    let replaced = BufferStateStamp::new(
        original.source(),
        BufferIncarnationStamp::new(
            original.buffer().buffer(),
            LifecycleIdentity::new(52, 53, Generation::new(2)),
        ),
        BufferVersion::INITIAL,
        original.history(),
    );
    for current in [advanced, replaced] {
        assert_eq!(
            candidate.check_admission(
                model.binding(),
                request.stamp(),
                ResourceStateTarget::Buffer(current),
                request.limits(),
                InstallationOutcome::Current,
            ),
            Err(ResourceStateError::StateMismatch)
        );
        assert_eq!(candidate.state(), b"b\xffc");
        assert_eq!(data, b"a\xffc");
        assert_eq!(view.stamp, original);
    }
}

#[test]
fn native_precondition_failure_preserves_original_state_and_history() {
    let model = ByteModel::new();
    let data = b"a\xffc".to_vec();
    let original = support::state_stamp(0, 900);
    let view = support::view(&model.binding(), &original, &data);
    let plan = support::plan(&original, &[0, b'b'], b"z");
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Edit {
            observed: &view,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
    );
    let rejected =
        request.prepare_with(&model).expect_err("native precondition");
    assert_eq!(rejected.cause, ResourceStateError::PreconditionFailed);
    assert!(rejected.output.is_none());
    assert!(model.entered.get());
    assert_eq!(data, b"a\xffc");
    assert_eq!(view.stamp.history(), original.history());
}

#[test]
fn precancel_and_deadline_reject_before_native_preparation() {
    let model = ByteModel::new();
    let observation = support::observation(b"a\xffc");
    let cases = [
        (
            CancellationState::Requested(CancellationCause::ExplicitRequest),
            DeadlineState::Pending,
            FenceCause::Cancellation,
        ),
        (CancellationState::Open, DeadlineState::Reached, FenceCause::Deadline),
    ];
    for (cancellation, deadline, cause) in cases {
        let result = ResourceStatePreparation::<Vec<u8>>::new(
            model.binding(),
            ResourceStateInput::Initial(&observation),
            support::limits(128, 8),
            cancellation,
            deadline,
        );
        assert_eq!(result.err(), Some(ResourceStateError::Fenced(cause)));
        assert!(!model.entered.get());
        assert_eq!(observation.payload().bytes(), b"a\xffc");
    }
}

#[test]
fn cancellation_after_preparation_preserves_owned_output() {
    let model = ByteModel::new();
    let observation = support::observation(b"a\xffc");
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Initial(&observation),
    );
    let candidate = request.prepare_with(&model).expect("prepared data");
    assert_eq!(
        candidate.check_admission(
            model.binding(),
            request.stamp(),
            request.stamp().target,
            request.limits(),
            InstallationOutcome::Fenced(FenceCause::Cancellation),
        ),
        Err(ResourceStateError::Fenced(FenceCause::Cancellation))
    );
    assert_eq!(candidate.into_output().state, b"a\xffc");
}

#[test]
fn native_usage_pressure_returns_the_prepared_state() {
    let model = ByteModel::new();
    let observation = support::observation(b"a\xffc");
    for limits in [support::limits(31, 8), support::limits(32, 7)] {
        let request = ResourceStatePreparation::new(
            model.binding(),
            ResourceStateInput::Initial(&observation),
            limits,
            CancellationState::Open,
            DeadlineState::Pending,
        )
        .expect("input admitted within observation reservation");
        let rejected =
            request.prepare_with(&model).expect_err("usage pressure");
        assert_eq!(rejected.cause, ResourceStateError::Pressure);
        let output = rejected.output.expect("produced state remains owned");
        assert_eq!(output.state, b"a\xffc");
        assert_eq!(output.usage, model.usage);
    }
}

#[test]
fn observation_pressure_prevents_native_callback() {
    let model = ByteModel::new();
    let observation = support::observation(b"a\xffc");
    let mut no_observations = support::limits(128, 8);
    no_observations.observation_count = support::count(0);
    let mut no_bytes = support::limits(128, 8);
    no_bytes.observation_bytes = support::bytes(0);
    for limits in [no_observations, no_bytes] {
        let result = ResourceStatePreparation::<Vec<u8>>::new(
            model.binding(),
            ResourceStateInput::Initial(&observation),
            limits,
            CancellationState::Open,
            DeadlineState::Pending,
        );
        assert_eq!(result.err(), Some(ResourceStateError::Pressure));
        assert!(!model.entered.get());
        assert_eq!(observation.payload().bytes(), b"a\xffc");
    }
}
