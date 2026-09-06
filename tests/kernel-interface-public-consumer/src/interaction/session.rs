use reovim_kernel_interface::bound::{CheckedCount, ZeroPolicy};
use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::interaction::bounds::InteractionBoundField;
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseError, InteractionCloseRequest,
    InteractionCloseRetainedState, InteractionIncompleteClose,
    InteractionNegotiationFailure, InteractionNegotiationOutcome,
    InteractionNegotiationRequest, InteractionNegotiationRequestError,
    InteractionNegotiationStatus, RepresentationAttachmentFailure,
    RepresentationAttachmentOutcome, RepresentationAttachmentReady,
    RepresentationAttachmentReadyBindingOutcome,
    RepresentationAttachmentReadyError, RepresentationAttachmentRejection,
    RepresentationAttachmentRequest,
    RepresentationAttachmentRequestBindingOutcome,
    RepresentationAttachmentRequestError,
    RepresentationAttachmentResynchronizing, RepresentationAttachmentStatus,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionCapabilityError, InteractionCapabilitySupport,
    InteractionSessionCapabilities,
};
use reovim_kernel_interface::interaction::identity::{
    ClientIdentity, InteractionSessionIdentity, RepresentationUpdateCursor,
};

use super::support;

pub const fn const_cases() -> bool {
    let next = match RepresentationUpdateCursor::new(4).checked_next() {
        Some(value) => value.value() == 5,
        None => false,
    };
    ClientIdentity::new(1).value() == 1
        && InteractionSessionIdentity::new(2).value() == 2
        && RepresentationUpdateCursor::INITIAL.value() == 0
        && next
        && RepresentationUpdateCursor::new(u64::MAX).checked_next().is_none()
}

pub fn exercise() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    negotiation_cases(&fixture)
        && attachment_cases(&fixture)
        && close_cases(&fixture)
        && capability_faults()
}

fn negotiation_cases(fixture: &support::InteractionFixture) -> bool {
    let request = InteractionNegotiationRequest::new(
        &fixture.session,
        fixture.session.stamp(),
        fixture.session.scope(),
        fixture.session.capabilities(),
    );
    let Some(request) = request.ok() else { return false };
    let accepted =
        InteractionNegotiationOutcome::negotiated(request.session().handle());
    let rejected = InteractionNegotiationOutcome::rejected(
        InteractionNegotiationFailure::Denied,
    );
    let Some(replacement) = support::replacement_session(fixture) else {
        return false;
    };
    let mismatched = InteractionNegotiationRequest::new(
        &fixture.session,
        replacement.stamp(),
        fixture.session.scope(),
        fixture.session.capabilities(),
    );
    request.scope().same_scope(fixture.session.scope())
        && request.capabilities() == fixture.session.capabilities()
        && accepted.status() == InteractionNegotiationStatus::Negotiated
        && rejected.status()
            == InteractionNegotiationStatus::Rejected(
                InteractionNegotiationFailure::Denied,
            )
        && mismatched.err()
            == Some(InteractionNegotiationRequestError::SessionMismatch)
}

fn attachment_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(initial) = support::initial_update(fixture) else { return false };
    let ready = RepresentationAttachmentReady::bind(
        &fixture.session,
        fixture.attachment.clone(),
        initial,
    );
    let RepresentationAttachmentReadyBindingOutcome::Bound(ready) = ready
    else {
        return false;
    };
    let initial_matches =
        ready.initial().validate(&fixture.session, ready.descriptor()).is_ok();
    let request = RepresentationAttachmentRequest::bind(
        &fixture.session,
        fixture.attachment.clone(),
    );
    let RepresentationAttachmentRequestBindingOutcome::Bound(request) = request
    else {
        return false;
    };
    let rejected = RepresentationAttachmentOutcome::rejected(
        RepresentationAttachmentRejection::new(
            RepresentationAttachmentFailure::Pressure,
            request,
        ),
    );
    let resynchronizing = RepresentationAttachmentOutcome::resynchronizing(
        RepresentationAttachmentResynchronizing::new(
            fixture.attachment.clone(),
            reovim_kernel_interface::interaction::identity::
                RepresentationResynchronizationIdentity::new(9),
        ),
    );
    let Some(replacement) = support::replacement_session(fixture) else {
        return false;
    };
    let mismatched = RepresentationAttachmentRequest::bind(
        &replacement,
        fixture.attachment.clone(),
    );
    let mismatch_retained = match mismatched {
        RepresentationAttachmentRequestBindingOutcome::Rejected(rejection) => {
            rejection.cause()
                == RepresentationAttachmentRequestError::SessionMismatch
                && rejection
                    .descriptor()
                    .handle()
                    .same_generation(fixture.attachment.handle())
        }
        RepresentationAttachmentRequestBindingOutcome::Bound(_) => false,
    };
    let Some(delta) = support::delta_update(fixture) else { return false };
    let delta_stamp = delta.stamp().identity();
    let invalid_initial = RepresentationAttachmentReady::bind(
        &fixture.session,
        fixture.attachment.clone(),
        delta,
    );
    let invalid_initial_retained = match invalid_initial {
        RepresentationAttachmentReadyBindingOutcome::Rejected(rejection) => {
            rejection.cause()
                == RepresentationAttachmentReadyError::NotFullSnapshot
                && rejection
                    .initial()
                    .stamp()
                    .identity()
                    .same_update(delta_stamp)
        }
        RepresentationAttachmentReadyBindingOutcome::Bound(_) => false,
    };
    initial_matches
        && rejected.status()
            == RepresentationAttachmentStatus::Rejected(
                RepresentationAttachmentFailure::Pressure,
            )
        && resynchronizing.status()
            == RepresentationAttachmentStatus::Resynchronizing
        && mismatch_retained
        && invalid_initial_retained
}

fn close_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(close) = InteractionCloseRequest::new(&fixture.session, 4).ok()
    else {
        return false;
    };
    let Some(zero) = CheckedCount::new(0, ZeroPolicy::Allow).ok() else {
        return false;
    };
    let Some(one) = CheckedCount::new(1, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let Some(large) = CheckedCount::new(5, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let invalid = InteractionIncompleteClose::new(
        fixture.session.bounds(),
        ExecutionOutcome::Stopped,
        InteractionCloseRetainedState::new(zero, zero, zero, zero, zero, None),
    );
    let pending = InteractionIncompleteClose::new(
        fixture.session.bounds(),
        ExecutionOutcome::OwnedNonterminal,
        InteractionCloseRetainedState::new(one, zero, zero, zero, zero, None),
    );
    let oversized = InteractionIncompleteClose::new(
        fixture.session.bounds(),
        ExecutionOutcome::OwnedNonterminal,
        InteractionCloseRetainedState::new(large, zero, zero, zero, zero, None),
    );
    let oversized_pending = match oversized {
        Err(InteractionCloseError::Bound(error)) => {
            error.field() == Some(InteractionBoundField::PendingIntents)
        }
        _ => false,
    };
    close.session().matches(fixture.session.stamp())
        && close.work().value() == 4
        && invalid.is_err()
        && pending.is_ok()
        && oversized_pending
}

const fn capability_faults() -> bool {
    let unsupported = InteractionCapabilitySupport::Unsupported;
    let supported = InteractionCapabilitySupport::Supported;
    let delta = InteractionSessionCapabilities::new(
        supported,
        supported,
        unsupported,
        supported,
        unsupported,
    );
    let resync = InteractionSessionCapabilities::new(
        supported,
        supported,
        unsupported,
        unsupported,
        supported,
    );
    capability_result_code(delta) == 1 && capability_result_code(resync) == 2
}

const fn capability_result_code(
    result: Result<InteractionSessionCapabilities, InteractionCapabilityError>,
) -> u8 {
    match result {
        Ok(_) => 0,
        Err(InteractionCapabilityError::DeltaWithoutUpdates) => 1,
        Err(InteractionCapabilityError::ResynchronizationWithoutUpdates) => 2,
    }
}
