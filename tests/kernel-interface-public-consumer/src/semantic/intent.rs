use reovim_kernel_interface::bound::{
    BoundError, BoundUnit, CheckedByteCount, ZeroPolicy,
};
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::semantic::bounds::{
    SemanticBoundField, SemanticBoundViolation, SemanticBoundsError,
    SemanticIntentBounds, SemanticPolicyCallCharge,
};
use reovim_kernel_interface::semantic::intent::{
    SemanticIntentBindingOutcome, SemanticIntentCarrierStamp,
    SemanticIntentError,
};

use super::support;

pub const fn const_cases() -> bool {
    identity_derives()
        && stamp_derives()
        && descriptor_derives()
        && bound_derives()
        && intent_derives()
}

pub fn exercise() -> bool {
    valid_intent()
        && stamp_mismatches()
        && carrier_mismatches()
        && bound_failures()
        && binding_rejection()
}

fn valid_intent() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(left) = support::intent_stamp(&fixture, 1_060, 1_061) else {
        return false;
    };
    let Some(right) = support::intent_stamp(&fixture, 1_060, 1_061) else {
        return false;
    };
    let stamps_match = left.same_request(&right);
    let Some(intent) = support::make_intent(&fixture, support::INTENT_BYTES)
    else {
        return false;
    };
    crate::require_debug(&intent);
    stamps_match
        && intent.stamp().intent().value() == 1_060
        && intent.stamp().request().value() == 1_061
        && intent.stamp().target().same_state(
            crate::buffer::state(1, 0, 0).expect("public fixture state"),
        )
        && intent
            .stamp()
            .plan()
            .owner()
            .same_generation(crate::buffer::owner(1))
        && intent.carrier().type_identity() == support::INTENT_TYPE
        && intent.shape().items().value() == 1
        && intent.shape().depth().value() == 0
        && intent.shape().bytes().value() == 4
        && intent.bytes() == support::INTENT_BYTES
        && intent.into_bytes().as_ref() == support::INTENT_BYTES
}

fn stamp_mismatches() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(other_domain) = support::domain_stamp(1) else {
        return false;
    };
    let Some(wrong_representation) =
        support::representation_stamp(other_domain, 2)
    else {
        return false;
    };
    let coordinate = fixture.representation.coordinate().stamp();
    let Some(wrong_generation) = support::intent_stamp_for(
        1_060,
        1_061,
        wrong_representation,
        coordinate,
    ) else {
        return false;
    };
    let generation_error = wrong_generation.prepare_intent(
        &fixture.representation,
        fixture.representation.intent().selected_stamp(),
        1,
        0,
        4,
    );
    let Some(wrong_coordinate) = support::coordinate_stamp(9_999) else {
        return false;
    };
    let Some(coordinate_stamp) = support::intent_stamp_for(
        1_060,
        1_061,
        fixture.representation.stamp(),
        wrong_coordinate,
    ) else {
        return false;
    };
    let coordinate_error = coordinate_stamp.prepare_intent(
        &fixture.representation,
        fixture.representation.intent().selected_stamp(),
        1,
        0,
        4,
    );
    let generation_code = match generation_error {
        Err(SemanticIntentError::RepresentationGenerationMismatch) => 0,
        _ => 1,
    };
    let coordinate_code = match coordinate_error {
        Err(SemanticIntentError::CoordinateSpaceMismatch) => 0,
        _ => 1,
    };
    generation_code == 0 && coordinate_code == 0
}

fn carrier_mismatches() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(wrong_type_stamp) = support::intent_stamp(&fixture, 1_060, 1_061)
    else {
        return false;
    };
    let wrong_type = wrong_type_stamp.prepare_intent(
        &fixture.representation,
        SemanticIntentCarrierStamp::new(9_999, 1),
        1,
        0,
        4,
    );
    let Some(wrong_version_stamp) =
        support::intent_stamp(&fixture, 1_060, 1_061)
    else {
        return false;
    };
    let wrong_version = wrong_version_stamp.prepare_intent(
        &fixture.representation,
        SemanticIntentCarrierStamp::new(support::INTENT_TYPE, 2),
        1,
        0,
        4,
    );
    let type_code = match wrong_type {
        Err(SemanticIntentError::TypeMismatch) => 0,
        _ => 1,
    };
    let version_code = match wrong_version {
        Err(SemanticIntentError::IncompatibleSchema) => 0,
        _ => 1,
    };
    type_code == 0 && version_code == 0
}

fn bound_failures() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(empty_stamp) = support::intent_stamp(&fixture, 1_060, 1_061)
    else {
        return false;
    };
    let empty = empty_stamp.prepare_intent(
        &fixture.representation,
        fixture.representation.intent().selected_stamp(),
        0,
        0,
        4,
    );
    let Some(large_stamp) = support::intent_stamp(&fixture, 1_060, 1_061)
    else {
        return false;
    };
    let large = large_stamp.prepare_intent(
        &fixture.representation,
        fixture.representation.intent().selected_stamp(),
        1,
        0,
        65,
    );
    zero_items(&empty)
        && excessive_bytes(&large)
        && intent_dimension_bounds(&fixture)
        && bound_construction_errors()
        && aggregate_byte_failures(&fixture)
        && policy_dimension_bounds(&fixture)
}

fn intent_dimension_bounds(fixture: &support::SemanticFixture) -> bool {
    rejected_intent_field(fixture, 5, 0, 4)
        == Some(SemanticBoundField::IntentItems)
        && rejected_intent_field(fixture, 1, 5, 4)
            == Some(SemanticBoundField::IntentDepth)
        && rejected_intent_field(fixture, 1, 0, 65)
            == Some(SemanticBoundField::IntentBytes)
}

fn rejected_intent_field(
    fixture: &support::SemanticFixture,
    items: u64,
    depth: u64,
    bytes: u64,
) -> Option<SemanticBoundField> {
    let stamp = support::intent_stamp(fixture, 1_090, 1_091)?;
    match stamp.prepare_intent(
        &fixture.representation,
        fixture.representation.intent().selected_stamp(),
        items,
        depth,
        bytes,
    ) {
        Err(SemanticIntentError::Bound(violation)) => violation.field(),
        _ => None,
    }
}

fn bound_construction_errors() -> bool {
    let owner = InstanceId::new(support::INTENT_OWNER);
    let Some(count) =
        support::count_bound(support::INTENT_OWNER, 4, ZeroPolicy::Reject)
    else {
        return false;
    };
    let Some(optional) =
        support::count_bound(support::INTENT_OWNER, 4, ZeroPolicy::Allow)
    else {
        return false;
    };
    let Some(bytes) =
        support::byte_bound(support::INTENT_OWNER, 64, ZeroPolicy::Reject)
    else {
        return false;
    };
    let Some(other) = support::count_bound(9_999, 4, ZeroPolicy::Reject) else {
        return false;
    };
    let wrong_unit = SemanticIntentBounds::new(owner, bytes, optional, bytes);
    let wrong_zero =
        SemanticIntentBounds::new(owner, optional, optional, bytes);
    let wrong_owner = SemanticIntentBounds::new(owner, other, optional, bytes);
    construction_error_code(&wrong_unit) == 1
        && construction_error_code(&wrong_zero) == 2
        && construction_error_code(&wrong_owner) == 3
        && count.per_object().value() == 4
}

const fn construction_error_code(
    result: &Result<SemanticIntentBounds, SemanticBoundsError>,
) -> u8 {
    match result {
        Err(SemanticBoundsError::WrongUnit {
            field: SemanticBoundField::IntentItems,
            ..
        }) => 1,
        Err(SemanticBoundsError::WrongZeroPolicy {
            field: SemanticBoundField::IntentItems,
            ..
        }) => 2,
        Err(SemanticBoundsError::ChargeOwnerMismatch {
            field: SemanticBoundField::IntentItems,
            ..
        }) => 3,
        _ => 0,
    }
}

fn aggregate_byte_failures(fixture: &support::SemanticFixture) -> bool {
    let bounds = fixture.representation.coordinate().intent_bounds();
    let Some(current) = CheckedByteCount::new(60, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let Some(additional) = CheckedByteCount::new(5, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let Some(maximum) =
        CheckedByteCount::new(u64::MAX, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let Some(one) = CheckedByteCount::new(1, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let pressure = bounds.admit_retained_bytes(current, additional);
    let overflow = bounds.admit_retained_bytes(maximum, one);
    aggregate_code(pressure) == 1 && aggregate_code(overflow) == 2
}

const fn aggregate_code(
    result: Result<CheckedByteCount, SemanticBoundViolation>,
) -> u8 {
    match result {
        Err(SemanticBoundViolation::Bound {
            field: SemanticBoundField::IntentBytes,
            cause: BoundError::Exhausted { requested: 65, maximum: 64, .. },
        }) => 1,
        Err(SemanticBoundViolation::Bound {
            field: SemanticBoundField::IntentBytes,
            cause: BoundError::ArithmeticOverflow { .. },
        }) => 2,
        _ => 0,
    }
}

fn policy_dimension_bounds(fixture: &support::SemanticFixture) -> bool {
    let bounds = fixture.domain.validation().bounds();
    call_bound_field(&bounds.prepare(17, 0, 0, 1))
        == Some(SemanticBoundField::PolicyWork)
        && call_bound_field(&bounds.prepare(1, 33, 0, 1))
            == Some(SemanticBoundField::TemporaryBytes)
        && call_bound_field(&bounds.prepare(1, 0, 65, 1))
            == Some(SemanticBoundField::ResultBytes)
        && call_bound_field(&bounds.prepare(1, 0, 0, 2))
            == Some(SemanticBoundField::ConcurrentCalls)
}

const fn call_bound_field(
    result: &Result<SemanticPolicyCallCharge, SemanticBoundViolation>,
) -> Option<SemanticBoundField> {
    match result {
        Err(SemanticBoundViolation::Bound { field, .. }) => Some(*field),
        _ => None,
    }
}

const fn zero_items(
    result: &Result<
        reovim_kernel_interface::semantic::intent::SemanticIntentPreparation,
        SemanticIntentError,
    >,
) -> bool {
    let code = match result {
        Err(SemanticIntentError::Bound(
            reovim_kernel_interface::semantic::bounds::
                SemanticBoundViolation::Bound {
                    field: SemanticBoundField::IntentItems,
                    cause: BoundError::ZeroRejected {
                        unit: BoundUnit::Count,
                    },
                },
        )) => 0,
        _ => 1,
    };
    code == 0
}

const fn excessive_bytes(
    result: &Result<
        reovim_kernel_interface::semantic::intent::SemanticIntentPreparation,
        SemanticIntentError,
    >,
) -> bool {
    let code = match result {
        Err(SemanticIntentError::Bound(
            reovim_kernel_interface::semantic::bounds::
                SemanticBoundViolation::Bound {
                    field: SemanticBoundField::IntentBytes,
                    cause: BoundError::Exhausted {
                        unit: BoundUnit::Bytes,
                        requested: 65,
                        maximum: 64,
                    },
                },
        )) => 0,
        _ => 1,
    };
    code == 0
}

fn binding_rejection() -> bool {
    let Some(fixture) = support::fixture() else { return false };
    let Some(stamp) = support::intent_stamp(&fixture, 1_080, 1_081) else {
        return false;
    };
    let Some(preparation) = stamp
        .prepare_intent(
            &fixture.representation,
            fixture.representation.intent().selected_stamp(),
            1,
            0,
            7,
        )
        .ok()
    else {
        return false;
    };
    let outcome =
        preparation.bind(&fixture.representation, b"secret".as_slice().into());
    let SemanticIntentBindingOutcome::Rejected(rejection) = outcome else {
        return false;
    };
    crate::require_debug(&rejection);
    let visible = rejection.cause()
        == SemanticIntentError::LengthMismatch { prepared: 7, actual: 6 }
        && rejection.preparation().stamp().intent().value() == 1_080
        && rejection.bytes() == b"secret";
    let (preparation, bytes) = rejection.into_parts();
    visible
        && preparation.stamp().request().value() == 1_081
        && bytes.as_ref() == b"secret"
}

const fn identity_derives() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::DomainIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::RepresentationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::ProjectionIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::ProjectionGeneration,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::CoordinateSpaceIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::SemanticRequestIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::identity::SemanticIntentIdentity,
    >()
}

const fn stamp_derives() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::stamp::DomainPolicyStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::stamp::RepresentationPolicyStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::stamp::ProjectionStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::stamp::CoordinateSpaceStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::stamp::SemanticCallerStamp,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::semantic::stamp::SemanticIntentStamp,
    >()
}

const fn descriptor_derives() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::SemanticFidelity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::SemanticEditability,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticCorrespondence,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            RepresentationBehavior,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyProgress,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyConcurrency,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyExecutionContext,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyAffinity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyOutwardCalls,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyInterruptionObservation,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticDescriptorError,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticPolicyCallDescriptor,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticCoordinateSpaceDescriptor,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            DomainPolicyDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            SemanticBufferPlanContract,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::semantic::descriptor::
            RepresentationPolicyDescriptor,
    >()
}

const fn bound_derives() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticBoundField,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticBoundsError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticBoundViolation,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticIntentShape,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticIntentBounds,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticPolicyCallCharge,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::semantic::bounds::SemanticPolicyCallBounds,
    >()
}

const fn intent_derives() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::semantic::intent::
            SemanticIntentCarrierStamp,
    >()
        && crate::clone_debug_eq_witness::<
            ::reovim_kernel_interface::semantic::intent::
                SemanticIntentContract,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::semantic::intent::SemanticIntentError,
        >()
        && crate::send_sync_witness::<
            ::reovim_kernel_interface::semantic::intent::
                SemanticIntentPreparation,
        >()
        && crate::send_sync_witness::<
            ::reovim_kernel_interface::semantic::intent::
                SemanticIntentBindingOutcome,
        >()
        && crate::send_sync_witness::<
            ::reovim_kernel_interface::semantic::intent::SemanticIntent,
        >()
        && crate::send_sync_witness::<
            ::reovim_kernel_interface::semantic::intent::
                SemanticIntentRejection,
        >()
}
