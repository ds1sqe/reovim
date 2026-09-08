use reovim_kernel_interface::cancellation::{
    CancellationOutcome, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionIntentAdmission,
};
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;

use super::{Family, Session};

pub fn submit<Model: Family>(
    session: &mut Session<Model>,
    intent: InteractionIntent,
) -> InteractionIntentAdmission {
    if let Some(cause) = admission_failure(session, &intent) {
        return InteractionIntentAdmission::not_admitted(intent, cause);
    }
    let admitted = InteractionIntentAdmission::admitted(&intent);
    if let Some((retained, outcome)) = &session.completed
        && retained.same_request(&intent)
    {
        session.outcome = Some(*outcome);
        return admitted;
    }
    if session.pending.is_some() {
        return admitted; // Identical pending replay retains its original owner.
    }
    session.cancelled = false;
    session.pending = Some(intent);
    admitted
}

fn admission_failure<Model: Family>(
    session: &Session<Model>,
    intent: &InteractionIntent,
) -> Option<InteractionOutcomeCause> {
    if session.closed {
        return Some(InteractionOutcomeCause::Closed);
    }
    if !intent.stamp().session().matches(session.fixture.session.stamp())
        || !intent
            .stamp()
            .attachment()
            .same_generation(session.fixture.attachment.handle())
    {
        return Some(InteractionOutcomeCause::Replaced);
    }
    if !session.ready() || !session.is_negotiated() || !session.is_attached() {
        return Some(InteractionOutcomeCause::Denied);
    }
    if let Some(retained) = &session.pending {
        return replay_failure(retained, intent);
    }
    if let Some((retained, _)) = &session.completed
        && retained.stamp().request() == intent.stamp().request()
    {
        return replay_failure(retained, intent);
    }
    if same_history_id(session, intent) {
        return Some(InteractionOutcomeCause::InvalidIntent);
    }
    if session.history_pending.is_some()
        || session.outcome.is_some()
        || session.history_outcome.is_some()
    {
        return Some(InteractionOutcomeCause::Pressure);
    }
    if !intent.semantic().stamp().target().same_state(session.current_stamp()) {
        return Some(InteractionOutcomeCause::StaleState);
    }
    // The one-edit script retains its complete replay record until detach.
    // A fresh edit cannot silently evict it and permit an old ID to run again.
    if session.completed.is_some() {
        return Some(InteractionOutcomeCause::Pressure);
    }
    let retained = u64::from(session.history_completed.is_some())
        + u64::from(session.history_older.is_some());
    let Some(retained_bytes) =
        retained.checked_mul(intent.charge().outcome_bytes().value())
    else {
        return Some(InteractionOutcomeCause::Pressure);
    };
    if session
        .fixture
        .session
        .bounds()
        .requests()
        .admit_pending(0, 0, retained, retained_bytes, intent.charge())
        .is_err()
    {
        return Some(InteractionOutcomeCause::Pressure);
    }
    None
}

fn replay_failure(
    retained: &InteractionIntent,
    requested: &InteractionIntent,
) -> Option<InteractionOutcomeCause> {
    if retained.same_request(requested) {
        return None;
    }
    if retained.stamp().request() == requested.stamp().request() {
        return Some(InteractionOutcomeCause::InvalidIntent);
    }
    Some(InteractionOutcomeCause::Pressure)
}

fn same_history_id<Model: Family>(
    session: &Session<Model>,
    intent: &InteractionIntent,
) -> bool {
    let request = intent.stamp().request();
    session.history_pending.is_some_and(|value| value.request() == request)
        || session
            .history_older
            .is_some_and(|value| value.request().request() == request)
        || session
            .history_completed
            .is_some_and(|value| value.request().request() == request)
}

pub fn cancel<Model: Family>(
    session: &mut Session<Model>,
    request: &InteractionCancellationRequest,
) -> InteractionCancellationResult {
    if !request.request().session().matches(session.fixture.session.stamp()) {
        return InteractionCancellationResult::StaleSession;
    }
    if session.completed.as_ref().is_some_and(|(intent, _)| {
        intent.stamp().same_request(request.request())
    }) {
        return InteractionCancellationResult::AlreadyTerminal;
    }
    let Some(pending) = &session.pending else {
        return InteractionCancellationResult::UnknownRequest;
    };
    if !pending.stamp().same_request(request.request()) {
        return InteractionCancellationResult::UnknownRequest;
    }
    let outcome = CancellationOutcome::new(
        request.cause(),
        ExecutionOutcome::OwnedNonterminal,
    );
    if session.cancelled {
        return InteractionCancellationResult::AlreadyRequested(outcome);
    }
    session.cancelled = true;
    InteractionCancellationResult::Requested(outcome)
}
