use reovim_kernel_interface::bound::{CheckedCount, ZeroPolicy};
use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
    InteractionCloseRetainedState, InteractionIncompleteClose,
};

use super::{Family, Session};

pub fn close<Model: Family>(
    session: &mut Session<Model>,
    request: InteractionCloseRequest,
) -> InteractionCloseOutcome {
    if !request.session().matches(session.fixture.session.stamp()) {
        return InteractionCloseOutcome::StaleSession;
    }
    // Fence admission first. Pending producers and their results stay owned.
    session.closed = true;
    session.cancelled = true;
    if session.pending.is_some()
        || session.history_pending.is_some()
        || session.outcome.is_some()
        || session.history_outcome.is_some()
    {
        return incomplete(session);
    }
    // This fixture's full snapshots/notifications are discardable on detach.
    // It never discards an unknown or undelivered editing outcome here.
    session.update = None;
    session.delivered = None;
    session.notice = None;
    session.notice_gap = None;
    session.subscription = None;
    session.completed = None;
    session.history_completed = None;
    session.history_older = None;
    session.attachment_state = super::AttachmentState::Unnegotiated;
    InteractionCloseOutcome::Closed
}

fn incomplete<Model: Family>(
    session: &Session<Model>,
) -> InteractionCloseOutcome {
    let pending = u64::from(session.pending.is_some())
        + u64::from(session.history_pending.is_some());
    let outcomes = u64::from(session.outcome.is_some())
        + u64::from(session.history_outcome.is_some());
    let retained = InteractionCloseRetainedState::new(
        count(pending),
        count(outcomes),
        count(u64::from(session.update.is_some())),
        count(u64::from(session.delivered.is_some())),
        count(0),
        None,
    );
    let execution = if pending == 0 {
        ExecutionOutcome::Stopped
    } else {
        ExecutionOutcome::OwnedNonterminal
    };
    InteractionCloseOutcome::Incomplete(
        InteractionIncompleteClose::new(
            session.fixture.session.bounds(),
            execution,
            retained,
        )
        .expect("bounded pending/result slots remain explicitly owned"),
    )
}

fn count(value: u64) -> CheckedCount {
    CheckedCount::new(value, ZeroPolicy::Allow).expect("bounded slot count")
}
