//! Exact compiler-resolved Foundation public function items.

pub const fn witness() -> bool {
    bound_items();
    carrier_items();
    contract_items();
    identity_items();
    variant_items();
    true
}

const fn bound_items() {
    let _ = (
        ::reovim_foundation::bound::BoundDescriptor::admit_aggregate,
        ::reovim_foundation::bound::BoundDescriptor::admit_per_object,
        ::reovim_foundation::bound::BoundDescriptor::aggregate,
        ::reovim_foundation::bound::BoundDescriptor::arithmetic_rule,
        ::reovim_foundation::bound::BoundDescriptor::charge_owner,
        ::reovim_foundation::bound::BoundDescriptor::new,
        ::reovim_foundation::bound::BoundDescriptor::per_object,
        ::reovim_foundation::bound::BoundDescriptor::unit,
        ::reovim_foundation::bound::BoundDescriptor::validation_order,
        ::reovim_foundation::bound::BoundDescriptor::zero_policy,
        ::reovim_foundation::bound::BoundValue::checked_add,
        ::reovim_foundation::bound::BoundValue::unit,
        ::reovim_foundation::bound::BoundValue::value,
        ::reovim_foundation::bound::CheckedByteCount::checked_add,
        ::reovim_foundation::bound::CheckedByteCount::new,
        ::reovim_foundation::bound::CheckedByteCount::value,
        ::reovim_foundation::bound::CheckedCount::checked_add,
        ::reovim_foundation::bound::CheckedCount::new,
        ::reovim_foundation::bound::CheckedCount::value,
        ::reovim_foundation::bound::CheckedDuration::checked_add,
        ::reovim_foundation::bound::CheckedDuration::from_nanoseconds,
        ::reovim_foundation::bound::CheckedDuration::nanoseconds,
    );
}

const fn carrier_items() {
    let _ = (
        ::reovim_foundation::carrier::CarrierDescriptorError::invocation,
        ::reovim_foundation::carrier::CarrierDescriptorError::memory,
        ::reovim_foundation::carrier::CarrierDescriptorError::unwind,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::concurrency,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::invocation,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::memory,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::mismatch,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::new,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::schema_versions,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::type_id,
        ::reovim_foundation::carrier::CarrierSafetyDescriptor::unsafe_boundary,
        ::reovim_foundation::carrier::ConcurrencySafety::new,
        ::reovim_foundation::carrier::ConcurrencySafety::sharing,
        ::reovim_foundation::carrier::ConcurrencySafety::thread_transfer,
        ::reovim_foundation::carrier::InvocationSafety::invocation,
        ::reovim_foundation::carrier::InvocationSafety::new,
        ::reovim_foundation::carrier::InvocationSafety::unwind,
        ::reovim_foundation::carrier::MemorySafety::borrowing,
        ::reovim_foundation::carrier::MemorySafety::copying,
        ::reovim_foundation::carrier::MemorySafety::dropping,
        ::reovim_foundation::carrier::MemorySafety::lifetime,
        ::reovim_foundation::carrier::MemorySafety::new,
        ::reovim_foundation::carrier::MemorySafety::ownership,
    );
}

const fn contract_items() {
    let _ = (
        ::reovim_foundation::contract::CompatibleVersionRange::contains,
        ::reovim_foundation::contract::CompatibleVersionRange::maximum,
        ::reovim_foundation::contract::CompatibleVersionRange::minimum,
        ::reovim_foundation::contract::CompatibleVersionRange::new,
        ::reovim_foundation::contract::ContractId::new,
        ::reovim_foundation::contract::ContractId::value,
        ::reovim_foundation::contract::ContractVersion::new,
        ::reovim_foundation::contract::ContractVersion::value,
        ::reovim_foundation::contract::VersionRangeError::maximum,
        ::reovim_foundation::contract::VersionRangeError::minimum,
    );
}

const fn identity_items() {
    let _ = (
        ::reovim_foundation::identity::IncarnationId::new,
        ::reovim_foundation::identity::IncarnationId::value,
        ::reovim_foundation::identity::InstanceId::new,
        ::reovim_foundation::identity::InstanceId::value,
    );
}

const fn variant_items() {
    let _ = (
        bound_arithmetic_rule_variants,
        bound_bound_error_variants,
        bound_bound_unit_variants,
        bound_bound_value_variants,
        bound_validation_order_variants,
        bound_zero_policy_variants,
        carrier_borrow_model_variants,
        carrier_carrier_descriptor_error_variants,
        carrier_copy_model_variants,
        carrier_drop_model_variants,
        carrier_invocation_model_variants,
        carrier_lifetime_model_variants,
        carrier_memory_coherence_error_variants,
        carrier_ownership_model_variants,
        carrier_sharing_model_variants,
        carrier_thread_transfer_variants,
        carrier_type_mismatch_variants,
        carrier_unsafe_boundary_variants,
        carrier_unwind_model_variants,
    );
}

const fn bound_arithmetic_rule_variants(
    value: ::reovim_foundation::bound::ArithmeticRule,
) {
    match value {
        ::reovim_foundation::bound::ArithmeticRule::Checked => {}
    }
}

const fn bound_bound_error_variants(
    value: ::reovim_foundation::bound::BoundError,
) {
    match value {
        ::reovim_foundation::bound::BoundError::ZeroRejected { unit: _ }
        | ::reovim_foundation::bound::BoundError::ArithmeticOverflow {
            unit: _,
            left: _,
            right: _,
        }
        | ::reovim_foundation::bound::BoundError::UnitMismatch {
            expected: _,
            actual: _,
        }
        | ::reovim_foundation::bound::BoundError::IncoherentDescriptor {
            per_object: _,
            aggregate: _,
        }
        | ::reovim_foundation::bound::BoundError::Exhausted {
            unit: _,
            requested: _,
            maximum: _,
        } => {}
    }
}

const fn bound_bound_unit_variants(
    value: ::reovim_foundation::bound::BoundUnit,
) {
    match value {
        ::reovim_foundation::bound::BoundUnit::Count
        | ::reovim_foundation::bound::BoundUnit::Bytes
        | ::reovim_foundation::bound::BoundUnit::DurationNanoseconds => {}
    }
}

const fn bound_bound_value_variants(
    value: ::reovim_foundation::bound::BoundValue,
) {
    match value {
        ::reovim_foundation::bound::BoundValue::Count(_)
        | ::reovim_foundation::bound::BoundValue::Bytes(_)
        | ::reovim_foundation::bound::BoundValue::Duration(_) => {}
    }
}

const fn bound_validation_order_variants(
    value: ::reovim_foundation::bound::ValidationOrder,
) {
    match value {
        ::reovim_foundation::bound::ValidationOrder::
            BeforeTraversalAndAllocation => {}
    }
}

const fn bound_zero_policy_variants(
    value: ::reovim_foundation::bound::ZeroPolicy,
) {
    match value {
        ::reovim_foundation::bound::ZeroPolicy::Allow
        | ::reovim_foundation::bound::ZeroPolicy::Reject => {}
    }
}

const fn carrier_borrow_model_variants(
    value: ::reovim_foundation::carrier::BorrowModel,
) {
    match value {
        ::reovim_foundation::carrier::BorrowModel::Forbidden
        | ::reovim_foundation::carrier::BorrowModel::Shared
        | ::reovim_foundation::carrier::BorrowModel::Exclusive => {}
    }
}

const fn carrier_carrier_descriptor_error_variants(
    value: ::reovim_foundation::carrier::CarrierDescriptorError,
) {
    match value {
        ::reovim_foundation::carrier::CarrierDescriptorError::Memory(_)
        | ::reovim_foundation::carrier::CarrierDescriptorError::
            InvocationUnwind { invocation : _, unwind : _ } => {}
    }
}

const fn carrier_copy_model_variants(
    value: ::reovim_foundation::carrier::CopyModel,
) {
    match value {
        ::reovim_foundation::carrier::CopyModel::MoveOnly
        | ::reovim_foundation::carrier::CopyModel::Copy => {}
    }
}

const fn carrier_drop_model_variants(
    value: ::reovim_foundation::carrier::DropModel,
) {
    match value {
        ::reovim_foundation::carrier::DropModel::Trivial
        | ::reovim_foundation::carrier::DropModel::ReleasesOwnedState => {}
    }
}

const fn carrier_invocation_model_variants(
    value: ::reovim_foundation::carrier::InvocationModel,
) {
    match value {
        ::reovim_foundation::carrier::InvocationModel::DataOnly
        | ::reovim_foundation::carrier::InvocationModel::NoUserCallback
        | ::reovim_foundation::carrier::InvocationModel::UserCallback => {}
    }
}

const fn carrier_lifetime_model_variants(
    value: ::reovim_foundation::carrier::LifetimeModel,
) {
    match value {
        ::reovim_foundation::carrier::LifetimeModel::Value
        | ::reovim_foundation::carrier::LifetimeModel::InstanceBound
        | ::reovim_foundation::carrier::LifetimeModel::IncarnationBound => {}
    }
}

const fn carrier_memory_coherence_error_variants(
    value: ::reovim_foundation::carrier::MemoryCoherenceError,
) {
    match value {
        ::reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedWithoutBorrow
        | ::reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedValueLifetime
        | ::reovim_foundation::carrier::MemoryCoherenceError::
            BorrowedReleasesOwnedState
        | ::reovim_foundation::carrier::MemoryCoherenceError::
            CopyDuplicatesReleaseAuthority => {}
    }
}

const fn carrier_ownership_model_variants(
    value: ::reovim_foundation::carrier::OwnershipModel,
) {
    match value {
        ::reovim_foundation::carrier::OwnershipModel::Owned
        | ::reovim_foundation::carrier::OwnershipModel::Borrowed => {}
    }
}

const fn carrier_sharing_model_variants(
    value: ::reovim_foundation::carrier::SharingModel,
) {
    match value {
        ::reovim_foundation::carrier::SharingModel::Exclusive
        | ::reovim_foundation::carrier::SharingModel::Immutable
        | ::reovim_foundation::carrier::SharingModel::Synchronized => {}
    }
}

const fn carrier_thread_transfer_variants(
    value: ::reovim_foundation::carrier::ThreadTransfer,
) {
    match value {
        ::reovim_foundation::carrier::ThreadTransfer::Transferable
        | ::reovim_foundation::carrier::ThreadTransfer::ThreadAffine => {}
    }
}

const fn carrier_type_mismatch_variants(
    value: ::reovim_foundation::carrier::TypeMismatch,
) {
    match value {
        ::reovim_foundation::carrier::TypeMismatch::Reject => {}
    }
}

const fn carrier_unsafe_boundary_variants(
    value: ::reovim_foundation::carrier::UnsafeBoundary,
) {
    match value {
        ::reovim_foundation::carrier::UnsafeBoundary::None
        | ::reovim_foundation::carrier::UnsafeBoundary::Reviewed(_) => {}
    }
}

const fn carrier_unwind_model_variants(
    value: ::reovim_foundation::carrier::UnwindModel,
) {
    match value {
        ::reovim_foundation::carrier::UnwindModel::NotInvocable
        | ::reovim_foundation::carrier::UnwindModel::ForbiddenAcrossBoundary
        | ::reovim_foundation::carrier::UnwindModel::Contained => {}
    }
}
