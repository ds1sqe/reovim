use reovim_kernel_interface::buffer::plan::BufferPlanStamp;
use reovim_kernel_interface::interaction::{
    history::InteractionHistoryRequestError, outcome::InteractionOutcomeError,
    session::InteractionSessionStamp,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};

use super::{
    Action, BufferHistoryStamp, BufferStateStamp, BufferVersion, FIRST,
    Outcome, PARENT, Request, SIBLING, descriptor, edge, request, session,
    state,
};

#[test]
fn version_edges_and_explicit_branch_selections_are_revalidated() {
    let session = session();
    let undo = request(&session, 5, FIRST, Action::Undo);
    assert_eq!(
        Outcome::accepted_history(
            undo,
            edge(PARENT, FIRST),
            state(&session, 5, PARENT)
        ),
        Err(InteractionOutcomeError::BufferVersionNotAdvanced {
            expected: BufferVersion::new(6),
            actual: BufferVersion::new(5),
        })
    );
    assert_eq!(
        Outcome::accepted_history(
            undo,
            edge(PARENT, SIBLING),
            state(&session, 6, PARENT)
        ),
        Err(InteractionOutcomeError::HistoryEdgeMismatch)
    );
    assert_eq!(
        Outcome::accepted_history(
            undo,
            edge(PARENT, FIRST),
            state(&session, 6, SIBLING)
        ),
        Err(InteractionOutcomeError::HistoryEdgeMismatch)
    );
    let redo =
        request(&session, 6, PARENT, Action::Redo { child: Some(FIRST) });
    assert_eq!(
        Outcome::accepted_history(
            redo,
            edge(PARENT, SIBLING),
            state(&session, 7, SIBLING)
        ),
        Err(InteractionOutcomeError::HistoryBranchMismatch)
    );
    let overflow = request(&session, u64::MAX, FIRST, Action::Undo);
    assert_eq!(
        Outcome::accepted_history(
            overflow,
            edge(PARENT, FIRST),
            state(&session, 0, PARENT)
        ),
        Err(InteractionOutcomeError::BufferVersionOverflow)
    );
}

#[test]
fn request_matching_includes_selection_target_and_causal_identity() {
    let session = session();
    let descriptor = super::super::buffer_descriptor();
    let pending = request(&session, 5, PARENT, Action::Redo { child: None });
    assert_eq!(pending.validate(&session, &descriptor), Ok(()));
    assert!(pending.same_request(pending));
    let selected =
        request(&session, 5, PARENT, Action::Redo { child: Some(FIRST) });
    assert!(!pending.same_request(selected));
    let different_target = request(&session, 5, SIBLING, pending.action());
    assert!(!pending.same_request(different_target));
    assert_eq!(
        Request::new(
            &session,
            &descriptor,
            pending.request(),
            super::super::plan_stamp(2),
            pending.target(),
            pending.action()
        ),
        Err(InteractionHistoryRequestError::PlanOwnerMismatch)
    );
    assert_eq!(
        Request::new(
            &session,
            &descriptor,
            pending.request(),
            pending.plan(),
            super::super::buffer_state(),
            pending.action()
        ),
        Err(InteractionHistoryRequestError::ScopeMismatch)
    );
    let plan = pending.plan();
    let changed_cause = BufferPlanStamp::new(
        plan.owner(),
        plan.transaction(),
        plan.history_transition(),
        plan.cancellation(),
        super::super::BufferCorrelationIdentity::new(7_001),
        plan.deadline(),
    );
    let changed = Request::new(
        &session,
        &descriptor,
        pending.request(),
        changed_cause,
        pending.target(),
        pending.action(),
    )
    .expect("distinct cause");
    assert!(!pending.same_request(changed));
}

#[test]
fn replaced_session_and_history_generations_cannot_accept_old_work() {
    let session = session();
    let undo = request(&session, 5, FIRST, Action::Undo);
    let stamp = session.stamp();
    let lifecycle = stamp.lifecycle();
    let replacement = descriptor::InteractionSessionDescriptor::new(
        InteractionSessionStamp::new(
            stamp.session(),
            stamp.contract(),
            LifecycleIdentity::new(
                lifecycle.instance(),
                lifecycle.incarnation(),
                Generation::new(lifecycle.generation().value() + 1),
            ),
            stamp.client(),
        ),
        session.scope(),
        session.capabilities(),
        session.bounds().clone(),
        session.update_contract().clone(),
        session.operations(),
    )
    .expect("replacement Session");
    assert_eq!(
        undo.validate(&replacement, &super::super::buffer_descriptor()),
        Err(InteractionHistoryRequestError::StaleSession)
    );
    let result = state(&session, 6, PARENT);
    let history = result.history().lifecycle();
    let replaced = BufferStateStamp::new(
        result.source(),
        result.buffer(),
        result.version(),
        BufferHistoryStamp::new(
            LifecycleIdentity::new(
                history.instance(),
                history.incarnation(),
                Generation::new(history.generation().value() + 1),
            ),
            result.history().node(),
        ),
    );
    assert_eq!(
        Outcome::accepted_history(undo, edge(PARENT, FIRST), replaced),
        Err(InteractionOutcomeError::BufferMismatch)
    );
}
