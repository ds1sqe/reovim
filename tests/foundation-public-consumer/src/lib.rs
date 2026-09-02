#![forbid(unsafe_code)]
//! Compile-only external consumer of every admitted Foundation primitive.

use core::fmt::Debug;

const _: [(); 1] = [(); foundation_contract_cases() as usize];

const fn foundation_contract_cases() -> bool {
    contract_const_cases() && bound_const_cases() && carrier_const_cases()
}

const fn contract_const_cases() -> bool {
    let minimum = reovim_foundation::contract::ContractVersion::new(2);
    let maximum = reovim_foundation::contract::ContractVersion::new(4);
    let Ok(range) = reovim_foundation::contract::CompatibleVersionRange::new(
        minimum, maximum,
    ) else {
        return false;
    };
    let inverted = reovim_foundation::contract::CompatibleVersionRange::new(
        maximum, minimum,
    );

    range.contains(minimum)
        && range.contains(maximum)
        && !range.contains(reovim_foundation::contract::ContractVersion::new(1))
        && version_range_inverted(inverted)
}

const fn version_range_inverted(
    result: Result<
        reovim_foundation::contract::CompatibleVersionRange,
        reovim_foundation::contract::VersionRangeError,
    >,
) -> bool {
    match result {
        Err(error) => {
            error.minimum().value() == 4 && error.maximum().value() == 2
        }
        Ok(_) => false,
    }
}

const fn bound_const_cases() -> bool {
    let policy = reovim_foundation::bound::ZeroPolicy::Reject;
    let Ok(per_object) = reovim_foundation::bound::CheckedCount::new(4, policy)
    else {
        return false;
    };
    let Ok(aggregate) = reovim_foundation::bound::CheckedCount::new(8, policy)
    else {
        return false;
    };
    let descriptor = reovim_foundation::bound::BoundDescriptor::new(
        reovim_foundation::bound::BoundValue::Count(per_object),
        reovim_foundation::bound::BoundValue::Count(aggregate),
        policy,
        reovim_foundation::identity::InstanceId::new(1),
    );
    let Ok(descriptor) = descriptor else { return false };

    zero_const_case()
        && descriptor_zero_const_case(descriptor)
        && overflow_const_case()
        && exhaustion_const_case(descriptor)
}

const fn descriptor_zero_const_case(
    descriptor: reovim_foundation::bound::BoundDescriptor,
) -> bool {
    let zero = reovim_foundation::bound::CheckedCount::new(
        0,
        reovim_foundation::bound::ZeroPolicy::Allow,
    );
    let Ok(zero) = zero else { return false };
    let result = descriptor
        .admit_per_object(reovim_foundation::bound::BoundValue::Count(zero));
    zero_admission_code(result) == 1
}

const fn zero_admission_code(
    result: Result<(), reovim_foundation::bound::BoundError>,
) -> u8 {
    match result {
        Err(reovim_foundation::bound::BoundError::ZeroRejected {
            unit: reovim_foundation::bound::BoundUnit::Count,
        }) => 1,
        _ => 0,
    }
}

const fn zero_const_case() -> bool {
    let result = reovim_foundation::bound::CheckedCount::new(
        0,
        reovim_foundation::bound::ZeroPolicy::Reject,
    );
    zero_result_code(result) == 1
}

const fn zero_result_code(
    result: Result<
        reovim_foundation::bound::CheckedCount,
        reovim_foundation::bound::BoundError,
    >,
) -> u8 {
    match result {
        Err(reovim_foundation::bound::BoundError::ZeroRejected {
            unit: reovim_foundation::bound::BoundUnit::Count,
        }) => 1,
        _ => 0,
    }
}

const fn overflow_const_case() -> bool {
    let policy = reovim_foundation::bound::ZeroPolicy::Reject;
    let Ok(maximum) =
        reovim_foundation::bound::CheckedCount::new(u64::MAX, policy)
    else {
        return false;
    };
    let Ok(one) = reovim_foundation::bound::CheckedCount::new(1, policy) else {
        return false;
    };
    overflow_result_code(maximum.checked_add(one)) == 1
}

const fn overflow_result_code(
    result: Result<
        reovim_foundation::bound::CheckedCount,
        reovim_foundation::bound::BoundError,
    >,
) -> u8 {
    match result {
        Err(reovim_foundation::bound::BoundError::ArithmeticOverflow {
            unit: reovim_foundation::bound::BoundUnit::Count,
            ..
        }) => 1,
        _ => 0,
    }
}

const fn exhaustion_const_case(
    descriptor: reovim_foundation::bound::BoundDescriptor,
) -> bool {
    let Ok(over) = reovim_foundation::bound::CheckedCount::new(
        5,
        reovim_foundation::bound::ZeroPolicy::Reject,
    ) else {
        return false;
    };
    let result = descriptor
        .admit_per_object(reovim_foundation::bound::BoundValue::Count(over));
    exhaustion_result_code(result) == 1
}

const fn exhaustion_result_code(
    result: Result<(), reovim_foundation::bound::BoundError>,
) -> u8 {
    match result {
        Err(reovim_foundation::bound::BoundError::Exhausted {
            requested: 5,
            maximum: 4,
            ..
        }) => 1,
        _ => 0,
    }
}

const fn carrier_const_cases() -> bool {
    let version = reovim_foundation::contract::ContractVersion::new(1);
    let Ok(versions) = reovim_foundation::contract::CompatibleVersionRange::new(
        version, version,
    ) else {
        return false;
    };
    let memory = reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Owned,
        reovim_foundation::carrier::BorrowModel::Forbidden,
        reovim_foundation::carrier::CopyModel::Copy,
        reovim_foundation::carrier::DropModel::Trivial,
        reovim_foundation::carrier::LifetimeModel::Value,
    );
    let concurrency = reovim_foundation::carrier::ConcurrencySafety::new(
        reovim_foundation::carrier::ThreadTransfer::Transferable,
        reovim_foundation::carrier::SharingModel::Immutable,
    );
    let invalid_invocation = reovim_foundation::carrier::InvocationSafety::new(
        reovim_foundation::carrier::InvocationModel::DataOnly,
        reovim_foundation::carrier::UnwindModel::Contained,
    );
    let valid_invocation = reovim_foundation::carrier::InvocationSafety::new(
        reovim_foundation::carrier::InvocationModel::DataOnly,
        reovim_foundation::carrier::UnwindModel::NotInvocable,
    );
    let result = reovim_foundation::carrier::CarrierSafetyDescriptor::new(
        reovim_foundation::contract::ContractId::new(1),
        versions,
        memory,
        concurrency,
        invalid_invocation,
        reovim_foundation::carrier::TypeMismatch::Reject,
        reovim_foundation::carrier::UnsafeBoundary::None,
    );
    invocation_error_const_case(result)
        && memory_error_const_cases(versions, concurrency, valid_invocation)
}

const fn invocation_error_const_case(
    result: Result<
        reovim_foundation::carrier::CarrierSafetyDescriptor,
        reovim_foundation::carrier::CarrierDescriptorError,
    >,
) -> bool {
    let Err(error) = result else { return false };
    invocation_error_code(error) == 1
}

const fn invocation_error_code(
    error: reovim_foundation::carrier::CarrierDescriptorError,
) -> u8 {
    match (error.invocation(), error.unwind()) {
        (
            Some(reovim_foundation::carrier::InvocationModel::DataOnly),
            Some(reovim_foundation::carrier::UnwindModel::Contained),
        ) => 1,
        _ => 0,
    }
}

const fn memory_error_const_cases(
    versions: reovim_foundation::contract::CompatibleVersionRange,
    concurrency: reovim_foundation::carrier::ConcurrencySafety,
    invocation: reovim_foundation::carrier::InvocationSafety,
) -> bool {
    let borrowed_without_borrow = memory_descriptor(
        borrowed_without_borrow(),
        versions,
        concurrency,
        invocation,
    );
    let borrowed_value =
        memory_descriptor(borrowed_value(), versions, concurrency, invocation);
    let borrowed_release = memory_descriptor(
        borrowed_release(),
        versions,
        concurrency,
        invocation,
    );
    let copy_release =
        memory_descriptor(copy_release(), versions, concurrency, invocation);
    let valid_borrowed =
        memory_descriptor(valid_borrowed(), versions, concurrency, invocation);
    let valid_owned_release = memory_descriptor(
        valid_owned_release(),
        versions,
        concurrency,
        invocation,
    );
    let valid_copy_exclusive = memory_descriptor(
        valid_copy_exclusive(),
        versions,
        concurrency,
        invocation,
    );

    memory_error_matches(borrowed_without_borrow, 1)
        && memory_error_matches(borrowed_value, 2)
        && memory_error_matches(borrowed_release, 3)
        && memory_error_matches(copy_release, 4)
        && valid_borrowed.is_ok()
        && valid_owned_release.is_ok()
        && valid_copy_exclusive.is_ok()
}

const fn memory_descriptor(
    memory: reovim_foundation::carrier::MemorySafety,
    versions: reovim_foundation::contract::CompatibleVersionRange,
    concurrency: reovim_foundation::carrier::ConcurrencySafety,
    invocation: reovim_foundation::carrier::InvocationSafety,
) -> Result<
    reovim_foundation::carrier::CarrierSafetyDescriptor,
    reovim_foundation::carrier::CarrierDescriptorError,
> {
    reovim_foundation::carrier::CarrierSafetyDescriptor::new(
        reovim_foundation::contract::ContractId::new(2),
        versions,
        memory,
        concurrency,
        invocation,
        reovim_foundation::carrier::TypeMismatch::Reject,
        reovim_foundation::carrier::UnsafeBoundary::None,
    )
}

const fn borrowed_without_borrow() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Borrowed,
        reovim_foundation::carrier::BorrowModel::Forbidden,
        reovim_foundation::carrier::CopyModel::MoveOnly,
        reovim_foundation::carrier::DropModel::Trivial,
        reovim_foundation::carrier::LifetimeModel::InstanceBound,
    )
}

const fn borrowed_value() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Borrowed,
        reovim_foundation::carrier::BorrowModel::Shared,
        reovim_foundation::carrier::CopyModel::MoveOnly,
        reovim_foundation::carrier::DropModel::Trivial,
        reovim_foundation::carrier::LifetimeModel::Value,
    )
}

const fn borrowed_release() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Borrowed,
        reovim_foundation::carrier::BorrowModel::Shared,
        reovim_foundation::carrier::CopyModel::MoveOnly,
        reovim_foundation::carrier::DropModel::ReleasesOwnedState,
        reovim_foundation::carrier::LifetimeModel::InstanceBound,
    )
}

const fn copy_release() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Owned,
        reovim_foundation::carrier::BorrowModel::Forbidden,
        reovim_foundation::carrier::CopyModel::Copy,
        reovim_foundation::carrier::DropModel::ReleasesOwnedState,
        reovim_foundation::carrier::LifetimeModel::Value,
    )
}

const fn valid_borrowed() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Borrowed,
        reovim_foundation::carrier::BorrowModel::Shared,
        reovim_foundation::carrier::CopyModel::Copy,
        reovim_foundation::carrier::DropModel::Trivial,
        reovim_foundation::carrier::LifetimeModel::IncarnationBound,
    )
}

const fn valid_owned_release() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Owned,
        reovim_foundation::carrier::BorrowModel::Forbidden,
        reovim_foundation::carrier::CopyModel::MoveOnly,
        reovim_foundation::carrier::DropModel::ReleasesOwnedState,
        reovim_foundation::carrier::LifetimeModel::Value,
    )
}

const fn valid_copy_exclusive() -> reovim_foundation::carrier::MemorySafety {
    reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Owned,
        reovim_foundation::carrier::BorrowModel::Exclusive,
        reovim_foundation::carrier::CopyModel::Copy,
        reovim_foundation::carrier::DropModel::Trivial,
        reovim_foundation::carrier::LifetimeModel::Value,
    )
}

const fn memory_error_matches(
    result: Result<
        reovim_foundation::carrier::CarrierSafetyDescriptor,
        reovim_foundation::carrier::CarrierDescriptorError,
    >,
    expected: u8,
) -> bool {
    let Err(error) = result else { return false };
    let Some(memory) = error.memory() else { return false };
    memory_error_code(memory) == expected && invocation_absent(error)
}

const fn invocation_absent(
    error: reovim_foundation::carrier::CarrierDescriptorError,
) -> bool {
    error.invocation().is_none() && error.unwind().is_none()
}

const fn memory_error_code(
    error: reovim_foundation::carrier::MemoryCoherenceError,
) -> u8 {
    match error {
        reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedWithoutBorrow => 1,
        reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedValueLifetime => 2,
        reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedReleasesOwnedState => 3,
        reovim_foundation::carrier::MemoryCoherenceError::
            CopyDuplicatesReleaseAuthority => 4,
    }
}

/// Exercises the complete admitted Foundation surface through its public API.
#[must_use]
pub fn exercise_public_api() -> bool {
    contract_cases()
        && identity_cases()
        && quantity_cases()
        && bound_cases()
        && carrier_cases()
        && derive_witnesses()
}

fn contract_cases() -> bool {
    let identity = reovim_foundation::contract::ContractId::new(11);
    let minimum = reovim_foundation::contract::ContractVersion::new(2);
    let maximum = reovim_foundation::contract::ContractVersion::new(4);
    let Some(range) = reovim_foundation::contract::CompatibleVersionRange::new(
        minimum, maximum,
    )
    .ok() else {
        return false;
    };
    let inverted = reovim_foundation::contract::CompatibleVersionRange::new(
        maximum, minimum,
    );
    let Err(error) = inverted else { return false };

    identity.value() == 11
        && range.minimum() == minimum
        && range.maximum() == maximum
        && range.contains(minimum)
        && range.contains(maximum)
        && !range.contains(reovim_foundation::contract::ContractVersion::new(1))
        && error.minimum() == maximum
        && error.maximum() == minimum
        && traits(identity)
        && traits(minimum)
        && traits(range)
        && traits(error)
}

fn identity_cases() -> bool {
    let instance = reovim_foundation::identity::InstanceId::new(21);
    let incarnation = reovim_foundation::identity::IncarnationId::new(22);

    instance.value() == 21
        && incarnation.value() == 22
        && traits(instance)
        && traits(incarnation)
}

fn quantity_cases() -> bool {
    let Some(zero) = reovim_foundation::bound::CheckedCount::new(
        0,
        reovim_foundation::bound::ZeroPolicy::Allow,
    )
    .ok() else {
        return false;
    };
    let rejected = reovim_foundation::bound::CheckedCount::new(
        0,
        reovim_foundation::bound::ZeroPolicy::Reject,
    );
    let count_overflow = count_overflow();
    let byte_overflow = byte_overflow();
    let duration_overflow = duration_overflow();

    zero.value() == 0
        && zero_rejected(rejected)
        && overflow_is_count(count_overflow)
        && overflow_is_bytes(byte_overflow)
        && overflow_is_duration(duration_overflow)
}

fn count_overflow() -> Result<
    reovim_foundation::bound::CheckedCount,
    reovim_foundation::bound::BoundError,
> {
    let maximum = reovim_foundation::bound::CheckedCount::new(
        u64::MAX,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )?;
    let one = reovim_foundation::bound::CheckedCount::new(
        1,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )?;
    maximum.checked_add(one)
}

fn byte_overflow() -> Result<
    reovim_foundation::bound::CheckedByteCount,
    reovim_foundation::bound::BoundError,
> {
    let maximum = reovim_foundation::bound::CheckedByteCount::new(
        u64::MAX,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )?;
    let one = reovim_foundation::bound::CheckedByteCount::new(
        1,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )?;
    maximum.checked_add(one)
}

fn duration_overflow() -> Result<
    reovim_foundation::bound::CheckedDuration,
    reovim_foundation::bound::BoundError,
> {
    let maximum = reovim_foundation::bound::CheckedDuration::from_nanoseconds(
        u64::MAX,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )?;
    let one = reovim_foundation::bound::CheckedDuration::from_nanoseconds(
        1,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )?;
    maximum.checked_add(one)
}

fn zero_rejected(
    result: Result<
        reovim_foundation::bound::CheckedCount,
        reovim_foundation::bound::BoundError,
    >,
) -> bool {
    match result {
        Err(reovim_foundation::bound::BoundError::ZeroRejected { unit }) => {
            unit == reovim_foundation::bound::BoundUnit::Count
        }
        _ => false,
    }
}

fn overflow_is_count(
    result: Result<
        reovim_foundation::bound::CheckedCount,
        reovim_foundation::bound::BoundError,
    >,
) -> bool {
    overflow_has_unit(result.err(), reovim_foundation::bound::BoundUnit::Count)
}

fn overflow_is_bytes(
    result: Result<
        reovim_foundation::bound::CheckedByteCount,
        reovim_foundation::bound::BoundError,
    >,
) -> bool {
    overflow_has_unit(result.err(), reovim_foundation::bound::BoundUnit::Bytes)
}

fn overflow_is_duration(
    result: Result<
        reovim_foundation::bound::CheckedDuration,
        reovim_foundation::bound::BoundError,
    >,
) -> bool {
    overflow_has_unit(
        result.err(),
        reovim_foundation::bound::BoundUnit::DurationNanoseconds,
    )
}

fn overflow_has_unit(
    error: Option<reovim_foundation::bound::BoundError>,
    expected: reovim_foundation::bound::BoundUnit,
) -> bool {
    match error {
        Some(
            error @ reovim_foundation::bound::BoundError::ArithmeticOverflow {
                unit,
                ..
            },
        ) => unit == expected && traits(error),
        _ => false,
    }
}

fn bound_cases() -> bool {
    let Some(per_object) = checked_count(4) else { return false };
    let Some(aggregate) = checked_count(8) else { return false };
    let owner = reovim_foundation::identity::InstanceId::new(31);
    let descriptor = reovim_foundation::bound::BoundDescriptor::new(
        reovim_foundation::bound::BoundValue::Count(per_object),
        reovim_foundation::bound::BoundValue::Count(aggregate),
        reovim_foundation::bound::ZeroPolicy::Reject,
        owner,
    );
    let Some(descriptor) = descriptor.ok() else { return false };
    let Some(exact) = checked_count(4) else { return false };
    let Some(over) = checked_count(5) else { return false };
    let exact = reovim_foundation::bound::BoundValue::Count(exact);
    let over = reovim_foundation::bound::BoundValue::Count(over);

    descriptor.unit() == reovim_foundation::bound::BoundUnit::Count
        && descriptor.per_object().value() == 4
        && descriptor.aggregate().value() == 8
        && descriptor.zero_policy()
            == reovim_foundation::bound::ZeroPolicy::Reject
        && descriptor.charge_owner() == owner
        && descriptor.arithmetic_rule()
            == reovim_foundation::bound::ArithmeticRule::Checked
        && descriptor.validation_order()
            == reovim_foundation::bound::ValidationOrder::
                BeforeTraversalAndAllocation
        && descriptor.admit_per_object(exact).is_ok()
        && exhausted(descriptor.admit_per_object(over))
        && aggregate_cases(descriptor)
        && bound_value_cases()
        && bound_variant_traits(descriptor)
}

fn checked_count(value: u64) -> Option<reovim_foundation::bound::CheckedCount> {
    reovim_foundation::bound::CheckedCount::new(
        value,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )
    .ok()
}

const fn exhausted(
    result: Result<(), reovim_foundation::bound::BoundError>,
) -> bool {
    match result {
        Err(reovim_foundation::bound::BoundError::Exhausted {
            requested,
            maximum,
            ..
        }) => requested == 5 && maximum == 4,
        _ => false,
    }
}

fn aggregate_cases(
    descriptor: reovim_foundation::bound::BoundDescriptor,
) -> bool {
    let Some(seven) = checked_count(7) else { return false };
    let Some(one) = checked_count(1) else { return false };
    let Some(two) = checked_count(2) else { return false };
    let seven = reovim_foundation::bound::BoundValue::Count(seven);
    let one = reovim_foundation::bound::BoundValue::Count(one);
    let two = reovim_foundation::bound::BoundValue::Count(two);
    let accepted = descriptor.admit_aggregate(seven, one);
    let exhausted = descriptor.admit_aggregate(seven, two);
    let zero = reovim_foundation::bound::CheckedCount::new(
        0,
        reovim_foundation::bound::ZeroPolicy::Allow,
    );
    let Some(zero) = zero.ok() else { return false };
    let zero = reovim_foundation::bound::BoundValue::Count(zero);

    accepted.ok().map(reovim_foundation::bound::BoundValue::value) == Some(8)
        && aggregate_zero_code(descriptor.admit_aggregate(one, zero)) == 1
        && match exhausted {
            Err(reovim_foundation::bound::BoundError::Exhausted {
                requested,
                maximum,
                ..
            }) => requested == 9 && maximum == 8,
            _ => false,
        }
}

const fn aggregate_zero_code(
    result: Result<
        reovim_foundation::bound::BoundValue,
        reovim_foundation::bound::BoundError,
    >,
) -> u8 {
    match result {
        Err(reovim_foundation::bound::BoundError::ZeroRejected {
            unit: reovim_foundation::bound::BoundUnit::Count,
        }) => 1,
        _ => 0,
    }
}

fn bound_value_cases() -> bool {
    let Some(bytes) = reovim_foundation::bound::CheckedByteCount::new(
        16,
        reovim_foundation::bound::ZeroPolicy::Reject,
    )
    .ok() else {
        return false;
    };
    let Some(duration) =
        reovim_foundation::bound::CheckedDuration::from_nanoseconds(
            25,
            reovim_foundation::bound::ZeroPolicy::Reject,
        )
        .ok()
    else {
        return false;
    };
    let bytes = reovim_foundation::bound::BoundValue::Bytes(bytes);
    let duration = reovim_foundation::bound::BoundValue::Duration(duration);

    bytes.unit() == reovim_foundation::bound::BoundUnit::Bytes
        && duration.unit()
            == reovim_foundation::bound::BoundUnit::DurationNanoseconds
        && bytes.checked_add(duration).is_err()
        && traits(bytes)
        && traits(duration)
}

fn bound_variant_traits(
    descriptor: reovim_foundation::bound::BoundDescriptor,
) -> bool {
    traits(reovim_foundation::bound::ZeroPolicy::Allow)
        && traits(reovim_foundation::bound::ZeroPolicy::Reject)
        && traits(reovim_foundation::bound::BoundUnit::Count)
        && traits(reovim_foundation::bound::BoundUnit::Bytes)
        && traits(
            reovim_foundation::bound::BoundUnit::DurationNanoseconds,
        )
        && traits(reovim_foundation::bound::ArithmeticRule::Checked)
        && traits(
            reovim_foundation::bound::ValidationOrder::
                BeforeTraversalAndAllocation,
        )
        && traits(descriptor)
}

fn carrier_cases() -> bool {
    let minimum = reovim_foundation::contract::ContractVersion::new(1);
    let maximum = reovim_foundation::contract::ContractVersion::new(2);
    let Some(versions) =
        reovim_foundation::contract::CompatibleVersionRange::new(
            minimum, maximum,
        )
        .ok()
    else {
        return false;
    };
    let memory = reovim_foundation::carrier::MemorySafety::new(
        reovim_foundation::carrier::OwnershipModel::Owned,
        reovim_foundation::carrier::BorrowModel::Forbidden,
        reovim_foundation::carrier::CopyModel::Copy,
        reovim_foundation::carrier::DropModel::Trivial,
        reovim_foundation::carrier::LifetimeModel::Value,
    );
    let concurrency = reovim_foundation::carrier::ConcurrencySafety::new(
        reovim_foundation::carrier::ThreadTransfer::Transferable,
        reovim_foundation::carrier::SharingModel::Immutable,
    );
    let invocation = reovim_foundation::carrier::InvocationSafety::new(
        reovim_foundation::carrier::InvocationModel::DataOnly,
        reovim_foundation::carrier::UnwindModel::NotInvocable,
    );
    let descriptor = reovim_foundation::carrier::CarrierSafetyDescriptor::new(
        reovim_foundation::contract::ContractId::new(41),
        versions,
        memory,
        concurrency,
        invocation,
        reovim_foundation::carrier::TypeMismatch::Reject,
        reovim_foundation::carrier::UnsafeBoundary::None,
    );
    let Some(descriptor) = descriptor.ok() else { return false };
    let invalid = reovim_foundation::carrier::CarrierSafetyDescriptor::new(
        reovim_foundation::contract::ContractId::new(41),
        versions,
        memory,
        concurrency,
        reovim_foundation::carrier::InvocationSafety::new(
            reovim_foundation::carrier::InvocationModel::DataOnly,
            reovim_foundation::carrier::UnwindModel::Contained,
        ),
        reovim_foundation::carrier::TypeMismatch::Reject,
        reovim_foundation::carrier::UnsafeBoundary::None,
    );

    carrier_accessors(descriptor, memory, concurrency, invocation)
        && invalid_carrier(invalid)
        && memory_error_const_cases(versions, concurrency, invocation)
        && carrier_variant_traits()
        && traits(memory)
        && traits(concurrency)
        && traits(invocation)
        && traits(descriptor)
}

fn invalid_carrier(
    result: Result<
        reovim_foundation::carrier::CarrierSafetyDescriptor,
        reovim_foundation::carrier::CarrierDescriptorError,
    >,
) -> bool {
    let Err(error) = result else { return false };
    error.memory().is_none()
        && error.invocation()
            == Some(reovim_foundation::carrier::InvocationModel::DataOnly)
        && error.unwind()
            == Some(reovim_foundation::carrier::UnwindModel::Contained)
        && traits(error)
}

fn carrier_accessors(
    descriptor: reovim_foundation::carrier::CarrierSafetyDescriptor,
    memory: reovim_foundation::carrier::MemorySafety,
    concurrency: reovim_foundation::carrier::ConcurrencySafety,
    invocation: reovim_foundation::carrier::InvocationSafety,
) -> bool {
    descriptor.type_id().value() == 41
        && descriptor
            .schema_versions()
            .contains(reovim_foundation::contract::ContractVersion::new(2))
        && descriptor.memory() == memory
        && descriptor.concurrency() == concurrency
        && descriptor.invocation() == invocation
        && descriptor.mismatch()
            == reovim_foundation::carrier::TypeMismatch::Reject
        && descriptor.unsafe_boundary()
            == reovim_foundation::carrier::UnsafeBoundary::None
        && memory.ownership()
            == reovim_foundation::carrier::OwnershipModel::Owned
        && memory.borrowing()
            == reovim_foundation::carrier::BorrowModel::Forbidden
        && memory.copying() == reovim_foundation::carrier::CopyModel::Copy
        && memory.dropping() == reovim_foundation::carrier::DropModel::Trivial
        && memory.lifetime() == reovim_foundation::carrier::LifetimeModel::Value
        && concurrency.thread_transfer()
            == reovim_foundation::carrier::ThreadTransfer::Transferable
        && concurrency.sharing()
            == reovim_foundation::carrier::SharingModel::Immutable
        && invocation.invocation()
            == reovim_foundation::carrier::InvocationModel::DataOnly
        && invocation.unwind()
            == reovim_foundation::carrier::UnwindModel::NotInvocable
}

fn carrier_variant_traits() -> bool {
    ownership_traits()
        && borrow_traits()
        && copy_drop_lifetime_traits()
        && concurrency_traits()
        && invocation_traits()
        && traits(reovim_foundation::carrier::TypeMismatch::Reject)
        && traits(reovim_foundation::carrier::UnsafeBoundary::None)
        && traits(reovim_foundation::carrier::UnsafeBoundary::Reviewed(
            reovim_foundation::contract::ContractId::new(42),
        ))
        && memory_error_variant_traits()
}

fn memory_error_variant_traits() -> bool {
    traits(
        reovim_foundation::carrier::MemoryCoherenceError::BorrowedWithoutBorrow,
    ) && traits(
        reovim_foundation::carrier::MemoryCoherenceError::BorrowedValueLifetime,
    ) && traits(
        reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedReleasesOwnedState,
    ) && traits(
        reovim_foundation::carrier::MemoryCoherenceError::
            CopyDuplicatesReleaseAuthority,
    )
}

fn ownership_traits() -> bool {
    traits(reovim_foundation::carrier::OwnershipModel::Owned)
        && traits(reovim_foundation::carrier::OwnershipModel::Borrowed)
}

fn borrow_traits() -> bool {
    traits(reovim_foundation::carrier::BorrowModel::Forbidden)
        && traits(reovim_foundation::carrier::BorrowModel::Shared)
        && traits(reovim_foundation::carrier::BorrowModel::Exclusive)
}

fn copy_drop_lifetime_traits() -> bool {
    traits(reovim_foundation::carrier::CopyModel::MoveOnly)
        && traits(reovim_foundation::carrier::CopyModel::Copy)
        && traits(reovim_foundation::carrier::DropModel::Trivial)
        && traits(reovim_foundation::carrier::DropModel::ReleasesOwnedState)
        && traits(reovim_foundation::carrier::LifetimeModel::Value)
        && traits(reovim_foundation::carrier::LifetimeModel::InstanceBound)
        && traits(reovim_foundation::carrier::LifetimeModel::IncarnationBound)
}

fn concurrency_traits() -> bool {
    traits(reovim_foundation::carrier::ThreadTransfer::Transferable)
        && traits(reovim_foundation::carrier::ThreadTransfer::ThreadAffine)
        && traits(reovim_foundation::carrier::SharingModel::Exclusive)
        && traits(reovim_foundation::carrier::SharingModel::Immutable)
        && traits(reovim_foundation::carrier::SharingModel::Synchronized)
}

fn invocation_traits() -> bool {
    traits(reovim_foundation::carrier::InvocationModel::DataOnly)
        && traits(reovim_foundation::carrier::InvocationModel::NoUserCallback)
        && traits(reovim_foundation::carrier::InvocationModel::UserCallback)
        && traits(reovim_foundation::carrier::UnwindModel::NotInvocable)
        && traits(reovim_foundation::carrier::UnwindModel::Contained)
        && traits(
            reovim_foundation::carrier::UnwindModel::ForbiddenAcrossBoundary,
        )
}

const fn derive_witnesses() -> bool {
    bound_derive_witnesses()
        && carrier_derive_witnesses()
        && contract_derive_witnesses()
        && identity_derive_witnesses()
}

const fn bound_derive_witnesses() -> bool {
    derive_witness::<reovim_foundation::bound::ArithmeticRule>()
        && derive_witness::<reovim_foundation::bound::BoundDescriptor>()
        && derive_witness::<reovim_foundation::bound::BoundError>()
        && derive_witness::<reovim_foundation::bound::BoundUnit>()
        && derive_witness::<reovim_foundation::bound::BoundValue>()
        && derive_witness::<reovim_foundation::bound::CheckedByteCount>()
        && derive_witness::<reovim_foundation::bound::CheckedCount>()
        && derive_witness::<reovim_foundation::bound::CheckedDuration>()
        && derive_witness::<reovim_foundation::bound::ValidationOrder>()
        && derive_witness::<reovim_foundation::bound::ZeroPolicy>()
}

const fn carrier_derive_witnesses() -> bool {
    derive_witness::<reovim_foundation::carrier::BorrowModel>()
        && derive_witness::<reovim_foundation::carrier::CarrierDescriptorError>(
        )
        && carrier_descriptor_derive_witness()
        && derive_witness::<reovim_foundation::carrier::ConcurrencySafety>()
        && derive_witness::<reovim_foundation::carrier::CopyModel>()
        && derive_witness::<reovim_foundation::carrier::DropModel>()
        && derive_witness::<reovim_foundation::carrier::InvocationModel>()
        && derive_witness::<reovim_foundation::carrier::InvocationSafety>()
        && derive_witness::<reovim_foundation::carrier::LifetimeModel>()
        && derive_witness::<reovim_foundation::carrier::MemoryCoherenceError>()
        && derive_witness::<reovim_foundation::carrier::MemorySafety>()
        && derive_witness::<reovim_foundation::carrier::OwnershipModel>()
        && derive_witness::<reovim_foundation::carrier::SharingModel>()
        && derive_witness::<reovim_foundation::carrier::ThreadTransfer>()
        && derive_witness::<reovim_foundation::carrier::TypeMismatch>()
        && derive_witness::<reovim_foundation::carrier::UnsafeBoundary>()
        && derive_witness::<reovim_foundation::carrier::UnwindModel>()
}

const fn carrier_descriptor_derive_witness() -> bool {
    derive_witness::<reovim_foundation::carrier::CarrierSafetyDescriptor>()
}

const fn contract_derive_witnesses() -> bool {
    derive_witness::<reovim_foundation::contract::CompatibleVersionRange>()
        && derive_witness::<reovim_foundation::contract::ContractId>()
        && derive_witness::<reovim_foundation::contract::ContractVersion>()
        && derive_witness::<reovim_foundation::contract::VersionRangeError>()
}

const fn identity_derive_witnesses() -> bool {
    derive_witness::<reovim_foundation::identity::IncarnationId>()
        && derive_witness::<reovim_foundation::identity::InstanceId>()
}

const fn derive_witness<T>() -> bool
where
    T: Clone + Copy + Debug + PartialEq + Eq,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

fn clone_value<T: Clone>(value: &T) -> T {
    value.clone()
}

const fn copy_pair<T: Copy>(value: T) -> (T, T) {
    (value, value)
}

const fn require_debug<T: Debug>(_value: &T) {}

fn require_eq<T: Eq>(left: &T, right: &T) -> bool {
    left == right
}

fn traits<T>(value: T) -> bool
where
    T: Clone + Copy + Debug + Eq,
{
    let cloned = clone_value(&value);
    let (left, right) = copy_pair(cloned);
    require_debug(&left);
    require_eq(&left, &right)
}
