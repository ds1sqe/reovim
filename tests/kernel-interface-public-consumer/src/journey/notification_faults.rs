use alloc::rc::Rc;
use core::cell::RefCell;

use reovim_kernel_interface::bound::{CheckedCount, ZeroPolicy};
use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferHistoryStamp, BufferIdentity,
    BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::interaction::history::{
    InteractionHistoryAction, InteractionHistoryOutcome,
    InteractionHistoryRequest,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDelivery, BufferChangeError, BufferChangeNotification,
    BufferChangeSubscription,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause,
};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};

use super::session::{Session, Shared};
use crate::first_edit::native::{models::ByteModel, support as native};
use crate::interaction::support;

/// Exercises notification construction without claiming executed editing.
///
/// # Panics
///
/// Panics if a public correlation, scope or checkpoint fence is bypassed.
#[must_use]
pub fn exercise_notification_fences() -> bool {
    let session = endpoint();
    rejected_and_substituted_outcomes(&session);
    changed_initial_history(&session);
    gap_before_initial_frontier(&session);
    stale_and_wrong_scope(&session);
    true
}

fn rejected_and_substituted_outcomes(session: &Session<ByteModel>) {
    let intent = session.intent(6100, b"B");
    let rejected = InteractionOutcome::not_accepted(
        intent.stamp(),
        InteractionOutcomeCause::Conflict,
    );
    assert_eq!(
        BufferChangeNotification::from_edit(rejected, &intent),
        Err(BufferChangeError::NotAccepted),
    );
    let result = state(&session.current_stamp(), 1, 6);
    let accepted = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        result,
    )
    .expect("structural accepted edit");
    let other = session.intent(6200, b"B");
    assert_eq!(
        BufferChangeNotification::from_edit(accepted, &other),
        Err(BufferChangeError::RequestMismatch),
    );
    let history = InteractionHistoryRequest::new(
        &session.fixture.session,
        &session.fixture.semantic.buffer,
        InteractionRequestIdentity::new(6300),
        native::plan_stamp(6301),
        session.current_stamp(),
        InteractionHistoryAction::Undo,
    )
    .expect("root History remains requestable");
    let missing = InteractionHistoryOutcome::not_accepted(
        history,
        InteractionOutcomeCause::NoHistory,
    );
    assert_eq!(
        BufferChangeNotification::from_history(missing),
        Err(BufferChangeError::NotAccepted),
    );
}

fn changed_initial_history(session: &Session<ByteModel>) {
    let initial = session.current_stamp();
    let subscription = subscription(session, &initial);
    let changed = state(&initial, initial.version().value(), 7);
    let intent = session.intent_at(6400, b"B", &changed);
    let accepted = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        state(&changed, 1, 8),
    )
    .expect("coherent transition over a different initial History node");
    let notification = BufferChangeNotification::from_edit(accepted, &intent)
        .expect("source request/result are internally coherent");
    assert_eq!(
        BufferChangeDelivery::changed(
            &subscription,
            initial.version(),
            notification,
        ),
        Err(BufferChangeError::CheckpointMismatch),
    );
}

fn gap_before_initial_frontier(session: &Session<ByteModel>) {
    let initial = state(&session.current_stamp(), 5, 700);
    let subscription = subscription(session, &initial);
    assert_eq!(
        BufferChangeDelivery::gap(
            &subscription,
            BufferVersion::new(3),
            state(&initial, 4, 701),
        ),
        Err(BufferChangeError::CheckpointMismatch),
    );
}

fn stale_and_wrong_scope(session: &Session<ByteModel>) {
    let initial = session.current_stamp();
    let subscription = subscription(session, &initial);
    let replacement = support::replacement_session(&session.fixture)
        .expect("replacement observer Session");
    assert_eq!(
        subscription.validate(&replacement),
        Err(BufferChangeError::StaleSubscription),
    );
    let foreign = BufferStateStamp::new(
        initial.source(),
        BufferIncarnationStamp::new(
            BufferIdentity::new(9900),
            initial.buffer().lifecycle(),
        ),
        initial.version(),
        initial.history(),
    );
    assert_eq!(
        BufferChangeSubscription::new(
            &session.fixture.session,
            lifecycle(),
            foreign,
            count(),
        ),
        Err(BufferChangeError::ScopeMismatch),
    );
    assert_eq!(
        BufferChangeDelivery::gap(&subscription, initial.version(), foreign),
        Err(BufferChangeError::ScopeMismatch),
    );
}

fn endpoint() -> Session<ByteModel> {
    let client = support::fixture()
        .expect("existing client fixture")
        .session
        .stamp()
        .client();
    Session::new(Rc::new(RefCell::new(Shared::new(ByteModel::new()))), client)
}

fn subscription(
    session: &Session<ByteModel>,
    checkpoint: &BufferStateStamp,
) -> BufferChangeSubscription {
    BufferChangeSubscription::new(
        &session.fixture.session,
        lifecycle(),
        *checkpoint,
        count(),
    )
    .expect("bounded exact notification checkpoint")
}

const fn lifecycle() -> LifecycleIdentity {
    LifecycleIdentity::new(6500, 6501, Generation::new(1))
}

fn count() -> CheckedCount {
    CheckedCount::new(1, ZeroPolicy::Reject).expect("one retained notice")
}

const fn state(
    before: &BufferStateStamp,
    version: u64,
    node: u128,
) -> BufferStateStamp {
    BufferStateStamp::new(
        before.source(),
        before.buffer(),
        BufferVersion::new(version),
        BufferHistoryStamp::new(
            before.history().lifecycle(),
            BufferHistoryNodeIdentity::new(node),
        ),
    )
}
