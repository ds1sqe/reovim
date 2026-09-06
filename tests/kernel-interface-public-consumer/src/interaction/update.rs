use reovim_kernel_interface::bound::BoundValue;
use reovim_kernel_interface::interaction::bounds::{
    InteractionBoundField, InteractionBoundViolation,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationResynchronizationAdmission,
    RepresentationResynchronizationFailure,
    RepresentationResynchronizationRequest,
    RepresentationResynchronizationRequestError,
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
    RepresentationUpdateDelivery, RepresentationUpdateDeliveryError,
    RepresentationUpdateDeliveryStatus, RepresentationUpdateRequest,
    RepresentationUpdateUnavailable,
};
use reovim_kernel_interface::interaction::identity::RepresentationUpdateCursor;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateBindingOutcome,
    RepresentationUpdateError, RepresentationUpdateLineage,
    RepresentationUpdatePayloadClaim, RepresentationUpdateStamp,
    RepresentationUpdateStampError,
};
use reovim_kernel_interface::semantic::identity::{
    ProjectionGeneration, ProjectionIdentity,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::support;

pub const fn const_cases() -> bool {
    let lineage = RepresentationUpdateLineage::Invalidation {
        invalidated: ProjectionGeneration::new(1),
    };
    !lineage.has_payload() && lineage.result().is_none()
}

pub fn exercise() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    update_cases(&fixture)
        && stamp_faults(&fixture)
        && replacement_fences(&fixture)
        && delivery_cases(&fixture)
        && delivery_binding_fault(&fixture)
        && resynchronization_cases(&fixture)
        && binding_fault(&fixture)
        && lineage_faults(&fixture)
        && capability_faults(&fixture)
        && bound_cases(&fixture)
        && reservation_faults(&fixture)
}

fn stamp_faults(fixture: &support::InteractionFixture) -> bool {
    let Some(target) = crate::buffer::state(1, 1, 1) else { return false };
    let projection = support::projection(fixture, 4);
    let coordinate = fixture.attachment.coordinate();
    let Some(replacement) = support::replacement_session(fixture) else {
        return false;
    };
    let session =
        stamp_error(fixture, &replacement, &target, &projection, coordinate);
    let Some(foreign_target) = crate::buffer::state(2, 1, 1) else {
        return false;
    };
    let scope = stamp_error(
        fixture,
        &fixture.session,
        &foreign_target,
        &projection,
        coordinate,
    );
    let attached = fixture.attachment.projection();
    let foreign_representation = crate::semantic::support::representation_stamp(
        fixture.semantic.domain.stamp(),
        2,
    );
    let Some(foreign_representation) = foreign_representation else {
        return false;
    };
    let foreign_representation = ProjectionStamp::new(
        attached.projection(),
        attached.lifecycle(),
        foreign_representation,
        attached.predecessor(),
    );
    let representation_error = stamp_error(
        fixture,
        &fixture.session,
        &target,
        &foreign_representation,
        coordinate,
    );
    let foreign_projection = ProjectionStamp::new(
        ProjectionIdentity::new(9_001),
        attached.lifecycle(),
        attached.representation(),
        attached.predecessor(),
    );
    let projection_error = stamp_error(
        fixture,
        &fixture.session,
        &target,
        &foreign_projection,
        coordinate,
    );
    let Some(foreign_coordinate) =
        crate::semantic::support::coordinate_stamp(9_002)
    else {
        return false;
    };
    let coordinate_error = stamp_error(
        fixture,
        &fixture.session,
        &target,
        &projection,
        foreign_coordinate,
    );
    session == Some(RepresentationUpdateStampError::SessionMismatch)
        && scope == Some(RepresentationUpdateStampError::ScopeMismatch)
        && representation_error
            == Some(RepresentationUpdateStampError::RepresentationMismatch)
        && projection_error
            == Some(RepresentationUpdateStampError::ProjectionMismatch)
        && coordinate_error
            == Some(RepresentationUpdateStampError::CoordinateMismatch)
}

fn replacement_fences(fixture: &support::InteractionFixture) -> bool {
    let Some(delta) = support::delta_update(fixture) else { return false };
    let Some(replacement) = support::replacement_attachment(fixture) else {
        return false;
    };
    let stamp = delta.stamp().identity();
    let unresolved =
        delta.stamp().resolve_target(&fixture.session, &replacement).err()
            == Some(RepresentationUpdateError::StreamMismatch);
    let invalid = delta.validate(&fixture.session, &replacement)
        == Err(RepresentationUpdateError::StreamMismatch);
    let request = RepresentationUpdateRequest::new(
        &fixture.session,
        &replacement,
        stamp.cursor(),
    );
    let Some(request) = request.ok() else { return false };
    let mut candidate = Some(delta);
    let result =
        RepresentationUpdateDelivery::try_bind(&request, &mut candidate);
    let rejected =
        result.err() == Some(RepresentationUpdateDeliveryError::StreamMismatch);
    let retained = candidate
        .as_ref()
        .is_some_and(|value| value.stamp().identity().same_update(stamp));
    let acknowledgement = RepresentationUpdateAcknowledgement::new(
        &fixture.session,
        &replacement,
        stamp,
        RepresentationUpdateApplication::Installed,
    );
    unresolved && invalid && rejected && retained && acknowledgement.is_err()
}

fn stamp_error(
    fixture: &support::InteractionFixture,
    session: &reovim_kernel_interface::interaction::descriptor::
        InteractionSessionDescriptor,
    target: &reovim_kernel_interface::buffer::BufferStateStamp,
    projection: &ProjectionStamp,
    coordinate: reovim_kernel_interface::semantic::stamp::CoordinateSpaceStamp,
) -> Option<RepresentationUpdateStampError> {
    RepresentationUpdateStamp::new(
        session,
        &fixture.attachment,
        reovim_kernel_interface::interaction::identity::
            RepresentationUpdateIdentity::new(9_003),
        RepresentationUpdateCursor::new(9),
        target,
        projection,
        &coordinate,
    )
    .err()
}

fn delivery_binding_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(delta) = support::delta_update(fixture) else { return false };
    let stamp = delta.stamp().identity();
    let request = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        RepresentationUpdateCursor::new(2),
    );
    let Some(request) = request.ok() else { return false };
    let mut candidate = Some(delta);
    let result =
        RepresentationUpdateDelivery::try_bind(&request, &mut candidate);
    let rejected = match result {
        Err(error) => {
            error == RepresentationUpdateDeliveryError::CursorMismatch
        }
        Ok(_) => false,
    };
    let retained = candidate
        .as_ref()
        .is_some_and(|update| update.stamp().identity().same_update(stamp));
    rejected && retained && request.expected().value() == 2
}

fn update_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(initial) = support::initial_update(fixture) else { return false };
    let Some(delta) = support::delta_update(fixture) else { return false };
    let Some(invalidation) = invalidation_update(fixture) else {
        return false;
    };
    let Some(gap) = support::gap_update(fixture) else { return false };
    let Some(resync) = support::resync_update(fixture) else { return false };
    let Some(target) = crate::buffer::state(1, 0, 0) else { return false };
    let resolved_target =
        initial.stamp().resolve_target(&fixture.session, &fixture.attachment);
    let resolved_projection = initial
        .stamp()
        .resolve_projection(&fixture.session, &fixture.attachment);
    let gap_lineage = RepresentationUpdateLineage::Gap {
        expected: ProjectionGeneration::new(4),
        available: ProjectionGeneration::new(6),
        cause: reovim_kernel_interface::interaction::update::
            RepresentationGapCause::RetentionEvicted,
        resynchronization:
            reovim_kernel_interface::interaction::identity::
                RepresentationResynchronizationIdentity::new(1_303),
    };
    initial.validate(&fixture.session, &fixture.attachment).is_ok()
        && delta.validate(&fixture.session, &fixture.attachment).is_ok()
        && invalidation.validate(&fixture.session, &fixture.attachment).is_ok()
        && gap.validate(&fixture.session, &fixture.attachment).is_ok()
        && resync.validate(&fixture.session, &fixture.attachment).is_ok()
        && initial.carrier().is_some()
        && invalidation.carrier().is_none()
        && gap.carrier().is_none()
        && gap.lineage() == gap_lineage
        && resolved_target.is_ok_and(|value| value.same_state(target))
        && resolved_projection.is_ok_and(|value| {
            value.same_state(support::projection(fixture, 3))
        })
}

fn invalidation_update(
    fixture: &support::InteractionFixture,
) -> Option<RepresentationUpdate> {
    let target = crate::buffer::state(1, 1, 1)?;
    let stamp = support::update_stamp(
        fixture,
        2_900,
        RepresentationUpdateCursor::new(4),
        &target,
        &support::projection(fixture, 4),
    )?;
    RepresentationUpdate::invalidation(
        &fixture.session,
        &fixture.attachment,
        stamp,
        ProjectionGeneration::new(4),
    )
    .ok()
}

fn delivery_cases(fixture: &support::InteractionFixture) -> bool {
    let request = RepresentationUpdateRequest::new(
        &fixture.session,
        &fixture.attachment,
        RepresentationUpdateCursor::new(1),
    );
    let Some(delta) = support::delta_update(fixture) else { return false };
    let stamp = delta.stamp().identity();
    let acknowledgement = RepresentationUpdateAcknowledgement::new(
        &fixture.session,
        &fixture.attachment,
        stamp,
        RepresentationUpdateApplication::Installed,
    );
    let Some(request) = request.ok() else { return false };
    let mut candidate = Some(delta);
    let delivered =
        RepresentationUpdateDelivery::try_bind(&request, &mut candidate);
    let delivered_ok = delivered.is_ok_and(|delivery| {
        delivery.status() == RepresentationUpdateDeliveryStatus::Delivered
            && candidate.is_none()
    });
    acknowledgement.is_ok()
        && delivered_ok
        && RepresentationUpdateDelivery::unavailable(
            RepresentationUpdateUnavailable::NoCredit,
        )
        .status()
            == RepresentationUpdateDeliveryStatus::Unavailable(
                RepresentationUpdateUnavailable::NoCredit,
            )
        && acknowledgement_result_code(
            RepresentationUpdateAcknowledgementResult::Accepted,
        ) == 0
}

fn resynchronization_cases(fixture: &support::InteractionFixture) -> bool {
    let Some(gap) = support::gap_update(fixture) else { return false };
    let request = RepresentationResynchronizationRequest::new(
        &fixture.session,
        &fixture.attachment,
        &gap,
    );
    let Some(request) = request.ok() else { return false };
    let rejected_request = RepresentationResynchronizationRequest::new(
        &fixture.session,
        &fixture.attachment,
        &gap,
    );
    let Some(rejected_request) = rejected_request.ok() else {
        return false;
    };
    let admitted = RepresentationResynchronizationAdmission::admitted(&request);
    let rejected = RepresentationResynchronizationAdmission::NotAdmitted {
        cause: RepresentationResynchronizationFailure::Pressure,
        request: rejected_request,
    };
    let admitted_ok = match admitted {
        RepresentationResynchronizationAdmission::Admitted(value) => {
            value.resynchronization().value() == 1_303
        }
        RepresentationResynchronizationAdmission::NotAdmitted { .. } => false,
    };
    let rejected_ok = match rejected {
        RepresentationResynchronizationAdmission::NotAdmitted {
            cause,
            request,
        } => {
            cause == RepresentationResynchronizationFailure::Pressure
                && request.resynchronization().value() == 1_303
        }
        RepresentationResynchronizationAdmission::Admitted(_) => false,
    };
    admitted_ok && rejected_ok
}

fn binding_fault(fixture: &support::InteractionFixture) -> bool {
    let Some(target) = crate::buffer::state(1, 1, 1) else { return false };
    let Some(stamp) = support::update_stamp(
        fixture,
        3_000,
        RepresentationUpdateCursor::new(4),
        &target,
        &support::projection(fixture, 4),
    ) else {
        return false;
    };
    let lineage = RepresentationUpdateLineage::Resynchronization {
        result: ProjectionGeneration::new(6),
        resynchronization:
            reovim_kernel_interface::interaction::identity::
                RepresentationResynchronizationIdentity::new(3_001),
    };
    let preparation = fixture.session.prepare_update(
        &fixture.attachment,
        stamp,
        lineage,
        RepresentationUpdatePayloadClaim::new(
            fixture.session.update_contract().selected_stamp(),
            1,
            0,
            4,
        ),
    );
    let Some(preparation) = preparation.ok() else { return false };
    match preparation.bind((&[1_u8, 2, 3][..]).into()) {
        RepresentationUpdateBindingOutcome::Rejected(rejection) => {
            let length_mismatch = rejection.cause()
                == RepresentationUpdateError::LengthMismatch {
                    prepared: 4,
                    actual: 3,
                };
            length_mismatch && rejection.bytes() == [1, 2, 3]
        }
        RepresentationUpdateBindingOutcome::Bound(_) => false,
    }
}

fn lineage_faults(fixture: &support::InteractionFixture) -> bool {
    invalid_delta(fixture) && invalid_gap(fixture)
}

fn invalid_delta(fixture: &support::InteractionFixture) -> bool {
    let Some(target) = crate::buffer::state(1, 1, 1) else { return false };
    let Some(stamp) = support::update_stamp(
        fixture,
        3_100,
        RepresentationUpdateCursor::new(5),
        &target,
        &support::projection(fixture, 4),
    ) else {
        return false;
    };
    let lineage = RepresentationUpdateLineage::Delta {
        predecessor: ProjectionGeneration::new(4),
        result: ProjectionGeneration::new(6),
    };
    let result = fixture.session.prepare_update(
        &fixture.attachment,
        stamp,
        lineage,
        RepresentationUpdatePayloadClaim::new(
            fixture.session.update_contract().selected_stamp(),
            1,
            0,
            4,
        ),
    );
    result.err() == Some(RepresentationUpdateError::InvalidLineage)
}

fn invalid_gap(fixture: &support::InteractionFixture) -> bool {
    let Some(target) = crate::buffer::state(1, 1, 1) else { return false };
    let Some(stamp) = support::update_stamp(
        fixture,
        3_110,
        RepresentationUpdateCursor::new(6),
        &target,
        &support::projection(fixture, 4),
    ) else {
        return false;
    };
    let result = reovim_kernel_interface::interaction::update::
        RepresentationUpdate::gap(
            &fixture.session,
            &fixture.attachment,
            stamp,
            ProjectionGeneration::new(6),
            ProjectionGeneration::new(5),
            reovim_kernel_interface::interaction::update::
                RepresentationGapCause::ProtocolLoss,
            reovim_kernel_interface::interaction::identity::
                RepresentationResynchronizationIdentity::new(3_111),
        );
    result.err() == Some(RepresentationUpdateError::InvalidLineage)
}

fn capability_faults(fixture: &support::InteractionFixture) -> bool {
    let Some(delta) = support::delta_update(fixture) else { return false };
    let Some(without_deltas) = support::session_without_deltas(fixture) else {
        return false;
    };
    let delta_rejected = delta.validate(&without_deltas, &fixture.attachment)
        == Err(RepresentationUpdateError::DeltaUnsupported);
    let Some(gap) = support::gap_update(fixture) else { return false };
    let Some(without_resync) =
        support::session_without_resynchronization(fixture)
    else {
        return false;
    };
    let resync = RepresentationResynchronizationRequest::new(
        &without_resync,
        &fixture.attachment,
        &gap,
    );
    delta_rejected
        && gap.validate(&without_resync, &fixture.attachment).is_ok()
        && resync.err()
            == Some(RepresentationResynchronizationRequestError::Unsupported)
}

const fn bound_cases(fixture: &support::InteractionFixture) -> bool {
    let bounds = fixture.session.bounds().updates();
    let accepted = bounds.admit_retained(7, 500, 4);
    let count_pressure = bounds.admit_retained(8, 100, 4);
    let byte_pressure = bounds.admit_retained(1, 510, 4);
    let credit = bounds.prepare_credit(2);
    let credit_ok = match credit {
        Ok(value) => {
            bounds.admit_credits(2, value).is_ok()
                && bounds.admit_credits(3, value).is_err()
        }
        Err(_) => false,
    };
    let resynchronization = bounds.prepare_resynchronization();
    let resynchronization_ok = match resynchronization {
        Ok(value) => {
            let admitted = bounds.admit_resynchronization(1, 8, 64, value);
            let count_pressure = bounds.admit_resynchronization(2, 0, 0, value);
            let work_pressure = bounds.admit_resynchronization(0, 9, 0, value);
            let byte_pressure =
                bounds.admit_resynchronization(0, 0, 449, value);
            admitted.is_ok()
                && count_pressure.is_err()
                && work_pressure.is_err()
                && byte_pressure.is_err()
        }
        Err(_) => false,
    };
    accepted.is_ok()
        && count_pressure.is_err()
        && byte_pressure.is_err()
        && credit_ok
        && resynchronization_ok
        && bounds.prepare_credit(0).is_err()
        && bounds.require_control_reserve().is_ok()
}

const fn acknowledgement_result_code(
    result: RepresentationUpdateAcknowledgementResult,
) -> u8 {
    match result {
        RepresentationUpdateAcknowledgementResult::Accepted => 0,
        RepresentationUpdateAcknowledgementResult::AlreadyRecorded => 1,
        RepresentationUpdateAcknowledgementResult::UnknownUpdate => 2,
        RepresentationUpdateAcknowledgementResult::WrongCursor => 3,
        RepresentationUpdateAcknowledgementResult::StaleStream => 4,
        RepresentationUpdateAcknowledgementResult::Closing => 5,
        RepresentationUpdateAcknowledgementResult::Closed => 6,
    }
}

fn reservation_faults(fixture: &support::InteractionFixture) -> bool {
    reservation_fault(
        fixture,
        4,
        64,
        InteractionBoundField::ResynchronizationWork,
    ) && reservation_fault(
        fixture,
        8,
        32,
        InteractionBoundField::ResynchronizationBytes,
    )
}

fn reservation_fault(
    fixture: &support::InteractionFixture,
    work: u64,
    bytes: u64,
    field: InteractionBoundField,
) -> bool {
    let bounds = fixture.session.bounds().updates();
    let owner = bounds.charge_owner().value();
    let Some(smaller) =
        support::bounds_with_reservations(owner, 256, work, bytes)
    else {
        return false;
    };
    let Ok(charge) = smaller.updates().prepare_resynchronization() else {
        return false;
    };
    let (expected, actual) = match field {
        InteractionBoundField::ResynchronizationWork => (
            bounds.resynchronization_work().per_object(),
            BoundValue::Count(charge.work()),
        ),
        InteractionBoundField::ResynchronizationBytes => (
            bounds.resynchronization_bytes().per_object(),
            BoundValue::Bytes(charge.bytes()),
        ),
        _ => return false,
    };
    let error = InteractionBoundViolation::ReservationMismatch {
        field,
        expected,
        actual,
    };
    bounds.validate_resynchronization_charge(charge) == Err(error)
        && bounds.admit_resynchronization(0, 0, 0, charge) == Err(error)
}
