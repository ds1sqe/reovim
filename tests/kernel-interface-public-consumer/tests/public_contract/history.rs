use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferHistoryEdgeError,
    BufferHistoryNodeIdentity as Node, BufferHistoryStamp, BufferStateStamp,
    BufferVersion,
};
use reovim_kernel_interface::effect::EffectCertainty;
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryAction as Action, InteractionHistoryOutcome as Outcome,
    InteractionHistoryRequest as Request,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause, InteractionOutcomeStatus,
};
use reovim_kernel_interface::resource::ResourceSnapshotStamp;

#[path = "history/faults.rs"]
mod faults;

const PARENT: Node = Node::new(0xa1);
const FIRST: Node = Node::new(0x03);
const SIBLING: Node = Node::new(u128::MAX);

#[test]
fn undo_and_selected_redo_follow_edges_not_node_order() {
    let session = session();
    let first_edge = edge(PARENT, FIRST);
    let sibling_edge = edge(PARENT, SIBLING);
    let undo = request(&session, 5, FIRST, Action::Undo);
    let restored = state(&session, 6, PARENT);
    let undone = Outcome::accepted_history(undo, first_edge, restored)
        .expect("undo follows the parent despite its larger node identity");
    let accepted = undone.accepted_state().expect("accepted transition");
    assert_eq!(undone.request(), undo);
    assert_eq!(undone.status(), InteractionOutcomeStatus::Accepted);
    assert_eq!(undone.effect_certainty(), EffectCertainty::Applied);
    assert_eq!(accepted.result(), restored);
    assert_eq!(accepted.history_edge(), first_edge);
    assert_eq!(
        accepted.history_direction(),
        Some(BufferHistoryDirection::Undo)
    );
    assert_eq!(accepted.transaction(), undo.plan().transaction());
    assert_eq!(accepted.history(), undo.plan().history_transition());

    let redo =
        request(&session, 6, PARENT, Action::Redo { child: Some(SIBLING) });
    let redone = Outcome::accepted_history(
        redo,
        sibling_edge,
        state(&session, 7, SIBLING),
    )
    .expect("redo selects the named sibling");
    let accepted = redone.accepted_state().expect("accepted sibling");
    assert_eq!(accepted.result().history().node(), SIBLING);
    assert_eq!(accepted.history_edge(), sibling_edge);
    assert_eq!(
        accepted.history_direction(),
        Some(BufferHistoryDirection::Redo)
    );

    // The earlier child remains a distinct traversable branch. These are
    // contract values, not an executing Core tree or a retention proof.
    let old_branch =
        request(&session, 8, PARENT, Action::Redo { child: Some(FIRST) });
    let old_result = Outcome::accepted_history(
        old_branch,
        first_edge,
        state(&session, 9, FIRST),
    )
    .expect("earlier sibling remains selectable");
    assert_eq!(old_result.accepted_state().unwrap().history_edge(), first_edge);
}

#[test]
fn edits_report_new_child_edges_without_ordinal_assumptions() {
    let inputs =
        reovim_kernel_interface_public_consumer::interaction_debug_inputs()
            .expect("public semantic edit fixture");
    let stamp = inputs.intent.semantic().stamp();
    let target = stamp.target();
    let result = BufferStateStamp::new(
        target.source(),
        target.buffer(),
        BufferVersion::new(1),
        BufferHistoryStamp::new(target.history().lifecycle(), SIBLING),
    );
    let accepted =
        InteractionOutcome::accepted(inputs.intent.stamp(), stamp, result)
            .expect("Core-selected fresh child need not follow a node counter")
            .accepted_state()
            .unwrap();
    assert_eq!(accepted.history_edge(), edge(target.history().node(), SIBLING));
    assert_eq!(accepted.history_direction(), None);
}

#[test]
fn missing_eligible_edges_are_rejections_not_identity_arithmetic() {
    let session = session();
    for action in [Action::Undo, Action::Redo { child: None }] {
        let request = request(&session, 2, SIBLING, action);
        let outcome =
            Outcome::not_accepted(request, InteractionOutcomeCause::NoHistory);
        assert_eq!(outcome.request(), request);
        assert_eq!(outcome.accepted_state(), None);
        assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::NoHistory));
        assert_eq!(
            outcome.effect_certainty(),
            EffectCertainty::DefinitelyNotApplied
        );
    }
    assert_eq!(
        BufferHistoryEdge::new(PARENT, PARENT),
        Err(BufferHistoryEdgeError::SelfEdge)
    );
}

fn session() -> descriptor::InteractionSessionDescriptor {
    reovim_kernel_interface_public_consumer::interaction_debug_inputs()
        .expect("existing public Session descriptor fixture")
        .session
}

fn request(
    session: &descriptor::InteractionSessionDescriptor,
    version: u64,
    node: Node,
    action: Action,
) -> Request {
    Request::new(
        session,
        &super::buffer_descriptor(),
        InteractionRequestIdentity::new(7_000),
        super::plan_stamp(1),
        state(session, version, node),
        action,
    )
    .expect("structurally bound History request without a semantic Intent")
}

fn edge(parent: Node, child: Node) -> BufferHistoryEdge {
    BufferHistoryEdge::new(parent, child).expect("distinct History endpoints")
}

const fn state(
    session: &descriptor::InteractionSessionDescriptor,
    version: u64,
    node: Node,
) -> BufferStateStamp {
    let seed = super::buffer_state();
    let source = ResourceSnapshotStamp::new(
        session.scope().access(),
        seed.source().revision(),
        seed.source().snapshot(),
    );
    BufferStateStamp::new(
        source,
        session.scope().buffer(),
        BufferVersion::new(version),
        BufferHistoryStamp::new(seed.history().lifecycle(), node),
    )
}
