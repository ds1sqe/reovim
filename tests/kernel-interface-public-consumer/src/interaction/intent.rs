use reovim_kernel_interface::bound::BoundValue;
use reovim_kernel_interface::cancellation::{
    CancellationCause, CancellationState, DeadlineState,
};
use reovim_kernel_interface::effect::EffectCertainty;
use reovim_kernel_interface::interaction::bounds::{
    InteractionBoundField, InteractionBoundViolation,
};
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionIntentAdmission,
    InteractionIntentAdmissionError, InteractionIntentNonAdmission,
    InteractionIntentNonAdmissionBindingOutcome, InteractionOutcomeDelivery,
    InteractionOutcomeDeliveryStatus, InteractionOutcomeRequest,
    InteractionOutcomeUnavailable, InteractionReconcileRequest,
    InteractionReconcileResult, InteractionReconcileStatus,
    InteractionReconcileUnavailable,
};
use reovim_kernel_interface::interaction::identity;
use reovim_kernel_interface::interaction::intent::{
    InteractionIntent, InteractionIntentBindingOutcome, InteractionIntentError,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause, InteractionOutcomeError,
    InteractionOutcomeStatus,
};
use reovim_kernel_interface::interaction::session::InteractionRequestStamp;

use super::support;

pub const fn const_cases() -> bool {
    let _ = InteractionOutcomeCause::Conflict;
    let _ = InteractionOutcomeStatus::Accepted;
    true
}

pub fn exercise() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    admission_cases(&fixture)
        && outcome_cases(&fixture)
        && request_cases(&fixture)
        && ownership_fault(&fixture)
        && state_faults(&fixture)
        && authority_faults(&fixture)
        && bound_cases(&fixture)
        && reservation_fault(&fixture)
}

fn admission_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(admitted) = support::make_intent(fixture, 2_000, 2_001, 2_002)
    else {
        return false;
    };
    let stamp = admitted.stamp();
    let admitted_result = InteractionIntentAdmission::admitted(&admitted);
    let Some(returned) = support::make_intent(fixture, 2_010, 2_011, 2_012)
    else {
        return false;
    };
    let outcome = InteractionOutcome::not_accepted(
        returned.stamp(),
        InteractionOutcomeCause::Pressure,
    );
    let InteractionIntentNonAdmissionBindingOutcome::Bound(non_admission) =
        InteractionIntentNonAdmission::bind(outcome, returned)
    else {
        return false;
    };
    let returned_bytes_match = non_admission.intent().semantic().bytes()
        == crate::semantic::support::INTENT_BYTES;
    let not_admitted = InteractionIntentAdmission::not_admitted(
        non_admission.into_parts().1,
        InteractionOutcomeCause::Pressure,
    );
    admitted_result.admitted_request() == Some(stamp)
        && not_admitted.non_admission().is_some()
        && returned_bytes_match
        && non_admission_faults(fixture)
}

fn non_admission_faults(fixture: &support::InteractionFixture) -> bool {
    wrong_non_admission_request(fixture) && accepted_non_admission(fixture)
}

fn wrong_non_admission_request(fixture: &support::InteractionFixture) -> bool {
    let Some(intent) = support::make_intent(fixture, 2_100, 2_101, 2_102)
    else {
        return false;
    };
    let Some(other) = support::make_intent(fixture, 2_110, 2_111, 2_112) else {
        return false;
    };
    let outcome = InteractionOutcome::not_accepted(
        other.stamp(),
        InteractionOutcomeCause::Pressure,
    );
    let binding = InteractionIntentNonAdmission::bind(outcome, intent);
    match binding {
        InteractionIntentNonAdmissionBindingOutcome::Rejected(rejection) => {
            rejection.cause()
                == InteractionIntentAdmissionError::OutcomeRequestMismatch
                && rejection.intent().semantic().bytes()
                    == crate::semantic::support::INTENT_BYTES
        }
        InteractionIntentNonAdmissionBindingOutcome::Bound(_) => false,
    }
}

fn accepted_non_admission(fixture: &support::InteractionFixture) -> bool {
    let Some(intent) = support::make_intent(fixture, 2_120, 2_121, 2_122)
    else {
        return false;
    };
    let Some(result) = crate::buffer::state(1, 1, 1) else { return false };
    let outcome = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        result,
    );
    let Some(outcome) = outcome.ok() else { return false };
    let binding = InteractionIntentNonAdmission::bind(outcome, intent);
    match binding {
        InteractionIntentNonAdmissionBindingOutcome::Rejected(rejection) => {
            rejection.cause()
                == InteractionIntentAdmissionError::OutcomeNotDefinitelyRejected
                && rejection.intent().semantic().bytes()
                    == crate::semantic::support::INTENT_BYTES
        }
        InteractionIntentNonAdmissionBindingOutcome::Bound(_) => false,
    }
}

fn outcome_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(intent) = support::make_intent(fixture, 2_020, 2_021, 2_022)
    else {
        return false;
    };
    let Some(result) = crate::buffer::state(1, 1, 1) else { return false };
    let Some(accepted) = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        result,
    )
    .ok() else {
        return false;
    };
    let rejected = InteractionOutcome::not_accepted(
        intent.stamp(),
        InteractionOutcomeCause::Conflict,
    );
    let reconcile = InteractionOutcome::reconciliation_required(
        intent.stamp(),
        InteractionOutcomeCause::CallerTimeout,
        identity::InteractionReconcileIdentity::new(2_023),
    );
    let invalid = InteractionOutcome::reconciliation_required(
        intent.stamp(),
        InteractionOutcomeCause::Conflict,
        identity::InteractionReconcileIdentity::new(2_024),
    );
    accepted.status() == InteractionOutcomeStatus::Accepted
        && accepted.effect_certainty() == EffectCertainty::Applied
        && accepted.accepted_state().is_some()
        && rejected.effect_certainty() == EffectCertainty::DefinitelyNotApplied
        && reconcile.is_ok_and(|value| {
            value.effect_certainty() == EffectCertainty::OutcomeUnknown
        })
        && invalid.err()
            == Some(InteractionOutcomeError::CauseCannotRequireReconciliation)
        && accepted_outcome_faults(fixture, &intent)
}

fn accepted_outcome_faults(
    fixture: &support::InteractionFixture,
    intent: &InteractionIntent,
) -> bool {
    let Some(version) = crate::buffer::state(1, 2, 1) else { return false };
    let Some(history) = crate::buffer::state(1, 1, 0) else { return false };
    let Some(buffer) = crate::buffer::state(2, 1, 1) else { return false };
    let wrong_version = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        version,
    );
    let wrong_history = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        history,
    );
    let wrong_buffer = InteractionOutcome::accepted(
        intent.stamp(),
        intent.semantic().stamp(),
        buffer,
    );
    let version_failed = match wrong_version {
        Err(InteractionOutcomeError::BufferVersionNotAdvanced {
            expected,
            actual,
        }) => expected.value() == 1 && actual.value() == 2,
        _ => false,
    };
    let history_failed = wrong_history.err()
        == Some(InteractionOutcomeError::HistoryEdgeMismatch);
    version_failed
        && history_failed
        && wrong_buffer.err() == Some(InteractionOutcomeError::BufferMismatch)
        && fixture.session.scope().contains(*intent.semantic().stamp().target())
}

fn request_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(intent) = support::make_intent(fixture, 2_030, 2_031, 2_032)
    else {
        return false;
    };
    let cancellation = InteractionCancellationRequest::new(
        &fixture.session,
        intent.stamp(),
        CancellationCause::ExplicitRequest,
    );
    let outcome =
        InteractionOutcomeRequest::new(&fixture.session, intent.stamp());
    let reconcile = InteractionReconcileRequest::new(
        &fixture.session,
        intent.stamp(),
        identity::InteractionReconcileIdentity::new(2_033),
    );
    cancellation.is_ok()
        && outcome.is_ok()
        && reconcile.is_ok()
        && InteractionOutcomeDelivery::unavailable(
            InteractionOutcomeUnavailable::Pending,
        )
        .status()
            == InteractionOutcomeDeliveryStatus::Unavailable(
                InteractionOutcomeUnavailable::Pending,
            )
        && InteractionReconcileResult::<InteractionRequestStamp>::unavailable(
            InteractionReconcileUnavailable::Pending,
        )
        .status()
            == InteractionReconcileStatus::Unavailable(
                InteractionReconcileUnavailable::Pending,
            )
}

fn ownership_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(semantic) = support::make_semantic_intent(fixture, 2_040, 2_041)
    else {
        return false;
    };
    let Some(stamp) = support::request_stamp(fixture, &semantic, 2_042) else {
        return false;
    };
    let Some(charge) = fixture
        .session
        .bounds()
        .requests()
        .prepare_intent(semantic.shape().bytes().value() + 1)
        .ok()
    else {
        return false;
    };
    let rejected = InteractionIntent::bind(
        &fixture.session,
        &fixture.attachment,
        stamp,
        charge,
        semantic,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    match rejected {
        InteractionIntentBindingOutcome::Rejected(rejection) => {
            let charge_mismatch = rejection.cause()
                == InteractionIntentError::IntentByteChargeMismatch {
                    reserved: 5,
                    actual: 4,
                };
            charge_mismatch
                && rejection.semantic().bytes()
                    == crate::semantic::support::INTENT_BYTES
        }
        InteractionIntentBindingOutcome::Bound(_) => false,
    }
}

fn state_faults(fixture: &support::InteractionFixture) -> bool {
    cancelled_fault(fixture) && deadline_fault(fixture)
}

fn cancelled_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(semantic) = support::make_semantic_intent(fixture, 2_050, 2_051)
    else {
        return false;
    };
    let Some(stamp) = support::request_stamp(fixture, &semantic, 2_052) else {
        return false;
    };
    let Some(charge) = fixture
        .session
        .bounds()
        .requests()
        .prepare_intent(semantic.shape().bytes().value())
        .ok()
    else {
        return false;
    };
    let outcome = InteractionIntent::bind(
        &fixture.session,
        &fixture.attachment,
        stamp,
        charge,
        semantic,
        CancellationState::Requested(CancellationCause::ExplicitRequest),
        DeadlineState::Pending,
    );
    rejected_intent_has(outcome, InteractionIntentError::Cancelled)
}

fn deadline_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(semantic) = support::make_semantic_intent(fixture, 2_060, 2_061)
    else {
        return false;
    };
    let Some(stamp) = support::request_stamp(fixture, &semantic, 2_062) else {
        return false;
    };
    let Some(charge) = fixture
        .session
        .bounds()
        .requests()
        .prepare_intent(semantic.shape().bytes().value())
        .ok()
    else {
        return false;
    };
    let outcome = InteractionIntent::bind(
        &fixture.session,
        &fixture.attachment,
        stamp,
        charge,
        semantic,
        CancellationState::Open,
        DeadlineState::Reached,
    );
    rejected_intent_has(outcome, InteractionIntentError::DeadlineReached)
}

fn authority_faults(fixture: &support::InteractionFixture) -> bool {
    session_fault(fixture) && charge_owner_fault(fixture)
}

fn session_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(semantic) = support::make_semantic_intent(fixture, 2_070, 2_071)
    else {
        return false;
    };
    let Some(stamp) = support::request_stamp(fixture, &semantic, 2_072) else {
        return false;
    };
    let Some(charge) = fixture
        .session
        .bounds()
        .requests()
        .prepare_intent(semantic.shape().bytes().value())
        .ok()
    else {
        return false;
    };
    let Some(replacement) = support::replacement_session(fixture) else {
        return false;
    };
    let outcome = InteractionIntent::bind(
        &replacement,
        &fixture.attachment,
        stamp,
        charge,
        semantic,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    rejected_intent_has(outcome, InteractionIntentError::SessionMismatch)
}

fn charge_owner_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(semantic) = support::make_semantic_intent(fixture, 2_080, 2_081)
    else {
        return false;
    };
    let Some(stamp) = support::request_stamp(fixture, &semantic, 2_082) else {
        return false;
    };
    let Some(charge) =
        support::foreign_intent_charge(semantic.shape().bytes().value())
    else {
        return false;
    };
    let outcome = InteractionIntent::bind(
        &fixture.session,
        &fixture.attachment,
        stamp,
        charge,
        semantic,
        CancellationState::Open,
        DeadlineState::Pending,
    );
    match outcome {
        InteractionIntentBindingOutcome::Rejected(rejection) => {
            let owner = match rejection.cause() {
                InteractionIntentError::Bound(
                    reovim_kernel_interface::interaction::bounds::
                        InteractionBoundViolation::ChargeOwnerMismatch {
                            expected,
                            actual,
                        },
                ) => expected.value() != actual.value(),
                _ => false,
            };
            owner
                && rejection.semantic().bytes()
                    == crate::semantic::support::INTENT_BYTES
        }
        InteractionIntentBindingOutcome::Bound(_) => false,
    }
}

fn rejected_intent_has(
    outcome: InteractionIntentBindingOutcome,
    expected: InteractionIntentError,
) -> bool {
    match outcome {
        InteractionIntentBindingOutcome::Rejected(rejection) => {
            rejection.cause() == expected
                && rejection.semantic().bytes()
                    == crate::semantic::support::INTENT_BYTES
        }
        InteractionIntentBindingOutcome::Bound(_) => false,
    }
}

fn bound_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(charge) =
        fixture.session.bounds().requests().prepare_intent(4).ok()
    else {
        return false;
    };
    let accepted = fixture
        .session
        .bounds()
        .requests()
        .admit_pending(3, 200, 3, 700, charge);
    let pressure = fixture
        .session
        .bounds()
        .requests()
        .admit_pending(4, 200, 3, 700, charge);
    let outcome_pressure = fixture
        .session
        .bounds()
        .requests()
        .admit_pending(1, 100, 1, 800, charge);
    accepted.is_ok() && pressure.is_err() && outcome_pressure.is_err()
}

fn reservation_fault(fixture: &support::InteractionFixture) -> bool {
    let bounds = fixture.session.bounds().requests();
    let owner = bounds.charge_owner().value();
    let Some(smaller) = support::bounds_with_reservations(owner, 128, 8, 64)
    else {
        return false;
    };
    let Ok(charge) = smaller.requests().prepare_intent(4) else {
        return false;
    };
    let expected = InteractionBoundViolation::ReservationMismatch {
        field: InteractionBoundField::RetainedOutcomeBytes,
        expected: bounds.retained_outcome_bytes().per_object(),
        actual: BoundValue::Bytes(charge.outcome_bytes()),
    };
    bounds.validate_charge(charge) == Err(expected)
        && bounds.admit_pending(0, 0, 0, 0, charge) == Err(expected)
        && expected.field() == Some(InteractionBoundField::RetainedOutcomeBytes)
        && expected.cause().is_none()
}
