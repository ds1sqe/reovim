use alloc::{boxed::Box, vec::Vec};

use reovim_kernel_interface::buffer::plan::BufferTransactionPlan;
use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryNodeIdentity, BufferHistoryStamp,
    BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::update::RepresentationUpdate;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceWorkingState,
};
use reovim_kernel_interface::semantic::domain::DomainValidationRequest;
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::intent::SemanticIntent;
use reovim_kernel_interface::semantic::policy::SemanticPolicyFailure;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;
use reovim_kernel_interface::semantic::{
    representation::RepresentationLoweringRequest, state::SemanticPolicyInput,
};

use super::families::Family;
use super::native::models::{ByteModel, KeyedModel, KeyedState};
use super::native::support as native;
use super::policies::Policies;
use super::snapshots;
use crate::semantic::support::{self, SemanticFixture};

/// Observable data from a composed contract witness, not a running editor.
#[derive(Debug)]
pub struct Evidence<State> {
    pub initial: State,
    pub prepared: State,
    pub restored: State,
    pub operations: Box<[u8]>,
    pub preconditions: Box<[u8]>,
    pub initial_snapshot: Box<[u8]>,
    pub prepared_snapshot: Box<[u8]>,
    pub restored_snapshot: Box<[u8]>,
    pub prepared_update: RepresentationUpdate,
}

/// Exercises a tiny heading-rename profile while preserving opaque bytes.
///
/// # Errors
/// Returns the Domain/Representation's expected semantic rejection.
pub fn heading(
    source: &[u8],
    replacement: u8,
) -> Result<Evidence<Vec<u8>>, SemanticPolicyFailure> {
    run(&ByteModel::new(), source, &[replacement])
}

/// Exercises keyed toggle semantics without treating key tags as offsets.
///
/// # Errors
/// Returns the Domain/Representation's expected semantic rejection.
pub fn keyed(
    source: &[u8],
    selected_key: u8,
) -> Result<Evidence<KeyedState>, SemanticPolicyFailure> {
    run(&KeyedModel::new(), source, &[selected_key])
}

fn run<Model: Family>(
    model: &Model,
    payload: &[u8],
    semantic_bytes: &[u8],
) -> Result<Evidence<Model::State>, SemanticPolicyFailure> {
    let fixture = support::fixture_for_model(&model.binding())
        .expect("existing semantic descriptors with selected native model");
    let initial_stamp = stamp(&fixture.state.stamp, 0, 900);
    let observation = native::observation_at(initial_stamp.source(), payload);
    let preparation = native::request(
        &model.binding(),
        ResourceStateInput::Initial(&observation),
    );
    let initial = preparation
        .prepare_with(model)
        .expect("initial native preparation")
        .into_output()
        .state;
    let current = native::view(&model.binding(), &initial_stamp, &initial);
    let intent = support::make_intent_on(
        &fixture,
        semantic_bytes,
        &initial_stamp,
        2060,
        2061,
    )
    .expect("bounded semantic intent");
    let plan = semantic_plan(model, &fixture, &current, &intent)?;
    let operations = plan.operations().bytes().into();
    let preconditions = plan.preconditions().bytes().into();
    let (initial_snapshot, _) = snapshots::produce(
        model,
        &fixture,
        &current,
        intent.stamp().projection(),
        ProjectionGeneration::new(3),
        RepresentationUpdateCursor::INITIAL,
    );
    let preparation = native::request(
        &model.binding(),
        ResourceStateInput::Edit {
            observed: &current,
            descriptor: &fixture.buffer,
            plan: &plan,
            observations: &[],
        },
    );
    let candidate = preparation.prepare_with(model).expect("native edit");
    candidate
        .check_admission(
            model.binding(),
            preparation.stamp(),
            preparation.stamp().target,
            preparation.limits(),
            InstallationOutcome::Current,
        )
        .expect("prepared plan remains current");
    let prepared = candidate.into_output().state;
    // These accepted observations are fixture inputs; no Core engine exists.
    let prepared_view =
        native::view(&model.binding(), &stamp(&initial_stamp, 1, 7), &prepared);
    let (prepared_snapshot, prepared_update) = snapshots::produce(
        model,
        &fixture,
        &prepared_view,
        intent.stamp().projection(),
        ProjectionGeneration::new(4),
        RepresentationUpdateCursor::new(1),
    );
    let restored = restore(model, &prepared_view, &current);
    let restored_view = native::view(
        &model.binding(),
        &stamp(&initial_stamp, 2, 900),
        &restored,
    );
    let (restored_snapshot, _) = snapshots::produce(
        model,
        &fixture,
        &restored_view,
        &next_projection(intent.stamp().projection(), 4),
        ProjectionGeneration::new(5),
        RepresentationUpdateCursor::new(2),
    );
    Ok(Evidence {
        initial,
        prepared,
        restored,
        operations,
        preconditions,
        initial_snapshot,
        prepared_snapshot,
        restored_snapshot,
        prepared_update,
    })
}

pub fn semantic_plan<Model: Family>(
    model: &Model,
    fixture: &SemanticFixture,
    observed: &ResourceWorkingState<'_, Model::State>,
    intent: &SemanticIntent,
) -> Result<BufferTransactionPlan, SemanticPolicyFailure> {
    let provider = Policies { fixture, model };
    let input = SemanticPolicyInput { intent, state: observed };
    let charge = fixture
        .domain
        .validation()
        .bounds()
        .prepare(8, 16, 0, 1)
        .expect("Domain work reservation");
    let request = DomainValidationRequest::new(
        &fixture.domain,
        &fixture.representation,
        input,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("state-aware Domain request");
    let validated = request.finish(
        &fixture.domain,
        request.invoke(&provider).expect("selected Domain"),
        InstallationOutcome::Current,
    );
    if let Some(cause) = validated.failure() {
        return Err(cause);
    }
    let receipt = validated.receipt().expect("exact state-aware receipt");
    let charge = fixture
        .representation
        .lowering()
        .bounds()
        .prepare(8, 16, 20, 1)
        .expect("lowering reservation");
    let request = RepresentationLoweringRequest::new(
        &fixture.representation,
        input,
        &receipt,
        &fixture.buffer,
        charge,
        CancellationState::Open,
        DeadlineState::Pending,
    )
    .expect("same observed state supplied to lowering");
    let lowered = request
        .invoke(&provider)
        .expect("selected Representation")
        .finish(request, &fixture.representation, InstallationOutcome::Current);
    if let Some(cause) = lowered.failure() {
        return Err(cause);
    }
    Ok(lowered.into_plan().expect("complete native plan"))
}

fn restore<Model: Family>(
    model: &Model,
    current: &ResourceWorkingState<'_, Model::State>,
    retained: &ResourceWorkingState<'_, Model::State>,
) -> Model::State {
    let request = native::request(
        &model.binding(),
        ResourceStateInput::Restore {
            current,
            retained,
            plan: native::plan_stamp(2070),
            direction: BufferHistoryDirection::Undo,
            edge: native::edge(900, 7),
        },
    );
    let candidate =
        request.prepare_with(model).expect("retained reconstruction");
    candidate
        .check_admission(
            model.binding(),
            request.stamp(),
            request.stamp().target,
            request.limits(),
            InstallationOutcome::Current,
        )
        .expect("same selected tree edge");
    candidate.into_output().state
}

const fn stamp(
    source: &BufferStateStamp,
    version: u64,
    node: u128,
) -> BufferStateStamp {
    BufferStateStamp::new(
        source.source(),
        source.buffer(),
        BufferVersion::new(version),
        BufferHistoryStamp::new(
            source.history().lifecycle(),
            BufferHistoryNodeIdentity::new(node),
        ),
    )
}

const fn next_projection(
    old: &ProjectionStamp,
    generation: u64,
) -> ProjectionStamp {
    ProjectionStamp::new(
        old.projection(),
        old.lifecycle(),
        old.representation(),
        ProjectionGeneration::new(generation),
    )
}
