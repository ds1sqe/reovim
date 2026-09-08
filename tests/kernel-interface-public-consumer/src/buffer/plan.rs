use reovim_kernel_interface::bound::BoundError;
use reovim_kernel_interface::buffer::carrier::{
    BufferCarrierRole, BufferPlanCarrier,
};
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferDeadlineIdentity, BufferHistoryTransitionIdentity,
    BufferPlanBindingOutcome, BufferPlanError, BufferPlanStamp,
    BufferTransactionIdentity, BufferTransactionPlan,
};
use reovim_kernel_interface::operation::{
    CallbackClass, CompletionClass, DeliveryClass, EffectClass,
    InvocationClass, ReentrancyClass,
};

pub const fn const_cases() -> bool {
    BufferTransactionIdentity::new(821).value() == 821
        && BufferHistoryTransitionIdentity::new(822).value() == 822
        && BufferCancellationIdentity::new(823).value() == 823
        && BufferCorrelationIdentity::new(824).value() == 824
        && BufferDeadlineIdentity::new(825).value() == 825
        && derive_cases()
}

pub fn exercise() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    descriptor.contract().identity() == 800
        && descriptor.owner().same_generation(super::owner(1))
        && descriptor.carriers().origin().schema().identity() == 811
        && descriptor.bounds().operations().per_object().value() == 4
        && operation_shape(descriptor.transition_operation())
        && accepted_plan()
        && wrong_owner_rejection()
        && wrong_role_rejection()
        && total_bound_rejection()
}

fn accepted_plan() -> bool {
    let Some(plan) = produce() else { return false };
    let stamp = plan.stamp();
    let target = plan.target();
    let fields = stamp.owner().same_generation(super::owner(1))
        && stamp.transaction().value() == 821
        && stamp.history_transition().value() == 822
        && stamp.cancellation().value() == 823
        && stamp.correlation().value() == 824
        && stamp.deadline()
            == BufferDeadline::Governed(BufferDeadlineIdentity::new(825))
        && target.buffer().buffer().value() == 701
        && target.version().value() == 0
        && target.history().node().value() == 0
        && plan.origin().bytes() == b"secret"
        && plan.operations().bytes() == b"hello"
        && plan.preconditions().bytes().is_empty()
        && plan.owned_bytes().value() == 11;
    let (origin, operations, preconditions) = plan.into_carriers();
    fields
        && origin.into_bytes().as_ref() == b"secret"
        && operations.into_bytes().as_ref() == b"hello"
        && preconditions.into_bytes().is_empty()
}

pub fn produce() -> Option<BufferTransactionPlan> {
    let descriptor = super::descriptor()?;
    let (origin, operations, preconditions) = normal_carriers(&descriptor)?;
    let outcome = descriptor.bind_plan(
        stamp(super::owner(1)),
        super::state(1, 0, 0)?,
        origin,
        operations,
        preconditions,
    );
    match outcome {
        BufferPlanBindingOutcome::Accepted(plan) => Some(plan),
        BufferPlanBindingOutcome::Rejected(_) => None,
    }
}

fn wrong_owner_rejection() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    let Some((origin, operations, preconditions)) =
        normal_carriers(&descriptor)
    else {
        return false;
    };
    let Some(target) = super::state(1, 0, 0) else { return false };
    let rejected_stamp = stamp(super::owner(2));
    let outcome = descriptor.bind_plan(
        rejected_stamp,
        target,
        origin,
        operations,
        preconditions,
    );
    let BufferPlanBindingOutcome::Rejected(rejection) = outcome else {
        return false;
    };
    let visible = rejection.cause() == BufferPlanError::OwnerGenerationMismatch
        && rejection.stamp() == rejected_stamp
        && rejection.target() == target
        && rejection.origin().bytes() == b"secret"
        && rejection.operations().bytes() == b"hello"
        && rejection.preconditions().bytes().is_empty();
    let (origin, operations, preconditions) = rejection.into_carriers();
    visible
        && origin.into_bytes().as_ref() == b"secret"
        && operations.into_bytes().as_ref() == b"hello"
        && preconditions.into_bytes().is_empty()
}

fn wrong_role_rejection() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    let Some((origin, operations, preconditions)) =
        normal_carriers(&descriptor)
    else {
        return false;
    };
    let Some(target) = super::state(1, 0, 0) else { return false };
    let outcome = descriptor.bind_plan(
        stamp(super::owner(1)),
        target,
        operations,
        origin,
        preconditions,
    );
    let BufferPlanBindingOutcome::Rejected(rejection) = outcome else {
        return false;
    };
    let cause_code = match rejection.cause() {
        BufferPlanError::Carrier(
            reovim_kernel_interface::buffer::carrier::BufferCarrierError::
                RoleMismatch {
                    expected: BufferCarrierRole::Origin,
                    actual: BufferCarrierRole::Operations,
                }
        ) => 0,
        _ => 1,
    };
    let (first, second, third) = rejection.into_carriers();
    cause_code == 0
        && first.into_bytes().as_ref() == b"hello"
        && second.into_bytes().as_ref() == b"secret"
        && third.into_bytes().is_empty()
}

fn total_bound_rejection() -> bool {
    let Some(descriptor) = super::descriptor() else { return false };
    let Some(origin) = super::carrier::make_carrier(
        &descriptor,
        BufferCarrierRole::Origin,
        1,
        b"12345678",
    ) else {
        return false;
    };
    let Some(operations) = super::carrier::make_carrier(
        &descriptor,
        BufferCarrierRole::Operations,
        1,
        b"1234567890123456",
    ) else {
        return false;
    };
    let Some(preconditions) = super::carrier::make_carrier(
        &descriptor,
        BufferCarrierRole::Preconditions,
        0,
        b"",
    ) else {
        return false;
    };
    let Some(target) = super::state(1, 0, 0) else { return false };
    let outcome = descriptor.bind_plan(
        stamp(super::owner(1)),
        target,
        origin,
        operations,
        preconditions,
    );
    let BufferPlanBindingOutcome::Rejected(rejection) = outcome else {
        return false;
    };
    let cause_code = match rejection.cause() {
        BufferPlanError::Bound(BoundError::Exhausted {
            requested: 24,
            maximum: 20,
            ..
        }) => 0,
        _ => 1,
    };
    let (origin, operations, preconditions) = rejection.into_carriers();
    cause_code == 0
        && origin.into_bytes().as_ref() == b"12345678"
        && operations.into_bytes().as_ref() == b"1234567890123456"
        && preconditions.into_bytes().is_empty()
}

fn normal_carriers(
    descriptor: &reovim_kernel_interface::buffer::plan::BufferPlanDescriptor,
) -> Option<(BufferPlanCarrier, BufferPlanCarrier, BufferPlanCarrier)> {
    Some((
        super::carrier::make_carrier(
            descriptor,
            BufferCarrierRole::Origin,
            1,
            b"secret",
        )?,
        super::carrier::make_carrier(
            descriptor,
            BufferCarrierRole::Operations,
            1,
            b"hello",
        )?,
        super::carrier::make_carrier(
            descriptor,
            BufferCarrierRole::Preconditions,
            0,
            b"",
        )?,
    ))
}

const fn stamp(
    owner: reovim_kernel_interface::lifecycle::LifecycleIdentity,
) -> BufferPlanStamp {
    BufferPlanStamp::new(
        owner,
        BufferTransactionIdentity::new(821),
        BufferHistoryTransitionIdentity::new(822),
        BufferCancellationIdentity::new(823),
        BufferCorrelationIdentity::new(824),
        BufferDeadline::Governed(BufferDeadlineIdentity::new(825)),
    )
}

fn operation_shape(
    operation: reovim_kernel_interface::operation::OperationDescriptor,
) -> bool {
    operation.invocation() == InvocationClass::NonBlockingInvocation
        && operation.completion() == CompletionClass::InlineTerminal
        && operation.delivery() == DeliveryClass::Single
        && operation.effect() == EffectClass::LocalOnly
        && operation.callback() == CallbackClass::NoUserCallback
        && operation.reentrancy() == ReentrancyClass::NonReentrant
}

const fn derive_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferTransactionIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::
            BufferHistoryTransitionIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferCancellationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferCorrelationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferDeadlineIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferDeadline,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferPlanStamp,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferPlanDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferPlanError,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferPlanBindingOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferPlanRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::buffer::plan::BufferTransactionPlan,
    >() && crate::error_traits(&BufferPlanError::OwnerGenerationMismatch)
}
