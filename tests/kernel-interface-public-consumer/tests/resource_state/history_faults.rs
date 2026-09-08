use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferStateStamp,
};
use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::resource::state::{
    ResourceStateError, ResourceStateInput, ResourceStateModel,
    ResourceStatePreparation, ResourceStateTransition,
};
use reovim_kernel_interface::resource::{
    ResourceRevision, ResourceSnapshotIdentity, ResourceSnapshotStamp,
};

use super::models::ByteModel;
use super::support;

#[test]
fn wrong_tree_edge_current_or_retained_node_rejects_before_callback() {
    let model = ByteModel::new();
    let root_data = b"a\xffc".to_vec();
    let child_data = b"b\xffc".to_vec();
    let cases = [
        (900, 7, support::edge(900, 42)),
        (42, 7, support::edge(900, 7)),
        (900, 42, support::edge(900, 7)),
    ];
    for (from, to, edge) in cases {
        let current = support::view(
            &model.binding(),
            &support::state_stamp(4, from),
            &root_data,
        );
        let retained = support::view(
            &model.binding(),
            &support::state_stamp(1, to),
            &child_data,
        );
        let result = ResourceStatePreparation::new(
            model.binding(),
            ResourceStateInput::Restore {
                current: &current,
                retained: &retained,
                plan: support::plan_stamp(81),
                direction: BufferHistoryDirection::Redo,
                edge,
            },
            support::limits(128, 8),
            CancellationState::Open,
            DeadlineState::Pending,
        );
        assert_eq!(result.err(), Some(ResourceStateError::StateMismatch));
        assert!(!model.entered.get());
        assert_eq!(root_data, b"a\xffc");
        assert_eq!(child_data, b"b\xffc");
    }
}

#[test]
fn changed_core_selected_branch_or_request_preserves_owned_candidate() {
    let model = ByteModel::new();
    let root_data = b"a\xffc".to_vec();
    let first_data = b"b\xffc".to_vec();
    let sibling_data = b"z\xffc".to_vec();
    let current = support::view(
        &model.binding(),
        &support::state_stamp(4, 900),
        &root_data,
    );
    let first = support::view(
        &model.binding(),
        &support::state_stamp(1, 7),
        &first_data,
    );
    let sibling = support::view(
        &model.binding(),
        &support::state_stamp(3, 42),
        &sibling_data,
    );
    let selected_first = support::request(
        &model.binding(),
        ResourceStateInput::Restore {
            current: &current,
            retained: &first,
            plan: support::plan_stamp(81),
            direction: BufferHistoryDirection::Redo,
            edge: support::edge(900, 7),
        },
    );
    let selected_sibling = support::request(
        &model.binding(),
        ResourceStateInput::Restore {
            current: &current,
            retained: &sibling,
            plan: support::plan_stamp(81),
            direction: BufferHistoryDirection::Redo,
            edge: support::edge(900, 42),
        },
    );
    let candidate = selected_first.prepare_with(&model).expect("first branch");
    let mut changed_request = selected_first.stamp();
    changed_request.transition = ResourceStateTransition::Restore {
        plan: support::plan_stamp(82),
        direction: BufferHistoryDirection::Redo,
        edge: support::edge(900, 7),
    };
    model.entered.set(false);
    for expected in [selected_sibling.stamp(), changed_request] {
        assert_eq!(
            candidate.check_admission(
                model.binding(),
                expected,
                selected_first.stamp().target,
                selected_first.limits(),
                InstallationOutcome::Current,
            ),
            Err(ResourceStateError::PreparationMismatch)
        );
        assert_eq!(candidate.state(), b"b\xffc");
        assert_eq!(root_data, b"a\xffc");
        assert_eq!(sibling_data, b"z\xffc");
        assert!(!model.entered.get());
    }
}

#[test]
fn retained_history_can_reconstruct_data_from_an_older_source_snapshot() {
    let model = ByteModel::new();
    let root_data = b"a\xffc".to_vec();
    let child_data = b"b\xffc".to_vec();
    let old = support::state_stamp(4, 900);
    let new_source = ResourceSnapshotStamp::new(
        old.source().access(),
        ResourceRevision::new(old.source().revision().schema(), 8),
        ResourceSnapshotIdentity::new(144),
    );
    let new_stamp = BufferStateStamp::new(
        new_source,
        old.buffer(),
        old.version(),
        old.history(),
    );
    let current = support::view(&model.binding(), &new_stamp, &root_data);
    let retained = support::view(
        &model.binding(),
        &support::state_stamp(1, 7),
        &child_data,
    );
    let request = support::request(
        &model.binding(),
        ResourceStateInput::Restore {
            current: &current,
            retained: &retained,
            plan: support::plan_stamp(81),
            direction: BufferHistoryDirection::Redo,
            edge: support::edge(900, 7),
        },
    );
    let candidate = request.prepare_with(&model).expect("retained source data");
    assert_eq!(candidate.state(), b"b\xffc");
    assert_eq!(candidate.stamp().target, request.stamp().target);
    assert_eq!(candidate.stamp().retained(), Some(retained.stamp));
    assert_eq!(
        candidate.check_admission(
            model.binding(),
            request.stamp(),
            request.stamp().target,
            request.limits(),
            InstallationOutcome::Current,
        ),
        Ok(())
    );
}
