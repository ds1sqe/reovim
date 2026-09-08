use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionOutcomeDelivery, InteractionOutcomeRequest,
    InteractionOutcomeUnavailable, InteractionReconcileRequest,
    InteractionReconcileResult, InteractionReconcileUnavailable,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateDelivery,
    RepresentationUpdateRequest, RepresentationUpdateUnavailable,
};

use super::{Family, Session};

pub fn outcome<Model: Family>(
    session: &mut Session<Model>,
    request: &InteractionOutcomeRequest,
) -> InteractionOutcomeDelivery {
    if !request.session().matches(session.fixture.session.stamp()) {
        return InteractionOutcomeDelivery::unavailable(
            InteractionOutcomeUnavailable::Replaced,
        );
    }
    if let Some(outcome) = session.outcome
        && outcome.request().same_request(request.request())
    {
        session.outcome = None;
        return InteractionOutcomeDelivery::delivered(outcome);
    }
    let cause = missing_outcome(session, request);
    InteractionOutcomeDelivery::unavailable(cause)
}

fn missing_outcome<Model: Family>(
    session: &Session<Model>,
    request: &InteractionOutcomeRequest,
) -> InteractionOutcomeUnavailable {
    if session
        .pending
        .as_ref()
        .is_some_and(|intent| intent.stamp().same_request(request.request()))
    {
        return InteractionOutcomeUnavailable::Pending;
    }
    if session.completed.as_ref().is_some_and(|(intent, _)| {
        intent.stamp().same_request(request.request())
    }) {
        return InteractionOutcomeUnavailable::AlreadyDelivered;
    }
    if session.closed {
        return InteractionOutcomeUnavailable::Closed;
    }
    InteractionOutcomeUnavailable::UnknownRequest
}

pub fn reconcile<Model: Family>(
    session: &Session<Model>,
    request: &InteractionReconcileRequest,
) -> InteractionReconcileResult {
    if !request.request().session().matches(session.fixture.session.stamp()) {
        return InteractionReconcileResult::unavailable(
            InteractionReconcileUnavailable::StaleSession,
        );
    }
    // This finite script issues no uncertainty/reconciliation token. A caller
    // cannot invent one to recover an already-transferred terminal outcome.
    let cause =
        if session.pending.as_ref().is_some_and(|intent| {
            intent.stamp().same_request(request.request())
        }) {
            InteractionReconcileUnavailable::Pending
        } else {
            InteractionReconcileUnavailable::UnknownRequest
        };
    InteractionReconcileResult::unavailable(cause)
}

pub fn update<Model: Family>(
    session: &mut Session<Model>,
    request: RepresentationUpdateRequest,
) -> RepresentationUpdateDelivery {
    if session.closed {
        return RepresentationUpdateDelivery::unavailable(
            RepresentationUpdateUnavailable::Closed,
        );
    }
    if request.stream() != session.fixture.attachment.stream().handle() {
        return RepresentationUpdateDelivery::unavailable(
            RepresentationUpdateUnavailable::Replaced,
        );
    }
    if session.delivered.is_some() {
        return RepresentationUpdateDelivery::unavailable(
            RepresentationUpdateUnavailable::NoCredit,
        );
    }
    let Some(candidate) = session.update.as_ref() else {
        return RepresentationUpdateDelivery::unavailable(
            RepresentationUpdateUnavailable::Pending,
        );
    };
    let stamp = candidate.stamp().identity();
    match RepresentationUpdateDelivery::try_bind(&request, &mut session.update)
    {
        Ok(delivery) => {
            session.delivered = Some(stamp);
            delivery
        }
        Err(cause) => RepresentationUpdateDelivery::unavailable(
            RepresentationUpdateUnavailable::ProviderContractViolation(cause),
        ),
    }
}

pub fn acknowledge<Model: Family>(
    session: &mut Session<Model>,
    acknowledgement: RepresentationUpdateAcknowledgement,
) -> RepresentationUpdateAcknowledgementResult {
    if session.closed {
        return RepresentationUpdateAcknowledgementResult::Closed;
    }
    if !session
        .delivered
        .is_some_and(|stamp| stamp.same_update(acknowledgement.stamp()))
    {
        return RepresentationUpdateAcknowledgementResult::UnknownUpdate;
    }
    session.delivered = None;
    RepresentationUpdateAcknowledgementResult::Accepted
}
