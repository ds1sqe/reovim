use core::fmt::Debug;
use std::collections::BTreeMap;

use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryEdge,
};
use reovim_kernel_interface::cancellation::InstallationOutcome;
use reovim_kernel_interface::resource::state::{
    ResourceStateInput, ResourceStateModel, ResourceStatePreparation,
    ResourceStateTransition, ResourceWorkingState,
};

use super::models::{ByteModel, Key, KeyedModel};
use super::support;

const ROOT: u128 = 900;
const FIRST_CHILD: u128 = 7;
const SIBLING: u128 = 42;

struct NativeEdit {
    operations: &'static [u8],
    preconditions: &'static [u8],
}

struct BranchCase<State> {
    payload: &'static [u8],
    first: NativeEdit,
    sibling: NativeEdit,
    initial: State,
    first_state: State,
    sibling_state: State,
}

#[test]
fn byte_branches_and_restoration_preserve_invalid_source_units() {
    branch_witness(
        &ByteModel::new(),
        &BranchCase {
            payload: b"a\xffc",
            first: NativeEdit { operations: &[0, b'b'], preconditions: b"a" },
            sibling: NativeEdit { operations: &[0, b'z'], preconditions: b"a" },
            initial: b"a\xffc".to_vec(),
            first_state: b"b\xffc".to_vec(),
            sibling_state: b"z\xffc".to_vec(),
        },
    );
}

#[test]
fn keyed_branches_use_same_preparation_and_restoration_boundary() {
    branch_witness(
        &KeyedModel::new(),
        &BranchCase {
            payload: &[0, 1],
            first: NativeEdit { operations: &[17, 1], preconditions: &[0] },
            sibling: NativeEdit { operations: &[93, 0], preconditions: &[1] },
            initial: BTreeMap::from([(Key::Alpha, false), (Key::Omega, true)]),
            first_state: BTreeMap::from([
                (Key::Alpha, true),
                (Key::Omega, true),
            ]),
            sibling_state: BTreeMap::from([
                (Key::Alpha, false),
                (Key::Omega, false),
            ]),
        },
    );
}

fn branch_witness<Model>(model: &Model, case: &BranchCase<Model::State>)
where
    Model: ResourceStateModel,
    Model::State: Debug + PartialEq,
{
    let observation = support::observation(case.payload);
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Initial(&observation),
    );
    let initial = prepare_and_admit(model, &request, &case.initial);
    assert_eq!(request.stamp().transition, ResourceStateTransition::Initial);
    let root = support::view(
        &model.binding(),
        &support::state_stamp(0, ROOT),
        &initial,
    );
    let first = edit(model, &root, &case.first, 61, &case.first_state);
    let first_view = support::view(
        &model.binding(),
        &support::state_stamp(1, FIRST_CHILD),
        &first,
    );
    // Relations/stamps are Core-selected fixture inputs, not a History engine.
    let undone = restore(
        model,
        &first_view,
        &root,
        BufferHistoryDirection::Undo,
        support::edge(ROOT, FIRST_CHILD),
        62,
    );
    assert_eq!(undone, case.initial);
    let undone_view = support::view(
        &model.binding(),
        &support::state_stamp(2, ROOT),
        &undone,
    );
    let sibling =
        edit(model, &undone_view, &case.sibling, 63, &case.sibling_state);
    let sibling_view = support::view(
        &model.binding(),
        &support::state_stamp(3, SIBLING),
        &sibling,
    );
    let back_at_root = restore(
        model,
        &sibling_view,
        &root,
        BufferHistoryDirection::Undo,
        support::edge(ROOT, SIBLING),
        64,
    );
    assert_eq!(back_at_root, case.initial);
    let current = support::view(
        &model.binding(),
        &support::state_stamp(4, ROOT),
        &back_at_root,
    );
    // These are alternative proposals from A; this witness installs neither.
    let restored_first = restore(
        model,
        &current,
        &first_view,
        BufferHistoryDirection::Redo,
        support::edge(ROOT, FIRST_CHILD),
        65,
    );
    let restored_sibling = restore(
        model,
        &current,
        &sibling_view,
        BufferHistoryDirection::Redo,
        support::edge(ROOT, SIBLING),
        66,
    );
    assert_eq!(restored_first, case.first_state);
    assert_eq!(restored_sibling, case.sibling_state);
    assert_eq!(first, case.first_state);
    assert_eq!(sibling, case.sibling_state);
    assert_eq!(initial, case.initial);
}

fn edit<Model>(
    model: &Model,
    observed: &ResourceWorkingState<'_, Model::State>,
    edit: &NativeEdit,
    transaction: u128,
    expected: &Model::State,
) -> Model::State
where
    Model: ResourceStateModel,
    Model::State: Debug + PartialEq,
{
    let plan = support::plan_for(
        &observed.stamp,
        transaction,
        edit.operations,
        edit.preconditions,
    );
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Edit {
            observed,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
    );
    prepare_and_admit(model, &request, expected)
}

fn restore<Model>(
    model: &Model,
    current: &ResourceWorkingState<'_, Model::State>,
    retained: &ResourceWorkingState<'_, Model::State>,
    direction: BufferHistoryDirection,
    edge: BufferHistoryEdge,
    transaction: u128,
) -> Model::State
where
    Model: ResourceStateModel,
    Model::State: Debug + PartialEq,
{
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Restore {
            current,
            retained,
            plan: support::plan_stamp(transaction),
            direction,
            edge,
        },
    );
    let restored = prepare_and_admit(model, &request, retained.state);
    assert_eq!(request.stamp().retained(), Some(retained.stamp));
    assert_eq!(
        request.stamp().transition,
        ResourceStateTransition::Restore {
            plan: support::plan_stamp(transaction),
            direction,
            edge,
        }
    );
    restored
}

fn prepare_and_admit<Model>(
    model: &Model,
    request: &ResourceStatePreparation<'_, Model::State>,
    expected: &Model::State,
) -> Model::State
where
    Model: ResourceStateModel,
    Model::State: Debug + PartialEq,
{
    let prepared = request.prepare_with(model).expect("native preparation");
    assert_eq!(prepared.state(), expected);
    assert_eq!(
        prepared.check_admission(
            model.binding(),
            request.stamp(),
            request.stamp().target,
            request.limits(),
            InstallationOutcome::Current,
        ),
        Ok(())
    );
    prepared.into_output().state
}
