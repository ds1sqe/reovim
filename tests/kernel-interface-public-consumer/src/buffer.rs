use reovim_kernel_interface::bound::{
    BoundDescriptor, BoundError, BoundUnit, BoundValue, CheckedByteCount,
    CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::buffer::bounds::BufferPlanBounds;
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferHistoryStamp, BufferIdentity,
    BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::identity::InstanceId;
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};

pub mod carrier;
mod plan;

const CHARGE_OWNER: u128 = 700;

pub const fn const_cases() -> bool {
    BufferIdentity::new(701).value() == 701
        && BufferVersion::INITIAL.value() == 0
        && BufferHistoryNodeIdentity::new(0xac).value() == 0xac
        && carrier::const_cases()
        && plan::const_cases()
}

pub fn exercise() -> bool {
    let Some(current) = state(1, 0, 0) else { return false };
    let Some(same) = state(1, 0, 0) else { return false };
    let Some(replaced) = state(2, 0, 0) else { return false };
    let Some(advanced) = state(1, 1, 1) else { return false };
    current.source().same_snapshot(same.source())
        && current.buffer().buffer().value() == 701
        && current.buffer().lifecycle().instance() == 702
        && current.version() == BufferVersion::INITIAL
        && current.history().lifecycle().instance() == 704
        && current.history().node() == BufferHistoryNodeIdentity::new(0)
        && current.same_state(same)
        && !current.same_state(replaced)
        && !current.same_state(advanced)
        && current.buffer().same_incarnation(replaced.buffer())
        && !current.buffer().same_generation(replaced.buffer())
        && state_derives()
        && bound_cases()
        && carrier::exercise()
        && plan::exercise()
}

pub fn descriptor() -> Option<BufferPlanDescriptor> {
    let carriers = carrier::contracts()?;
    let bounds = bounds()?;
    Some(BufferPlanDescriptor::new(
        crate::resource::selection(800)?,
        owner(1),
        &carriers,
        &bounds,
    ))
}

pub fn state(
    generation: u64,
    version: u64,
    history_node: u128,
) -> Option<BufferStateStamp> {
    let source = crate::resource::snapshot(1, 14)?;
    let buffer = BufferIncarnationStamp::new(
        BufferIdentity::new(701),
        LifecycleIdentity::new(702, 703, Generation::new(generation)),
    );
    let history = BufferHistoryStamp::new(
        LifecycleIdentity::new(704, 705, Generation::new(generation)),
        BufferHistoryNodeIdentity::new(history_node),
    );
    Some(BufferStateStamp::new(
        source,
        buffer,
        BufferVersion::new(version),
        history,
    ))
}

pub const fn owner(generation: u64) -> LifecycleIdentity {
    LifecycleIdentity::new(706, 707, Generation::new(generation))
}

pub fn bounds() -> Option<BufferPlanBounds> {
    BufferPlanBounds::new(
        InstanceId::new(CHARGE_OWNER),
        count_bound(4, 8, ZeroPolicy::Reject)?,
        count_bound(4, 8, ZeroPolicy::Allow)?,
        byte_bound(8, 32, ZeroPolicy::Reject)?,
        byte_bound(16, 64, ZeroPolicy::Reject)?,
        byte_bound(8, 32, ZeroPolicy::Allow)?,
        byte_bound(20, 128, ZeroPolicy::Reject)?,
    )
    .ok()
}

const fn state_derives() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::buffer::BufferIdentity>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::BufferVersion,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::BufferIncarnationStamp,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::BufferHistoryNodeIdentity,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::BufferHistoryStamp,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::BufferStateStamp,
        >()
}

fn bound_cases() -> bool {
    let Some(bounds) = bounds() else { return false };
    let Some(origin) = CheckedByteCount::new(6, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let Some(operations) = CheckedByteCount::new(5, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let Some(empty) = CheckedByteCount::new(0, ZeroPolicy::Allow).ok() else {
        return false;
    };
    let Some(current) = CheckedByteCount::new(11, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    bounds.operations().unit() == BoundUnit::Count
        && bounds.preconditions().unit() == BoundUnit::Count
        && bounds.origin_bytes().unit() == BoundUnit::Bytes
        && bounds.operation_bytes().unit() == BoundUnit::Bytes
        && bounds.precondition_bytes().unit() == BoundUnit::Bytes
        && bounds.owned_bytes().unit() == BoundUnit::Bytes
        && bounds.charge_owner().value() == CHARGE_OWNER
        && bounds.admit_plan_bytes(origin, operations, empty).ok()
            == Some(current)
        && bounds.admit_retained_bytes(current, current).is_ok()
        && bound_error_cases()
        && bound_pressure_cases(&bounds)
        && crate::clone_debug_eq_witness::<
            ::reovim_kernel_interface::buffer::bounds::BufferPlanBounds,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::bounds::BufferPlanBoundField,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::buffer::bounds::BufferPlanBoundsError,
        >()
}

fn bound_pressure_cases(bounds: &BufferPlanBounds) -> bool {
    let Some(maximum) =
        CheckedByteCount::new(u64::MAX, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let Some(one) = CheckedByteCount::new(1, ZeroPolicy::Reject).ok() else {
        return false;
    };
    let Some(current) = CheckedByteCount::new(120, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let Some(additional) = CheckedByteCount::new(9, ZeroPolicy::Reject).ok()
    else {
        return false;
    };
    let overflow = bounds.admit_plan_bytes(maximum, one, one);
    let pressure = bounds.admit_retained_bytes(current, additional);
    let overflow_matches = match overflow {
        Err(BoundError::ArithmeticOverflow {
            unit: BoundUnit::Bytes,
            left,
            right,
        }) => left == u64::MAX && right == 1,
        _ => false,
    };
    let pressure_matches = match pressure {
        Err(BoundError::Exhausted {
            unit: BoundUnit::Bytes,
            requested,
            maximum,
        }) => requested == 129 && maximum == 128,
        _ => false,
    };
    overflow_matches && pressure_matches
}

fn bound_error_cases() -> bool {
    use reovim_kernel_interface::buffer::bounds::{
        BufferPlanBoundField, BufferPlanBoundsError,
    };

    let Some(count) = count_bound(4, 8, ZeroPolicy::Reject) else {
        return false;
    };
    let Some(optional) = count_bound(4, 8, ZeroPolicy::Allow) else {
        return false;
    };
    let Some(bytes) = byte_bound(8, 32, ZeroPolicy::Reject) else {
        return false;
    };
    let Some(optional_bytes) = byte_bound(8, 32, ZeroPolicy::Allow) else {
        return false;
    };
    let wrong_unit = BufferPlanBounds::new(
        InstanceId::new(CHARGE_OWNER),
        bytes,
        optional,
        bytes,
        bytes,
        optional_bytes,
        bytes,
    );
    let wrong_zero = BufferPlanBounds::new(
        InstanceId::new(CHARGE_OWNER),
        count,
        optional,
        optional_bytes,
        bytes,
        optional_bytes,
        bytes,
    );
    let Some(other_owner) =
        count_bound_for_owner(4, 8, ZeroPolicy::Reject, InstanceId::new(799))
    else {
        return false;
    };
    let wrong_owner = BufferPlanBounds::new(
        InstanceId::new(CHARGE_OWNER),
        other_owner,
        optional,
        bytes,
        bytes,
        optional_bytes,
        bytes,
    );
    let unit_code = match wrong_unit {
        Err(BufferPlanBoundsError::WrongUnit {
            field: BufferPlanBoundField::Operations,
            expected: BoundUnit::Count,
            actual: BoundUnit::Bytes,
        }) => 0,
        _ => 1,
    };
    let zero_code = match wrong_zero {
        Err(BufferPlanBoundsError::WrongZeroPolicy {
            field: BufferPlanBoundField::OriginBytes,
            expected: ZeroPolicy::Reject,
            actual: ZeroPolicy::Allow,
        }) => 0,
        _ => 1,
    };
    let owner_matches = match wrong_owner {
        Err(BufferPlanBoundsError::ChargeOwnerMismatch {
            field: BufferPlanBoundField::Operations,
            expected,
            actual,
        }) => expected.value() == CHARGE_OWNER && actual.value() == 799,
        _ => false,
    };
    unit_code == 0
        && zero_code == 0
        && owner_matches
        && crate::error_traits(&BufferPlanBoundsError::WrongUnit {
            field: BufferPlanBoundField::Operations,
            expected: BoundUnit::Count,
            actual: BoundUnit::Bytes,
        })
}

fn count_bound(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
) -> Option<BoundDescriptor> {
    count_bound_for_owner(
        per_object,
        aggregate,
        zero,
        InstanceId::new(CHARGE_OWNER),
    )
}

fn count_bound_for_owner(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
    owner: InstanceId,
) -> Option<BoundDescriptor> {
    BoundDescriptor::new(
        BoundValue::Count(CheckedCount::new(per_object, zero).ok()?),
        BoundValue::Count(CheckedCount::new(aggregate, zero).ok()?),
        zero,
        owner,
    )
    .ok()
}

fn byte_bound(
    per_object: u64,
    aggregate: u64,
    zero: ZeroPolicy,
) -> Option<BoundDescriptor> {
    BoundDescriptor::new(
        BoundValue::Bytes(CheckedByteCount::new(per_object, zero).ok()?),
        BoundValue::Bytes(CheckedByteCount::new(aggregate, zero).ok()?),
        zero,
        InstanceId::new(CHARGE_OWNER),
    )
    .ok()
}
