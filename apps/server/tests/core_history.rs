use std::{cell::Cell, rc::Rc};

use reovim_kernel_core::{Buffer, BufferCommit, BufferError, HistorySelection};
use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryNodeIdentity,
};
use reovim_kernel_interface::cancellation::{FenceCause, InstallationOutcome};
use reovim_kernel_interface::resource::state::{
    ResourcePreparationStamp, ResourcePreparedState, ResourceStateError,
    ResourceStateInput, ResourceStateModel, ResourceStateTransition,
};
use reovim_resource_local::{ByteModel, ByteState, Key, KeyedModel};

#[path = "support/probe.rs"]
mod probe;
#[path = "support/core.rs"]
mod support;
use probe::{ProbeModel, Watch};
use support::{life, opened, preparation, prepared, stamp};

#[test]
fn siblings_and_preferred_redo_are_real_tree_relations() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    let root = buffer.stamp();
    assert!(
        buffer
            .select_history(
                &root,
                stamp(&model, 1),
                BufferHistoryDirection::Undo,
                None
            )
            .unwrap()
            .is_none()
    );
    let b = byte_edit(&mut buffer, &model, 10, b'b');
    traverse(&mut buffer, &model, 11, BufferHistoryDirection::Undo, None);
    let c = byte_edit(&mut buffer, &model, 12, b'c');
    traverse(&mut buffer, &model, 13, BufferHistoryDirection::Undo, None);
    let default =
        traverse(&mut buffer, &model, 14, BufferHistoryDirection::Redo, None);
    assert_eq!(default.edge, c.edge);
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# c\n\xff");
    traverse(&mut buffer, &model, 15, BufferHistoryDirection::Undo, None);
    let explicit = traverse(
        &mut buffer,
        &model,
        16,
        BufferHistoryDirection::Redo,
        Some(b.edge.child()),
    );
    assert_eq!(explicit.edge, b.edge);
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# b\n\xff");
    traverse(&mut buffer, &model, 17, BufferHistoryDirection::Undo, None);
    let preferred =
        traverse(&mut buffer, &model, 18, BufferHistoryDirection::Redo, None);
    assert_eq!(preferred.edge, b.edge);
    assert!(
        buffer
            .select_history(
                &buffer.stamp(),
                stamp(&model, 19),
                BufferHistoryDirection::Redo,
                None
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(root.history().node(), b.edge.parent());
    assert_eq!(root.history().node(), c.edge.parent());
    assert_ne!(b.edge.child(), c.edge.child());
}

#[test]
fn keyed_siblings_preserve_unrelated_native_records() {
    let model = KeyedModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, &[93, 1, 17, 0], 8, 4096);
    let b = native_edit(&mut buffer, &model, 10, &[17, 1], &[0]);
    traverse(&mut buffer, &model, 11, BufferHistoryDirection::Undo, None);
    let c = native_edit(&mut buffer, &model, 12, &[93, 0], &[1]);
    traverse(&mut buffer, &model, 13, BufferHistoryDirection::Undo, None);
    let restored =
        traverse(&mut buffer, &model, 14, BufferHistoryDirection::Redo, None);
    assert_eq!(restored.edge, c.edge);
    assert_eq!(buffer.observe().view().state.get(Key::Alpha), Some(false));
    assert_eq!(buffer.observe().view().state.get(Key::Omega), Some(false));
    traverse(&mut buffer, &model, 15, BufferHistoryDirection::Undo, None);
    traverse(
        &mut buffer,
        &model,
        16,
        BufferHistoryDirection::Redo,
        Some(b.edge.child()),
    );
    assert_eq!(buffer.observe().view().state.get(Key::Alpha), Some(true));
    assert_eq!(buffer.observe().view().state.get(Key::Omega), Some(true));
}

#[test]
fn stale_default_selection_cannot_follow_a_changed_preference() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    let b = byte_edit(&mut buffer, &model, 10, b'b');
    traverse(&mut buffer, &model, 11, BufferHistoryDirection::Undo, None);
    let c = byte_edit(&mut buffer, &model, 12, b'c');
    traverse(&mut buffer, &model, 13, BufferHistoryDirection::Undo, None);
    let pending =
        select(&buffer, &model, 14, BufferHistoryDirection::Redo, None);
    assert_eq!(pending.edge(), c.edge);
    let (expected, candidate) = reconstruction(&model, &pending);
    let staged = buffer.stage_history(&pending, &expected, candidate).unwrap();
    traverse(
        &mut buffer,
        &model,
        15,
        BufferHistoryDirection::Redo,
        Some(b.edge.child()),
    );
    traverse(&mut buffer, &model, 16, BufferHistoryDirection::Undo, None);
    let unchanged = buffer.stamp();
    let rejected = buffer
        .commit_history(staged, InstallationOutcome::Current)
        .unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::StateMismatch)
    );
    assert_eq!(buffer.stamp(), unchanged);
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# a\n\xff");
    assert_eq!(
        select(&buffer, &model, 17, BufferHistoryDirection::Redo, None).edge(),
        b.edge
    );
}

#[test]
fn reconstruction_is_charged_separately_without_consuming_node_capacity() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, sample) = opened(&model, b"# a\n\xff", 2, 4096);
    let charge = sample.resident_bytes().value();
    let (_source, mut full) = opened(&model, b"# a\n\xff", 2, charge * 2);
    byte_edit(&mut full, &model, 10, b'b');
    let pending = select(&full, &model, 11, BufferHistoryDirection::Undo, None);
    let (expected, candidate) = reconstruction(&model, &pending);
    let rejected =
        full.stage_history(&pending, &expected, candidate).unwrap_err();
    assert_eq!(rejected.cause, BufferError::Capacity);
    assert_eq!(rejected.value.state().as_bytes(), b"# a\n\xff");
    assert_eq!(full.observe().view().state.as_bytes(), b"# b\n\xff");
    let (_source, mut enough) = opened(&model, b"# a\n\xff", 2, charge * 3);
    byte_edit(&mut enough, &model, 20, b'b');
    traverse(&mut enough, &model, 21, BufferHistoryDirection::Undo, None);
    assert_eq!(enough.resident_bytes().value(), charge * 3);
    traverse(&mut enough, &model, 22, BufferHistoryDirection::Redo, None);
    assert_eq!(enough.resident_bytes().value(), charge * 3);
    assert_eq!(enough.observe().view().state.as_bytes(), b"# b\n\xff");
}

#[test]
fn cancelled_and_wrong_native_preparations_leave_history_unchanged() {
    let model = ByteModel::new(life(1), life(2));
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    byte_edit(&mut buffer, &model, 10, b'b');
    let pending =
        select(&buffer, &model, 11, BufferHistoryDirection::Undo, None);
    let (mut expected, candidate) = reconstruction(&model, &pending);
    expected.transition = ResourceStateTransition::Initial;
    let rejected =
        buffer.stage_history(&pending, &expected, candidate).unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::PreparationMismatch)
    );
    let (expected, candidate) = reconstruction(&model, &pending);
    let staged = buffer.stage_history(&pending, &expected, candidate).unwrap();
    let before = buffer.stamp();
    let rejected = buffer
        .commit_history(
            staged,
            InstallationOutcome::Fenced(FenceCause::Cancellation),
        )
        .unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::Fenced(
            FenceCause::Cancellation
        ))
    );
    assert_eq!(buffer.stamp(), before);
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# b\n\xff");
    assert!(
        buffer
            .select_history(
                &buffer.stamp(),
                stamp(&model, 12),
                BufferHistoryDirection::Redo,
                Some(BufferHistoryNodeIdentity::new(u128::MAX))
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_replacement_model_cannot_install_an_old_history_selection() {
    let model = ByteModel::new(life(1), life(2));
    let replacement = ByteModel::new(life(99), life(2));
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    byte_edit(&mut buffer, &model, 10, b'b');
    let selected =
        select(&buffer, &model, 11, BufferHistoryDirection::Undo, None);
    let mut current = selected.current().view();
    let mut retained = selected.retained().view();
    current.model = replacement.binding();
    retained.model = replacement.binding();
    let request = preparation(
        &replacement,
        ResourceStateInput::Restore {
            current: &current,
            retained: &retained,
            plan: selected.plan(),
            direction: selected.direction(),
            edge: selected.edge(),
        },
    );
    let candidate = request.prepare_with(&replacement).unwrap();
    let rejected = buffer
        .stage_history(&selected, &request.stamp(), candidate)
        .unwrap_err();
    assert_eq!(
        rejected.cause,
        BufferError::State(ResourceStateError::ModelMismatch)
    );
    assert_eq!(rejected.value.state().as_bytes(), b"# a\n\xff");
    assert_eq!(buffer.stamp(), selected.current().stamp());
    assert_eq!(buffer.observe().view().state.as_bytes(), b"# b\n\xff");
}

#[test]
fn reconstructed_data_is_installed_and_retired_outside_authority() {
    let watch =
        Rc::new(Watch { inside: Cell::new(false), dropped: Cell::new(None) });
    let model = ProbeModel {
        base: ByteModel::new(life(1), life(2)),
        next: Cell::new(0),
        watch: watch.clone(),
    };
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    let original = buffer.observe().view().state.serial;
    native_edit(&mut buffer, &model, 10, &[2, 0, 0, 0, 0, 0, 0, 0, b'b'], b"a");
    let b = buffer.observe().view().state.serial;
    let undo = select(&buffer, &model, 11, BufferHistoryDirection::Undo, None);
    assert_eq!(undo.retained().view().state.serial, original);
    let (expected, candidate) = reconstruction(&model, &undo);
    let rebuilt = candidate.state().serial;
    let staged = buffer.stage_history(&undo, &expected, candidate).unwrap();
    watch.inside.set(true);
    let installed = buffer.commit_history(staged, InstallationOutcome::Current);
    watch.inside.set(false);
    drop(installed.unwrap().retired);
    assert_eq!(buffer.observe().view().state.serial, rebuilt);
    assert_ne!(rebuilt, original);
    assert_eq!(undo.retained().view().state.serial, original);
    let redo = select(&buffer, &model, 12, BufferHistoryDirection::Redo, None);
    assert_eq!(redo.retained().view().state.serial, b);
    let (expected, candidate) = reconstruction(&model, &redo);
    let staged = buffer.stage_history(&redo, &expected, candidate).unwrap();
    watch.inside.set(true);
    let installed = buffer.commit_history(staged, InstallationOutcome::Current);
    watch.inside.set(false);
    let retired = installed.unwrap().retired;
    assert_eq!(retired.view().state.serial, rebuilt);
    drop(redo);
    assert_eq!(watch.dropped.get(), None);
    drop(retired);
    assert_eq!(watch.dropped.get(), Some(rebuilt));
    assert_eq!(buffer.observe().view().state.native.as_bytes(), b"# b\n\xff");
    assert_ne!(buffer.observe().view().state.serial, b);
}

#[test]
fn rejected_restores_and_edits_release_native_materializations_after_return() {
    let watch =
        Rc::new(Watch { inside: Cell::new(false), dropped: Cell::new(None) });
    let model = ProbeModel {
        base: ByteModel::new(life(1), life(2)),
        next: Cell::new(0),
        watch: watch.clone(),
    };
    let (_source, mut buffer) = opened(&model, b"# a\n\xff", 8, 4096);
    native_edit(&mut buffer, &model, 10, &[2, 0, 0, 0, 0, 0, 0, 0, b'b'], b"a");
    traverse(&mut buffer, &model, 11, BufferHistoryDirection::Undo, None);
    let materialization = buffer.observe().view().state.serial;
    let selected =
        select(&buffer, &model, 12, BufferHistoryDirection::Redo, None);
    let (expected, candidate) = reconstruction(&model, &selected);
    let rejected_serial = candidate.state().serial;
    let staged = buffer.stage_history(&selected, &expected, candidate).unwrap();
    watch.inside.set(true);
    let rejected = buffer.commit_history(
        staged,
        InstallationOutcome::Fenced(FenceCause::Cancellation),
    );
    watch.inside.set(false);
    let rejected = rejected.unwrap_err();
    assert_eq!(watch.dropped.get(), None);
    drop(rejected);
    assert_eq!(watch.dropped.get(), Some(rejected_serial));
    drop(selected);
    watch.dropped.set(None);
    let (expected, candidate) = native_candidate(
        &buffer,
        &model,
        13,
        &[2, 0, 0, 0, 0, 0, 0, 0, b'd'],
        b"a",
    );
    let staged = buffer.stage_edit(&expected, candidate).unwrap();
    watch.inside.set(true);
    let installed = buffer.commit_edit(staged, InstallationOutcome::Current);
    watch.inside.set(false);
    let retired = installed.unwrap().retired;
    assert_eq!(retired.view().state.serial, materialization);
    assert_eq!(watch.dropped.get(), None);
    drop(retired);
    assert_eq!(watch.dropped.get(), Some(materialization));
    assert_eq!(buffer.observe().view().state.native.as_bytes(), b"# d\n\xff");
}

fn byte_edit(
    buffer: &mut Buffer<ByteState>,
    model: &ByteModel,
    id: u128,
    replacement: u8,
) -> BufferCommit {
    let (expected, candidate) = prepared(model, buffer, id, replacement);
    let staged = buffer.stage_edit(&expected, candidate).unwrap();
    buffer.commit_edit(staged, InstallationOutcome::Current).unwrap().commit
}

fn native_edit<Model: ResourceStateModel>(
    buffer: &mut Buffer<Model::State>,
    model: &Model,
    id: u128,
    operations: &[u8],
    preconditions: &[u8],
) -> BufferCommit {
    let (expected, candidate) =
        native_candidate(buffer, model, id, operations, preconditions);
    let staged = buffer.stage_edit(&expected, candidate).unwrap();
    buffer.commit_edit(staged, InstallationOutcome::Current).unwrap().commit
}

fn native_candidate<Model: ResourceStateModel>(
    buffer: &Buffer<Model::State>,
    model: &Model,
    id: u128,
    operations: &[u8],
    preconditions: &[u8],
) -> (ResourcePreparationStamp, ResourcePreparedState<Model::State>) {
    let lease = buffer.observe();
    let view = lease.view();
    let plan = support::plan(model, &view.stamp, id, operations, preconditions);
    let request = preparation(
        model,
        ResourceStateInput::Edit {
            observed: &view,
            descriptor: model.plan_descriptor(),
            plan: &plan,
            observations: &[],
        },
    );
    (request.stamp(), request.prepare_with(model).unwrap())
}

fn select<Model: ResourceStateModel>(
    buffer: &Buffer<Model::State>,
    model: &Model,
    id: u128,
    direction: BufferHistoryDirection,
    child: Option<BufferHistoryNodeIdentity>,
) -> HistorySelection<Model::State> {
    buffer
        .select_history(&buffer.stamp(), stamp(model, id), direction, child)
        .unwrap()
        .expect("eligible retained edge")
}

fn traverse<Model: ResourceStateModel>(
    buffer: &mut Buffer<Model::State>,
    model: &Model,
    id: u128,
    direction: BufferHistoryDirection,
    child: Option<BufferHistoryNodeIdentity>,
) -> BufferCommit {
    let selected = select(buffer, model, id, direction, child);
    let (expected, candidate) = reconstruction(model, &selected);
    let staged = buffer.stage_history(&selected, &expected, candidate).unwrap();
    let installed =
        buffer.commit_history(staged, InstallationOutcome::Current).unwrap();
    assert_eq!(
        installed.commit.result.version().value(),
        installed.commit.prior.version().value() + 1
    );
    assert_eq!(
        installed.commit.result.history().node(),
        selected.retained().stamp().history().node()
    );
    installed.commit
}

fn reconstruction<Model: ResourceStateModel>(
    model: &Model,
    selected: &HistorySelection<Model::State>,
) -> (ResourcePreparationStamp, ResourcePreparedState<Model::State>) {
    let current = selected.current().view();
    let retained = selected.retained().view();
    let request = preparation(
        model,
        ResourceStateInput::Restore {
            current: &current,
            retained: &retained,
            plan: selected.plan(),
            direction: selected.direction(),
            edge: selected.edge(),
        },
    );
    (request.stamp(), request.prepare_with(model).unwrap())
}
