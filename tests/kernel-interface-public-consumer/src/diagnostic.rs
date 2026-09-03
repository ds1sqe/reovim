//! Public-only `DiagnosticOutput` contract and ownership journey.

use core::fmt::Debug;

use reovim_kernel_interface::bound::{
    ArithmeticRule, BoundDescriptor, BoundError, BoundUnit, BoundValue,
    CheckedByteCount, CheckedCount, CheckedDuration, ValidationOrder,
    ZeroPolicy,
};
use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierDescriptorError, CarrierSafetyDescriptor,
    ConcurrencySafety, CopyModel, DropModel, InvocationModel, InvocationSafety,
    LifetimeModel, MemoryCoherenceError, MemorySafety, OwnershipModel,
    SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary, UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion, VersionRangeError,
};
use reovim_kernel_interface::diagnostic::{
    DiagnosticClosedCause, DiagnosticDropCause, DiagnosticOutput,
    DiagnosticOutputCompletionBounds, DiagnosticOutputDescriptor,
    DiagnosticOutputError, DiagnosticOutputLimits, DiagnosticOutputOperation,
    DiagnosticOutputReadinessPredicate, DiagnosticOutputRecord,
    DiagnosticRecordContract, DiagnosticRecordError, DiagnosticShutdownPolicy,
    DiagnosticSubmissionOutcome, DiagnosticSubmissionStatus,
};
use reovim_kernel_interface::event::CanonicalEventRecordRef;
use reovim_kernel_interface::identity::{IncarnationId, InstanceId};
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::{
    CallbackClass, CompletionClass, ContractSelection, DeliveryClass,
    EffectClass, InvocationClass, OperationDescriptor, ReentrancyClass,
};

use super::{error_traits, event, require_debug, traits};

const OUTPUT_CONTRACT: u128 = 0x61;
const OUTPUT_INSTANCE: u128 = 0x62;
const OUTPUT_INCARNATION: u128 = 0x63;
const RECORD_TYPE: u128 = 0x64;
const READINESS_DERIVE_WITNESS: bool = crate::derive_witness::<
    ::reovim_kernel_interface::diagnostic::DiagnosticOutputReadinessPredicate,
>();

pub const fn const_cases() -> bool {
    contract_identity_facade_cases()
        && bound_facade_cases()
        && carrier_facade_cases()
        && completion_bound_cases()
        && output_limit_cases()
        && descriptor_cases()
        && descriptor_error_cases()
        && derive_cases()
}

const fn contract_identity_facade_cases() -> bool {
    let contract = ContractId::new(0x81);
    let minimum = ContractVersion::new(1);
    let maximum = ContractVersion::new(3);
    let Ok(range) = CompatibleVersionRange::new(minimum, maximum) else {
        return false;
    };
    let Err(inverted): Result<_, VersionRangeError> =
        CompatibleVersionRange::new(maximum, minimum)
    else {
        return false;
    };
    let instance = InstanceId::new(0x82);
    let incarnation = IncarnationId::new(0x83);
    contract.value() == 0x81
        && range.minimum().value() == 1
        && range.maximum().value() == 3
        && range.contains(ContractVersion::new(2))
        && !range.contains(ContractVersion::new(4))
        && inverted.minimum().value() == 3
        && inverted.maximum().value() == 1
        && instance.value() == 0x82
        && incarnation.value() == 0x83
}

const fn bound_facade_cases() -> bool {
    let Ok(one) = CheckedCount::new(1, ZeroPolicy::Allow) else {
        return false;
    };
    let Ok(two) = CheckedCount::new(2, ZeroPolicy::Reject) else {
        return false;
    };
    let Ok(one_byte) = CheckedByteCount::new(1, ZeroPolicy::Reject) else {
        return false;
    };
    let Ok(one_ns) = CheckedDuration::from_nanoseconds(1, ZeroPolicy::Allow)
    else {
        return false;
    };
    let Ok(bound) = BoundDescriptor::new(
        BoundValue::Count(one),
        BoundValue::Count(two),
        ZeroPolicy::Allow,
        InstanceId::new(0x84),
    ) else {
        return false;
    };
    bound_unit_code(bound.unit()) == 0
        && bound.per_object().value() == 1
        && bound.aggregate().value() == 2
        && zero_policy_code(bound.zero_policy()) == 0
        && bound.charge_owner().value() == 0x84
        && arithmetic_rule_code(bound.arithmetic_rule()) == 0
        && validation_order_code(bound.validation_order()) == 0
        && bound.admit_per_object(BoundValue::Count(one)).is_ok()
        && bound
            .admit_aggregate(BoundValue::Count(one), BoundValue::Count(one))
            .is_ok()
        && one.checked_add(one).is_ok()
        && one_byte.checked_add(one_byte).is_ok()
        && one_ns.checked_add(one_ns).is_ok()
        && bound_unit_code(BoundValue::Bytes(one_byte).unit()) == 1
        && bound_unit_code(BoundValue::Duration(one_ns).unit()) == 2
        && exact_bound_errors(one, two, one_byte)
}

const fn exact_bound_errors(
    one: CheckedCount,
    two: CheckedCount,
    one_byte: CheckedByteCount,
) -> bool {
    let zero = CheckedCount::new(0, ZeroPolicy::Reject);
    let Ok(maximum) = CheckedCount::new(u64::MAX, ZeroPolicy::Allow) else {
        return false;
    };
    let overflow = maximum.checked_add(one);
    let unit = BoundValue::Count(one).checked_add(BoundValue::Bytes(one_byte));
    let incoherent = BoundDescriptor::new(
        BoundValue::Count(two),
        BoundValue::Count(one),
        ZeroPolicy::Allow,
        InstanceId::new(0x84),
    );
    let Ok(bound) = BoundDescriptor::new(
        BoundValue::Count(one),
        BoundValue::Count(two),
        ZeroPolicy::Allow,
        InstanceId::new(0x84),
    ) else {
        return false;
    };
    let Ok(three) = CheckedCount::new(3, ZeroPolicy::Allow) else {
        return false;
    };
    let exhausted = bound.admit_per_object(BoundValue::Count(three));
    let Err(zero_error) = zero else {
        return false;
    };
    let Err(overflow_error) = overflow else {
        return false;
    };
    let Err(unit_error) = unit else {
        return false;
    };
    let Err(incoherent_error) = incoherent else {
        return false;
    };
    let Err(exhausted_error) = exhausted else {
        return false;
    };
    zero_rejected_is(zero_error, 0)
        && arithmetic_overflow_is(overflow_error, 0, u64::MAX, 1)
        && unit_mismatch_is(unit_error, 0, 1)
        && incoherent_bound_is(incoherent_error, 2, 1)
        && exhausted_bound_is(exhausted_error, 0, 3, 1)
}

const fn bound_unit_code(value: BoundUnit) -> u8 {
    match value {
        BoundUnit::Count => 0,
        BoundUnit::Bytes => 1,
        BoundUnit::DurationNanoseconds => 2,
    }
}

const fn zero_policy_code(value: ZeroPolicy) -> u8 {
    match value {
        ZeroPolicy::Allow => 0,
        ZeroPolicy::Reject => 1,
    }
}

const fn arithmetic_rule_code(value: ArithmeticRule) -> u8 {
    match value {
        ArithmeticRule::Checked => 0,
    }
}

const fn validation_order_code(value: ValidationOrder) -> u8 {
    match value {
        ValidationOrder::BeforeTraversalAndAllocation => 0,
    }
}

const fn zero_rejected_is(error: BoundError, unit: u8) -> bool {
    match error {
        BoundError::ZeroRejected { unit: actual } => {
            bound_unit_code(actual) == unit
        }
        BoundError::ArithmeticOverflow { .. }
        | BoundError::UnitMismatch { .. }
        | BoundError::IncoherentDescriptor { .. }
        | BoundError::Exhausted { .. } => false,
    }
}

const fn arithmetic_overflow_is(
    error: BoundError,
    unit: u8,
    left: u64,
    right: u64,
) -> bool {
    match error {
        BoundError::ArithmeticOverflow {
            unit: actual,
            left: actual_left,
            right: actual_right,
        } => {
            bound_unit_code(actual) == unit
                && actual_left == left
                && actual_right == right
        }
        BoundError::ZeroRejected { .. }
        | BoundError::UnitMismatch { .. }
        | BoundError::IncoherentDescriptor { .. }
        | BoundError::Exhausted { .. } => false,
    }
}

const fn unit_mismatch_is(error: BoundError, expected: u8, actual: u8) -> bool {
    match error {
        BoundError::UnitMismatch {
            expected: actual_expected,
            actual: actual_unit,
        } => {
            bound_unit_code(actual_expected) == expected
                && bound_unit_code(actual_unit) == actual
        }
        BoundError::ZeroRejected { .. }
        | BoundError::ArithmeticOverflow { .. }
        | BoundError::IncoherentDescriptor { .. }
        | BoundError::Exhausted { .. } => false,
    }
}

const fn incoherent_bound_is(
    error: BoundError,
    per_object: u64,
    aggregate: u64,
) -> bool {
    match error {
        BoundError::IncoherentDescriptor {
            per_object: actual_object,
            aggregate: actual_aggregate,
        } => actual_object == per_object && actual_aggregate == aggregate,
        BoundError::ZeroRejected { .. }
        | BoundError::ArithmeticOverflow { .. }
        | BoundError::UnitMismatch { .. }
        | BoundError::Exhausted { .. } => false,
    }
}

const fn exhausted_bound_is(
    error: BoundError,
    unit: u8,
    requested: u64,
    maximum: u64,
) -> bool {
    match error {
        BoundError::Exhausted {
            unit: actual,
            requested: actual_request,
            maximum: actual_maximum,
        } => {
            bound_unit_code(actual) == unit
                && actual_request == requested
                && actual_maximum == maximum
        }
        BoundError::ZeroRejected { .. }
        | BoundError::ArithmeticOverflow { .. }
        | BoundError::UnitMismatch { .. }
        | BoundError::IncoherentDescriptor { .. } => false,
    }
}

const fn carrier_facade_cases() -> bool {
    let Some(safety) = make_record_safety(SharingModel::Immutable) else {
        return false;
    };
    owned_safety_axes(safety) && carrier_error_cases()
}

const fn carrier_error_cases() -> bool {
    let Ok(versions) = CompatibleVersionRange::new(
        ContractVersion::new(1),
        ContractVersion::new(2),
    ) else {
        return false;
    };
    let concurrency = ConcurrencySafety::new(
        ThreadTransfer::Transferable,
        SharingModel::Immutable,
    );
    let data = InvocationSafety::new(
        InvocationModel::DataOnly,
        UnwindModel::NotInvocable,
    );
    let borrowed_without_borrow = MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Forbidden,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    );
    let borrowed_value = MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::Value,
    );
    let borrowed_release = MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::ReleasesOwnedState,
        LifetimeModel::IncarnationBound,
    );
    let copy_release = MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::ReleasesOwnedState,
        LifetimeModel::Value,
    );
    let owned_memory = MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::ReleasesOwnedState,
        LifetimeModel::Value,
    );
    let invocation_mismatch = InvocationSafety::new(
        InvocationModel::DataOnly,
        UnwindModel::Contained,
    );
    carrier_memory_error(
        versions,
        concurrency,
        data,
        borrowed_without_borrow,
        MemoryCoherenceError::BorrowedWithoutBorrow,
    ) && carrier_memory_error(
        versions,
        concurrency,
        data,
        borrowed_value,
        MemoryCoherenceError::BorrowedValueLifetime,
    ) && carrier_memory_error(
        versions,
        concurrency,
        data,
        borrowed_release,
        MemoryCoherenceError::BorrowedReleasesOwnedState,
    ) && carrier_memory_error(
        versions,
        concurrency,
        data,
        copy_release,
        MemoryCoherenceError::CopyDuplicatesReleaseAuthority,
    ) && carrier_invocation_error(
        versions,
        concurrency,
        owned_memory,
        invocation_mismatch,
    )
}

const fn carrier_memory_error(
    versions: CompatibleVersionRange,
    concurrency: ConcurrencySafety,
    invocation: InvocationSafety,
    memory: MemorySafety,
    expected: MemoryCoherenceError,
) -> bool {
    let result = CarrierSafetyDescriptor::new(
        ContractId::new(RECORD_TYPE),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    );
    match result {
        Err(CarrierDescriptorError::Memory(actual)) => {
            memory_error_code(actual) == memory_error_code(expected)
        }
        Err(CarrierDescriptorError::InvocationUnwind { .. }) | Ok(_) => false,
    }
}

const fn carrier_invocation_error(
    versions: CompatibleVersionRange,
    concurrency: ConcurrencySafety,
    memory: MemorySafety,
    invocation: InvocationSafety,
) -> bool {
    let result = CarrierSafetyDescriptor::new(
        ContractId::new(RECORD_TYPE),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        UnsafeBoundary::None,
    );
    let Err(error) = result else { return false };
    error.memory().is_none()
        && optional_invocation_code(error.invocation()) == 1
        && optional_unwind_code(error.unwind()) == 2
}

const fn memory_error_code(value: MemoryCoherenceError) -> u8 {
    match value {
        MemoryCoherenceError::BorrowedWithoutBorrow => 0,
        MemoryCoherenceError::BorrowedValueLifetime => 1,
        MemoryCoherenceError::BorrowedReleasesOwnedState => 2,
        MemoryCoherenceError::CopyDuplicatesReleaseAuthority => 3,
    }
}

const fn optional_invocation_code(value: Option<InvocationModel>) -> u8 {
    match value {
        None => 0,
        Some(model) => invocation_model_code(model) + 1,
    }
}

const fn optional_unwind_code(value: Option<UnwindModel>) -> u8 {
    match value {
        None => 0,
        Some(model) => unwind_model_code(model) + 1,
    }
}

const fn output_identity() -> LifecycleIdentity {
    LifecycleIdentity::new(
        OUTPUT_INSTANCE,
        OUTPUT_INCARNATION,
        Generation::new(6),
    )
}

const fn output_owner() -> LifecycleIdentity {
    LifecycleIdentity::new(0x71, 0x72, Generation::new(7))
}

const fn make_completion() -> Option<DiagnosticOutputCompletionBounds> {
    match DiagnosticOutputCompletionBounds::new(100, 200) {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

const fn make_limits_for(
    output: LifecycleIdentity,
    maximum_record_bytes: u64,
) -> Option<DiagnosticOutputLimits> {
    let Some(completion) = make_completion() else {
        return None;
    };
    match DiagnosticOutputLimits::new(
        output,
        8,
        maximum_record_bytes,
        4,
        4_096,
        completion,
    ) {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

pub const fn make_descriptor() -> Option<DiagnosticOutputDescriptor> {
    make_descriptor_for(output_identity(), event::stream_identity(), 1_024)
}

const fn make_descriptor_for(
    output: LifecycleIdentity,
    stream: LifecycleIdentity,
    maximum_record_bytes: u64,
) -> Option<DiagnosticOutputDescriptor> {
    let Ok(contract) =
        reovim_kernel_interface::operation::ContractSelection::new(
            OUTPUT_CONTRACT,
            1,
            1,
            2,
        )
    else {
        return None;
    };
    let Some(limits) = make_limits_for(output, maximum_record_bytes) else {
        return None;
    };
    let Some(safety) = make_record_safety(SharingModel::Immutable) else {
        return None;
    };
    match DiagnosticOutputDescriptor::new(
        contract,
        output,
        output_owner(),
        stream,
        limits,
        record_contract(safety),
        DiagnosticShutdownPolicy::Drain,
    ) {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

const fn record_contract(
    safety: CarrierSafetyDescriptor,
) -> DiagnosticRecordContract {
    DiagnosticRecordContract::new(ContractId::new(RECORD_TYPE), safety)
}

const fn make_record_safety(
    sharing: SharingModel,
) -> Option<CarrierSafetyDescriptor> {
    let Some(versions) = make_schema_versions(1, 2) else {
        return None;
    };
    let concurrency =
        ConcurrencySafety::new(ThreadTransfer::Transferable, sharing);
    make_safety(
        ContractId::new(RECORD_TYPE),
        versions,
        exact_record_memory(),
        concurrency,
        exact_record_invocation(),
        UnsafeBoundary::None,
    )
}

const fn make_schema_versions(
    minimum: u64,
    maximum: u64,
) -> Option<CompatibleVersionRange> {
    match CompatibleVersionRange::new(
        ContractVersion::new(minimum),
        ContractVersion::new(maximum),
    ) {
        Ok(versions) => Some(versions),
        Err(_) => None,
    }
}

const fn exact_record_memory() -> MemorySafety {
    MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::ReleasesOwnedState,
        LifetimeModel::Value,
    )
}

const fn exact_record_concurrency() -> ConcurrencySafety {
    ConcurrencySafety::new(
        ThreadTransfer::Transferable,
        SharingModel::Immutable,
    )
}

const fn exact_record_invocation() -> InvocationSafety {
    InvocationSafety::new(InvocationModel::DataOnly, UnwindModel::NotInvocable)
}

const fn make_safety(
    type_id: ContractId,
    versions: CompatibleVersionRange,
    memory: MemorySafety,
    concurrency: ConcurrencySafety,
    invocation: InvocationSafety,
    boundary: UnsafeBoundary,
) -> Option<CarrierSafetyDescriptor> {
    match CarrierSafetyDescriptor::new(
        type_id,
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        boundary,
    ) {
        Ok(safety) => Some(safety),
        Err(_) => None,
    }
}

const fn completion_bound_cases() -> bool {
    let Some(completion) = make_completion() else {
        return false;
    };
    completion.activation().nanoseconds() == 100
        && completion.shutdown().nanoseconds() == 200
        && completion_zero_case(DiagnosticOutputCompletionBounds::new(0, 1))
        && completion_zero_case(DiagnosticOutputCompletionBounds::new(1, 0))
}

const fn completion_zero_case(
    result: Result<DiagnosticOutputCompletionBounds, BoundError>,
) -> bool {
    match result {
        Err(error) => zero_rejected_is(error, 2),
        Ok(_) => false,
    }
}

const fn output_limit_cases() -> bool {
    let Some(limits) = make_limits_for(output_identity(), 1_024) else {
        return false;
    };
    limits.submission_work().per_object().value() == 8
        && limits.submission_work().aggregate().value() == 8
        && limits.temporary_bytes().per_object().value() == 0
        && limits.temporary_bytes().aggregate().value() == 0
        && limits.record_capacity().per_object().value() == 1
        && limits.record_capacity().aggregate().value() == 4
        && limits.byte_capacity().per_object().value() == 1_024
        && limits.byte_capacity().aggregate().value() == 4_096
        && limits.completion().activation().nanoseconds() == 100
        && limits.shutdown_report_records().value() == 4
        && limits.shutdown_report_record_bytes().value() == 4_096
        && limits.output().same_generation(output_identity())
        && limits.submission_work().charge_owner().value() == OUTPUT_INSTANCE
        && limit_error_cases()
}

const fn limit_error_cases() -> bool {
    let Some(completion) = make_completion() else {
        return false;
    };
    let zero_work = DiagnosticOutputLimits::new(
        output_identity(),
        0,
        16,
        1,
        16,
        completion,
    );
    let zero_records = DiagnosticOutputLimits::new(
        output_identity(),
        1,
        16,
        0,
        16,
        completion,
    );
    let zero_record_bytes =
        DiagnosticOutputLimits::new(output_identity(), 1, 0, 1, 16, completion);
    let zero_outstanding_bytes =
        DiagnosticOutputLimits::new(output_identity(), 1, 16, 1, 0, completion);
    let incoherent_bytes = DiagnosticOutputLimits::new(
        output_identity(),
        1,
        17,
        1,
        16,
        completion,
    );
    let Err(zero_work_error) = zero_work else {
        return false;
    };
    let Err(zero_records_error) = zero_records else {
        return false;
    };
    let Err(zero_record_bytes_error) = zero_record_bytes else {
        return false;
    };
    let Err(zero_outstanding_error) = zero_outstanding_bytes else {
        return false;
    };
    let Err(incoherent_bytes_error) = incoherent_bytes else {
        return false;
    };
    output_zero_rejected_is(zero_work_error, 0)
        && output_zero_rejected_is(zero_records_error, 0)
        && output_zero_rejected_is(zero_record_bytes_error, 1)
        && output_zero_rejected_is(zero_outstanding_error, 1)
        && output_incoherent_bound_is(incoherent_bytes_error, 17, 16)
}

const fn output_zero_rejected_is(
    error: DiagnosticOutputError,
    unit: u8,
) -> bool {
    match error {
        DiagnosticOutputError::Bound(bound) => zero_rejected_is(bound, unit),
        DiagnosticOutputError::LimitsLifecycleMismatch
        | DiagnosticOutputError::IncoherentCarrier
        | DiagnosticOutputError::UnsupportedContract
        | DiagnosticOutputError::IncompatibleContract => false,
    }
}

const fn output_incoherent_bound_is(
    error: DiagnosticOutputError,
    per_object: u64,
    aggregate: u64,
) -> bool {
    match error {
        DiagnosticOutputError::Bound(bound) => {
            incoherent_bound_is(bound, per_object, aggregate)
        }
        DiagnosticOutputError::LimitsLifecycleMismatch
        | DiagnosticOutputError::IncoherentCarrier
        | DiagnosticOutputError::UnsupportedContract
        | DiagnosticOutputError::IncompatibleContract => false,
    }
}

const fn descriptor_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let record_contract = record_contract(descriptor.record_safety());
    descriptor.contract().identity() == OUTPUT_CONTRACT
        && descriptor.contract().selected() == 1
        && descriptor.lifecycle().same_generation(output_identity())
        && descriptor.owner().same_generation(output_owner())
        && descriptor.source_stream().same_generation(event::stream_identity())
        && descriptor.limits().byte_capacity().per_object().value() == 1_024
        && descriptor.record_type().value() == RECORD_TYPE
        && descriptor.record_safety().type_id().value() == RECORD_TYPE
        && descriptor.record_safety().schema_versions().minimum().value() == 1
        && descriptor.record_safety().schema_versions().maximum().value() == 2
        && record_contract.type_id().value() == RECORD_TYPE
        && record_contract.safety().type_id().value() == RECORD_TYPE
        && owned_safety_axes(record_contract.safety())
        && record_safety_cases(&descriptor)
        && shutdown_policy_code(descriptor.shutdown_policy()) == 0
        && readiness_code(descriptor.readiness_predicate()) == 0
        && descriptor.negotiate(OUTPUT_CONTRACT, 1).is_ok()
        && descriptor.negotiate(OUTPUT_CONTRACT, 2).is_ok()
        && shutdown_policy_code(DiagnosticShutdownPolicy::Discard) == 1
        && operation_matrix(&descriptor)
        && degraded_matrix(&descriptor)
}

const fn record_safety_cases(descriptor: &DiagnosticOutputDescriptor) -> bool {
    owned_safety_axes(descriptor.record_safety())
}

const fn owned_safety_axes(safety: CarrierSafetyDescriptor) -> bool {
    let memory = safety.memory();
    let concurrency = safety.concurrency();
    let invocation = safety.invocation();
    ownership_model_code(memory.ownership()) == 0
        && borrow_model_code(memory.borrowing()) == 1
        && copy_model_code(memory.copying()) == 0
        && drop_model_code(memory.dropping()) == 1
        && lifetime_model_code(memory.lifetime()) == 0
        && thread_transfer_code(concurrency.thread_transfer()) == 0
        && sharing_model_code(concurrency.sharing()) == 1
        && invocation_model_code(invocation.invocation()) == 0
        && unwind_model_code(invocation.unwind()) == 0
        && type_mismatch_code(safety.mismatch()) == 0
        && unsafe_boundary_code(safety.unsafe_boundary()) == 0
}

const fn ownership_model_code(value: OwnershipModel) -> u8 {
    match value {
        OwnershipModel::Owned => 0,
        OwnershipModel::Borrowed => 1,
    }
}

const fn borrow_model_code(value: BorrowModel) -> u8 {
    match value {
        BorrowModel::Forbidden => 0,
        BorrowModel::Shared => 1,
        BorrowModel::Exclusive => 2,
    }
}

const fn copy_model_code(value: CopyModel) -> u8 {
    match value {
        CopyModel::MoveOnly => 0,
        CopyModel::Copy => 1,
    }
}

const fn drop_model_code(value: DropModel) -> u8 {
    match value {
        DropModel::Trivial => 0,
        DropModel::ReleasesOwnedState => 1,
    }
}

const fn lifetime_model_code(value: LifetimeModel) -> u8 {
    match value {
        LifetimeModel::Value => 0,
        LifetimeModel::InstanceBound => 1,
        LifetimeModel::IncarnationBound => 2,
    }
}

const fn thread_transfer_code(value: ThreadTransfer) -> u8 {
    match value {
        ThreadTransfer::Transferable => 0,
        ThreadTransfer::ThreadAffine => 1,
    }
}

const fn sharing_model_code(value: SharingModel) -> u8 {
    match value {
        SharingModel::Exclusive => 0,
        SharingModel::Immutable => 1,
        SharingModel::Synchronized => 2,
    }
}

const fn invocation_model_code(value: InvocationModel) -> u8 {
    match value {
        InvocationModel::DataOnly => 0,
        InvocationModel::NoUserCallback => 1,
        InvocationModel::UserCallback => 2,
    }
}

const fn unwind_model_code(value: UnwindModel) -> u8 {
    match value {
        UnwindModel::NotInvocable => 0,
        UnwindModel::Contained => 1,
        UnwindModel::ForbiddenAcrossBoundary => 2,
    }
}

const fn type_mismatch_code(value: TypeMismatch) -> u8 {
    match value {
        TypeMismatch::Reject => 0,
    }
}

const fn unsafe_boundary_code(value: UnsafeBoundary) -> u8 {
    match value {
        UnsafeBoundary::None => 0,
        UnsafeBoundary::Reviewed(_) => 1,
    }
}

const fn operation_matrix(descriptor: &DiagnosticOutputDescriptor) -> bool {
    local_inline(
        descriptor.operation(DiagnosticOutputOperation::DescriptorInspection),
    ) && local_inline(
        descriptor.operation(DiagnosticOutputOperation::RecordPreparation),
    ) && local_inline(
        descriptor.operation(DiagnosticOutputOperation::Submission),
    ) && local_deferred(
        descriptor.operation(DiagnosticOutputOperation::Activation),
    ) && local_deferred(
        descriptor.operation(DiagnosticOutputOperation::Shutdown),
    )
}

const fn degraded_matrix(descriptor: &DiagnosticOutputDescriptor) -> bool {
    descriptor.degraded_operation_is_safe(
        DiagnosticOutputOperation::DescriptorInspection,
    ) && descriptor.degraded_operation_is_safe(
        DiagnosticOutputOperation::RecordPreparation,
    ) && descriptor
        .degraded_operation_is_safe(DiagnosticOutputOperation::Submission)
        && !descriptor
            .degraded_operation_is_safe(DiagnosticOutputOperation::Activation)
        && descriptor
            .degraded_operation_is_safe(DiagnosticOutputOperation::Shutdown)
}

const fn local_inline(operation: OperationDescriptor) -> bool {
    invocation_class_code(operation.invocation()) == 0
        && completion_class_code(operation.completion()) == 0
        && delivery_class_code(operation.delivery()) == 0
        && effect_class_code(operation.effect()) == 0
        && callback_class_code(operation.callback()) == 0
        && reentrancy_class_code(operation.reentrancy()) == 0
}

const fn local_deferred(operation: OperationDescriptor) -> bool {
    invocation_class_code(operation.invocation()) == 1
        && completion_class_code(operation.completion()) == 1
        && delivery_class_code(operation.delivery()) == 0
        && effect_class_code(operation.effect()) == 0
        && callback_class_code(operation.callback()) == 0
        && reentrancy_class_code(operation.reentrancy()) == 0
}

const fn invocation_class_code(value: InvocationClass) -> u8 {
    match value {
        InvocationClass::NonBlockingInvocation => 0,
        InvocationClass::CallerMayWait => 1,
    }
}

const fn completion_class_code(value: CompletionClass) -> u8 {
    match value {
        CompletionClass::InlineTerminal => 0,
        CompletionClass::DeferredTerminal => 1,
    }
}

const fn delivery_class_code(value: DeliveryClass) -> u8 {
    match value {
        DeliveryClass::Single => 0,
        DeliveryClass::Stream => 1,
    }
}

const fn effect_class_code(value: EffectClass) -> u8 {
    match value {
        EffectClass::LocalOnly => 0,
        EffectClass::ExternalObservation => 1,
        EffectClass::ExternalMutation => 2,
    }
}

const fn callback_class_code(value: CallbackClass) -> u8 {
    match value {
        CallbackClass::NoUserCallback => 0,
        CallbackClass::MayInvokeUserCallback => 1,
    }
}

const fn reentrancy_class_code(value: ReentrancyClass) -> u8 {
    match value {
        ReentrancyClass::NonReentrant => 0,
        ReentrancyClass::Reentrant => 1,
    }
}

const fn descriptor_error_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Ok(contract) = ContractSelection::new(OUTPUT_CONTRACT, 1, 1, 2) else {
        return false;
    };
    let Some(safety) = make_record_safety(SharingModel::Immutable) else {
        return false;
    };
    let Err(unsupported) = descriptor.negotiate(OUTPUT_CONTRACT + 1, 1) else {
        return false;
    };
    let Err(incompatible) = descriptor.negotiate(OUTPUT_CONTRACT, 3) else {
        return false;
    };
    lifecycle_mismatch_cases(contract, safety)
        && record_contract_mismatch_matrix(contract)
        && diagnostic_output_error_code(unsupported) == 3
        && diagnostic_output_error_code(incompatible) == 4
}

const fn diagnostic_output_error_code(error: DiagnosticOutputError) -> u8 {
    match error {
        DiagnosticOutputError::Bound(_) => 0,
        DiagnosticOutputError::LimitsLifecycleMismatch => 1,
        DiagnosticOutputError::IncoherentCarrier => 2,
        DiagnosticOutputError::UnsupportedContract => 3,
        DiagnosticOutputError::IncompatibleContract => 4,
    }
}

const fn lifecycle_mismatch_cases(
    contract: ContractSelection,
    safety: CarrierSafetyDescriptor,
) -> bool {
    let Some(limits) = make_limits_for(output_identity(), 1_024) else {
        return false;
    };
    let other_output = LifecycleIdentity::new(
        OUTPUT_INSTANCE + 1,
        OUTPUT_INCARNATION,
        Generation::new(6),
    );
    let mismatch = DiagnosticOutputDescriptor::new(
        contract,
        other_output,
        output_owner(),
        event::stream_identity(),
        limits,
        record_contract(safety),
        DiagnosticShutdownPolicy::Discard,
    );
    let Some(stale_limits) = make_limits_for(output_identity(), 1_024) else {
        return false;
    };
    let stale_output = LifecycleIdentity::new(
        OUTPUT_INSTANCE,
        OUTPUT_INCARNATION,
        Generation::new(7),
    );
    let stale_mismatch = DiagnosticOutputDescriptor::new(
        contract,
        stale_output,
        output_owner(),
        event::stream_identity(),
        stale_limits,
        record_contract(safety),
        DiagnosticShutdownPolicy::Drain,
    );
    let other_incarnation = LifecycleIdentity::new(
        OUTPUT_INSTANCE,
        OUTPUT_INCARNATION + 1,
        Generation::new(6),
    );
    let Some(incarnation_limits) = make_limits_for(other_incarnation, 1_024)
    else {
        return false;
    };
    let incarnation_mismatch = DiagnosticOutputDescriptor::new(
        contract,
        output_identity(),
        output_owner(),
        event::stream_identity(),
        incarnation_limits,
        record_contract(safety),
        DiagnosticShutdownPolicy::Drain,
    );
    let Err(mismatch_error) = mismatch else {
        return false;
    };
    let Err(stale_error) = stale_mismatch else {
        return false;
    };
    let Err(incarnation_error) = incarnation_mismatch else {
        return false;
    };
    diagnostic_output_error_code(mismatch_error) == 1
        && diagnostic_output_error_code(stale_error) == 1
        && diagnostic_output_error_code(incarnation_error) == 1
}

const fn record_contract_mismatch_matrix(contract: ContractSelection) -> bool {
    record_identity_mismatch_cases(contract)
        && record_schema_mismatch_cases(contract)
        && record_memory_mismatch_cases(contract)
        && record_concurrency_mismatch_cases(contract)
        && record_invocation_mismatch_cases(contract)
        && record_boundary_mismatch_case(contract)
        && singleton_mismatch_axis_case()
}

const fn rejects_record_contract(
    contract: ContractSelection,
    record_contract: DiagnosticRecordContract,
) -> bool {
    let Some(limits) = make_limits_for(output_identity(), 1_024) else {
        return false;
    };
    let result = DiagnosticOutputDescriptor::new(
        contract,
        output_identity(),
        output_owner(),
        event::stream_identity(),
        limits,
        record_contract,
        DiagnosticShutdownPolicy::Drain,
    );
    let Err(error) = result else {
        return false;
    };
    diagnostic_output_error_code(error) == 2
}

const fn record_identity_mismatch_cases(contract: ContractSelection) -> bool {
    let Some(exact_safety) = make_record_safety(SharingModel::Immutable) else {
        return false;
    };
    let expected_type_mismatch = DiagnosticRecordContract::new(
        ContractId::new(RECORD_TYPE + 1),
        exact_safety,
    );
    let Some(versions) = make_schema_versions(1, 2) else {
        return false;
    };
    let Some(other_safety) = make_safety(
        ContractId::new(RECORD_TYPE + 1),
        versions,
        exact_record_memory(),
        exact_record_concurrency(),
        exact_record_invocation(),
        UnsafeBoundary::None,
    ) else {
        return false;
    };
    let safety_type_mismatch = record_contract(other_safety);
    rejects_record_contract(contract, expected_type_mismatch)
        && rejects_record_contract(contract, safety_type_mismatch)
}

const fn record_schema_mismatch_cases(contract: ContractSelection) -> bool {
    let Some(lower_minimum) = make_schema_versions(0, 2) else {
        return false;
    };
    let Some(higher_maximum) = make_schema_versions(1, 3) else {
        return false;
    };
    rejects_schema_versions(contract, lower_minimum)
        && rejects_schema_versions(contract, higher_maximum)
}

const fn rejects_schema_versions(
    contract: ContractSelection,
    versions: CompatibleVersionRange,
) -> bool {
    let Some(safety) = make_safety(
        ContractId::new(RECORD_TYPE),
        versions,
        exact_record_memory(),
        exact_record_concurrency(),
        exact_record_invocation(),
        UnsafeBoundary::None,
    ) else {
        return false;
    };
    rejects_record_contract(contract, record_contract(safety))
}

const fn record_memory_mismatch_cases(contract: ContractSelection) -> bool {
    rejects_record_memory(contract, borrowed_record_memory())
        && rejects_record_memory(
            contract,
            MemorySafety::new(
                OwnershipModel::Owned,
                BorrowModel::Forbidden,
                CopyModel::MoveOnly,
                DropModel::ReleasesOwnedState,
                LifetimeModel::Value,
            ),
        )
        && rejects_record_memory(
            contract,
            MemorySafety::new(
                OwnershipModel::Owned,
                BorrowModel::Exclusive,
                CopyModel::MoveOnly,
                DropModel::ReleasesOwnedState,
                LifetimeModel::Value,
            ),
        )
        && rejects_record_memory(contract, copied_record_memory())
        && rejects_record_memory(
            contract,
            MemorySafety::new(
                OwnershipModel::Owned,
                BorrowModel::Shared,
                CopyModel::MoveOnly,
                DropModel::Trivial,
                LifetimeModel::Value,
            ),
        )
        && rejects_record_memory(
            contract,
            MemorySafety::new(
                OwnershipModel::Owned,
                BorrowModel::Shared,
                CopyModel::MoveOnly,
                DropModel::ReleasesOwnedState,
                LifetimeModel::InstanceBound,
            ),
        )
        && rejects_record_memory(
            contract,
            MemorySafety::new(
                OwnershipModel::Owned,
                BorrowModel::Shared,
                CopyModel::MoveOnly,
                DropModel::ReleasesOwnedState,
                LifetimeModel::IncarnationBound,
            ),
        )
}

const fn borrowed_record_memory() -> MemorySafety {
    // Foundation requires borrowed state to be scoped and not own release.
    MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    )
}

const fn copied_record_memory() -> MemorySafety {
    // Foundation requires copied state to carry no release authority.
    MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::Value,
    )
}

const fn rejects_record_memory(
    contract: ContractSelection,
    memory: MemorySafety,
) -> bool {
    let Some(versions) = make_schema_versions(1, 2) else {
        return false;
    };
    let Some(safety) = make_safety(
        ContractId::new(RECORD_TYPE),
        versions,
        memory,
        exact_record_concurrency(),
        exact_record_invocation(),
        UnsafeBoundary::None,
    ) else {
        return false;
    };
    rejects_record_contract(contract, record_contract(safety))
}

const fn record_concurrency_mismatch_cases(
    contract: ContractSelection,
) -> bool {
    rejects_record_concurrency(
        contract,
        ConcurrencySafety::new(
            ThreadTransfer::ThreadAffine,
            SharingModel::Immutable,
        ),
    ) && rejects_record_concurrency(
        contract,
        ConcurrencySafety::new(
            ThreadTransfer::Transferable,
            SharingModel::Exclusive,
        ),
    ) && rejects_record_concurrency(
        contract,
        ConcurrencySafety::new(
            ThreadTransfer::Transferable,
            SharingModel::Synchronized,
        ),
    )
}

const fn rejects_record_concurrency(
    contract: ContractSelection,
    concurrency: ConcurrencySafety,
) -> bool {
    let Some(versions) = make_schema_versions(1, 2) else {
        return false;
    };
    let Some(safety) = make_safety(
        ContractId::new(RECORD_TYPE),
        versions,
        exact_record_memory(),
        concurrency,
        exact_record_invocation(),
        UnsafeBoundary::None,
    ) else {
        return false;
    };
    rejects_record_contract(contract, record_contract(safety))
}

const fn record_invocation_mismatch_cases(contract: ContractSelection) -> bool {
    rejects_record_invocation(
        contract,
        InvocationSafety::new(
            InvocationModel::NoUserCallback,
            UnwindModel::Contained,
        ),
    ) && rejects_record_invocation(
        contract,
        InvocationSafety::new(
            InvocationModel::NoUserCallback,
            UnwindModel::ForbiddenAcrossBoundary,
        ),
    ) && rejects_record_invocation(
        contract,
        InvocationSafety::new(
            InvocationModel::UserCallback,
            UnwindModel::Contained,
        ),
    ) && rejects_record_invocation(
        contract,
        InvocationSafety::new(
            InvocationModel::UserCallback,
            UnwindModel::ForbiddenAcrossBoundary,
        ),
    )
}

const fn rejects_record_invocation(
    contract: ContractSelection,
    invocation: InvocationSafety,
) -> bool {
    let Some(versions) = make_schema_versions(1, 2) else {
        return false;
    };
    let Some(safety) = make_safety(
        ContractId::new(RECORD_TYPE),
        versions,
        exact_record_memory(),
        exact_record_concurrency(),
        invocation,
        UnsafeBoundary::None,
    ) else {
        return false;
    };
    rejects_record_contract(contract, record_contract(safety))
}

const fn record_boundary_mismatch_case(contract: ContractSelection) -> bool {
    let Some(versions) = make_schema_versions(1, 2) else {
        return false;
    };
    let Some(safety) = make_safety(
        ContractId::new(RECORD_TYPE),
        versions,
        exact_record_memory(),
        exact_record_concurrency(),
        exact_record_invocation(),
        UnsafeBoundary::Reviewed(ContractId::new(0x91)),
    ) else {
        return false;
    };
    rejects_record_contract(contract, record_contract(safety))
}

const fn singleton_mismatch_axis_case() -> bool {
    type_mismatch_code(TypeMismatch::Reject) == 0
}

const fn derive_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputCompletionBounds,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputLimits,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownPolicy,
    >() && READINESS_DERIVE_WITNESS
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticOutputOperation,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticRecordContract,
        >()
        && crate::clone_debug_eq_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticOutputError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticRecordError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticDropCause,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticClosedCause,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionStatus,
        >()
        && move_only_witnesses()
        && facade_derive_cases()
        && diagnostic_reexport_derives()
}

const fn move_only_witnesses() -> bool {
    crate::send_sync_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputRecord,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticRecordRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticSubmissionOutcome,
    >()
}

const fn facade_derive_cases() -> bool {
    contract_identity_derives() && bound_derives() && carrier_derives()
}

const fn contract_identity_derives() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::contract::ContractId>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::contract::ContractVersion,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::contract::VersionRangeError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::contract::CompatibleVersionRange,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::identity::InstanceId,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::identity::IncarnationId,
        >()
}

const fn bound_derives() -> bool {
    let error =
        crate::derive_witness::<::reovim_kernel_interface::bound::BoundError>();
    let unit =
        crate::derive_witness::<::reovim_kernel_interface::bound::BoundUnit>();
    let value =
        crate::derive_witness::<::reovim_kernel_interface::bound::BoundValue>();
    let checked = crate::derive_witness::<
        ::reovim_kernel_interface::bound::CheckedCount,
    >();
    let zero =
        crate::derive_witness::<::reovim_kernel_interface::bound::ZeroPolicy>();
    crate::derive_witness::<::reovim_kernel_interface::bound::ArithmeticRule>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::bound::BoundDescriptor,
        >()
        && error
        && unit
        && value
        && crate::derive_witness::<
            ::reovim_kernel_interface::bound::CheckedByteCount,
        >()
        && checked
        && crate::derive_witness::<
            ::reovim_kernel_interface::bound::CheckedDuration,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::bound::ValidationOrder,
        >()
        && zero
}

const fn carrier_derives() -> bool {
    let copying = crate::derive_witness::<
        ::reovim_kernel_interface::carrier::CopyModel,
    >();
    let dropping = crate::derive_witness::<
        ::reovim_kernel_interface::carrier::DropModel,
    >();
    crate::derive_witness::<::reovim_kernel_interface::carrier::BorrowModel>()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::CarrierDescriptorError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::CarrierSafetyDescriptor,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::ConcurrencySafety,
        >()
        && copying
        && dropping
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::InvocationModel,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::InvocationSafety,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::LifetimeModel,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::MemoryCoherenceError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::MemorySafety,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::OwnershipModel,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::SharingModel,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::ThreadTransfer,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::TypeMismatch,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::UnsafeBoundary,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::carrier::UnwindModel,
        >()
}

const fn diagnostic_reexport_derives() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::BoundDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::BoundError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::CheckedByteCount,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::CheckedCount,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::CheckedDuration,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::CarrierSafetyDescriptor,
    >()
}

pub fn exercise() -> bool {
    preparation_faults()
        && bind_success()
        && bind_rejections()
        && accepted_case()
        && all_dropped_cases()
        && all_closed_cases()
        && error_and_debug_cases()
}

fn make_record(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticOutputRecord> {
    let record = event::make_ready_record()?;
    let source = record.canonical_ref();
    let size = descriptor.validate_record(source).ok()?;
    if size.value() != source.size().value() {
        return None;
    }
    let bytes = event::READY_LINE.into();
    DiagnosticOutputRecord::bind(descriptor, source, bytes).ok()
}

fn preparation_faults() -> bool {
    source_stream_mismatches() && oversized_record()
}

fn source_stream_mismatches() -> bool {
    let instance_only = LifecycleIdentity::new(
        event::STREAM_INSTANCE + 1,
        event::STREAM_INCARNATION,
        Generation::new(3),
    );
    let incarnation_only = LifecycleIdentity::new(
        event::STREAM_INSTANCE,
        event::STREAM_INCARNATION + 1,
        Generation::new(3),
    );
    let generation_only = LifecycleIdentity::new(
        event::STREAM_INSTANCE,
        event::STREAM_INCARNATION,
        Generation::new(99),
    );
    source_stream_mismatch(instance_only)
        && source_stream_mismatch(incarnation_only)
        && source_stream_mismatch(generation_only)
}

fn source_stream_mismatch(expected_stream: LifecycleIdentity) -> bool {
    let Some(descriptor) =
        make_descriptor_for(output_identity(), expected_stream, 1_024)
    else {
        return false;
    };
    let Some(record) = event::make_ready_record() else {
        return false;
    };
    let source = record.canonical_ref();
    let Err(expected) = descriptor.validate_record(source) else {
        return false;
    };
    if diagnostic_record_error_code(expected) != 1 {
        return false;
    }
    returned_bind_rejection(
        &descriptor,
        source,
        event::READY_LINE,
        None,
        expected,
    )
}

fn oversized_record() -> bool {
    let Ok(requested) = u64::try_from(event::READY_LINE.len()) else {
        return false;
    };
    let Some(maximum) = requested.checked_sub(1) else {
        return false;
    };
    let Some(descriptor) = make_descriptor_for(
        output_identity(),
        event::stream_identity(),
        maximum,
    ) else {
        return false;
    };
    let Some(record) = event::make_ready_record() else {
        return false;
    };
    let Err(error) = descriptor.validate_record(record.canonical_ref()) else {
        return false;
    };
    diagnostic_record_exhausted_is(error, requested, maximum)
}

fn bind_success() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = event::make_ready_record() else {
        return false;
    };
    let source = record.canonical_ref();
    let Ok(expected_size) = u64::try_from(event::READY_LINE.len()) else {
        return false;
    };
    let Ok(validated_size) = descriptor.validate_record(source) else {
        return false;
    };
    if validated_size.value() != expected_size {
        return false;
    }
    let bytes = event::READY_LINE.to_vec().into_boxed_slice();
    let pointer = bytes.as_ptr();
    let result = DiagnosticOutputRecord::bind(&descriptor, source, bytes);
    let Ok(record) = result else {
        return false;
    };
    require_debug(&record);
    expected_size == record.size().value()
        && record_matches(&record, pointer)
        && debug_omits_payload(&record)
}

fn bind_rejections() -> bool {
    bind_bound_exceeded() && bind_length_mismatch() && bind_content_mismatch()
}

fn bind_length_mismatch() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = event::make_ready_record() else {
        return false;
    };
    let source = record.canonical_ref();
    let Ok(validated_size) = descriptor.validate_record(source) else {
        return false;
    };
    if validated_size.value() != source.size().value() {
        return false;
    }
    returned_bind_rejection(
        &descriptor,
        source,
        &event::READY_LINE[..event::READY_LINE.len() - 1],
        None,
        DiagnosticRecordError::LengthMismatch,
    )
}

fn bind_bound_exceeded() -> bool {
    let Ok(maximum) = u64::try_from(event::READY_LINE.len() - 1) else {
        return false;
    };
    let Some(descriptor) = make_descriptor_for(
        output_identity(),
        event::stream_identity(),
        maximum,
    ) else {
        return false;
    };
    let Some(record) = event::make_ready_record() else {
        return false;
    };
    let source = record.canonical_ref();
    let Err(expected) = descriptor.validate_record(source) else {
        return false;
    };
    if !diagnostic_record_exhausted_is(expected, source.size().value(), maximum)
    {
        return false;
    }
    returned_bind_rejection(
        &descriptor,
        source,
        event::READY_LINE,
        None,
        expected,
    )
}

fn bind_content_mismatch() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = event::make_ready_record() else {
        return false;
    };
    let source = record.canonical_ref();
    let Ok(validated_size) = descriptor.validate_record(source) else {
        return false;
    };
    if validated_size.value() != source.size().value() {
        return false;
    }
    returned_bind_rejection(
        &descriptor,
        source,
        event::READY_LINE,
        Some(b'R'),
        DiagnosticRecordError::CanonicalMismatch,
    )
}

fn returned_bind_rejection(
    descriptor: &DiagnosticOutputDescriptor,
    source: CanonicalEventRecordRef<'_>,
    source_bytes: &[u8],
    replacement: Option<u8>,
    expected: DiagnosticRecordError,
) -> bool {
    if source_bytes.is_empty() {
        return false;
    }
    let mut bytes = source_bytes.to_vec().into_boxed_slice();
    if let Some(value) = replacement {
        bytes[0] = value;
    }
    let pointer = bytes.as_ptr();
    let result = DiagnosticOutputRecord::bind(descriptor, source, bytes);
    let Err(rejection) = result else {
        return false;
    };
    let borrowed_matches = rejection.cause() == expected
        && rejection_bytes_match(rejection.bytes(), source_bytes, replacement);
    require_debug(&rejection);
    let debug_matches = debug_omits_payload(&rejection);
    let returned = rejection.into_bytes();
    borrowed_matches
        && debug_matches
        && returned.as_ptr() == pointer
        && rejection_bytes_match(&returned, source_bytes, replacement)
}

fn rejection_bytes_match(
    bytes: &[u8],
    source_bytes: &[u8],
    replacement: Option<u8>,
) -> bool {
    let Some(expected_first) = replacement else {
        return bytes == source_bytes;
    };
    replaced_first_matches(bytes, source_bytes, expected_first)
}

fn replaced_first_matches(
    bytes: &[u8],
    source_bytes: &[u8],
    expected_first: u8,
) -> bool {
    let Some((actual_first, actual_rest)) = bytes.split_first() else {
        return false;
    };
    let Some((_, expected_rest)) = source_bytes.split_first() else {
        return false;
    };
    *actual_first == expected_first && actual_rest == expected_rest
}

const fn diagnostic_record_error_code(error: DiagnosticRecordError) -> u8 {
    match error {
        DiagnosticRecordError::Bound(_) => 0,
        DiagnosticRecordError::SourceStreamMismatch => 1,
        DiagnosticRecordError::LengthOverflow => 2,
        DiagnosticRecordError::LengthMismatch => 3,
        DiagnosticRecordError::CanonicalMismatch => 4,
    }
}

const fn diagnostic_record_exhausted_is(
    error: DiagnosticRecordError,
    requested: u64,
    maximum: u64,
) -> bool {
    match error {
        DiagnosticRecordError::Bound(bound) => {
            exhausted_bound_is(bound, 1, requested, maximum)
        }
        DiagnosticRecordError::SourceStreamMismatch
        | DiagnosticRecordError::LengthOverflow
        | DiagnosticRecordError::LengthMismatch
        | DiagnosticRecordError::CanonicalMismatch => false,
    }
}

#[derive(Clone, Copy)]
enum ScriptedDisposition {
    Accepted,
    Dropped(DiagnosticDropCause),
    Closed(DiagnosticClosedCause),
}

// This private double proves public outcome and ownership shapes only. Its
// dispositions neither detect causes nor prove atomic adapter conformance.
struct ScriptedOutputDouble {
    descriptor: DiagnosticOutputDescriptor,
    disposition: ScriptedDisposition,
}

impl ::reovim_kernel_interface::diagnostic::DiagnosticOutput
    for ScriptedOutputDouble
{
    fn descriptor(&self) -> &DiagnosticOutputDescriptor {
        &self.descriptor
    }

    fn submit(
        &self,
        record: DiagnosticOutputRecord,
    ) -> DiagnosticSubmissionOutcome {
        match self.disposition {
            ScriptedDisposition::Accepted => {
                DiagnosticSubmissionOutcome::accepted()
            }
            ScriptedDisposition::Dropped(cause) => {
                DiagnosticSubmissionOutcome::dropped(cause, record)
            }
            ScriptedDisposition::Closed(cause) => {
                DiagnosticSubmissionOutcome::closed(cause, record)
            }
        }
    }
}

fn accepted_case() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let expected = descriptor.clone();
    let output = ScriptedOutputDouble {
        descriptor,
        disposition: ScriptedDisposition::Accepted,
    };
    let outcome = output.submit(record);
    output.descriptor() == &expected
        && outcome.status() == DiagnosticSubmissionStatus::Accepted
        && outcome.returned_record().is_none()
        && outcome.into_returned_record().is_none()
}

fn all_dropped_cases() -> bool {
    let causes = [
        DiagnosticDropCause::InvalidRecord,
        DiagnosticDropCause::BoundExceeded,
        DiagnosticDropCause::NotReady,
        DiagnosticDropCause::SynchronizationPressure,
        DiagnosticDropCause::RecordCapacity,
        DiagnosticDropCause::ByteCapacity,
        DiagnosticDropCause::ChargeOverflow,
        DiagnosticDropCause::DestinationUnavailable,
        DiagnosticDropCause::HealthUnavailable,
        DiagnosticDropCause::InternalFailure,
    ];
    causes.into_iter().all(dropped_case)
}

fn dropped_case(cause: DiagnosticDropCause) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let pointer = record.bytes().as_ptr();
    let output = ScriptedOutputDouble {
        descriptor,
        disposition: ScriptedDisposition::Dropped(cause),
    };
    let outcome = output.submit(record);
    let correct_cause = dropped_outcome_has_cause(&outcome, cause);
    recovered_outcome(outcome, DiagnosticSubmissionStatus::Dropped, pointer)
        && correct_cause
}

const fn dropped_outcome_has_cause(
    outcome: &DiagnosticSubmissionOutcome,
    expected: DiagnosticDropCause,
) -> bool {
    match outcome {
        DiagnosticSubmissionOutcome::Dropped { cause, .. } => {
            drop_cause_code(*cause) == drop_cause_code(expected)
        }
        DiagnosticSubmissionOutcome::Accepted
        | DiagnosticSubmissionOutcome::Closed { .. } => false,
    }
}

const fn drop_cause_code(cause: DiagnosticDropCause) -> u8 {
    match cause {
        DiagnosticDropCause::InvalidRecord => 0,
        DiagnosticDropCause::BoundExceeded => 1,
        DiagnosticDropCause::NotReady => 2,
        DiagnosticDropCause::SynchronizationPressure => 3,
        DiagnosticDropCause::RecordCapacity => 4,
        DiagnosticDropCause::ByteCapacity => 5,
        DiagnosticDropCause::ChargeOverflow => 6,
        DiagnosticDropCause::DestinationUnavailable => 7,
        DiagnosticDropCause::HealthUnavailable => 8,
        DiagnosticDropCause::InternalFailure => 9,
    }
}

fn all_closed_cases() -> bool {
    let causes = [
        DiagnosticClosedCause::Closing,
        DiagnosticClosedCause::Closed,
        DiagnosticClosedCause::Replaced,
        DiagnosticClosedCause::IncompleteShutdown,
    ];
    causes.into_iter().all(closed_case)
}

fn closed_case(cause: DiagnosticClosedCause) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let pointer = record.bytes().as_ptr();
    let output = ScriptedOutputDouble {
        descriptor,
        disposition: ScriptedDisposition::Closed(cause),
    };
    let outcome = output.submit(record);
    let correct_cause = closed_outcome_has_cause(&outcome, cause);
    recovered_outcome(outcome, DiagnosticSubmissionStatus::Closed, pointer)
        && correct_cause
}

const fn closed_outcome_has_cause(
    outcome: &DiagnosticSubmissionOutcome,
    expected: DiagnosticClosedCause,
) -> bool {
    match outcome {
        DiagnosticSubmissionOutcome::Closed { cause, .. } => {
            closed_cause_code(*cause) == closed_cause_code(expected)
        }
        DiagnosticSubmissionOutcome::Accepted
        | DiagnosticSubmissionOutcome::Dropped { .. } => false,
    }
}

const fn closed_cause_code(cause: DiagnosticClosedCause) -> u8 {
    match cause {
        DiagnosticClosedCause::Closing => 0,
        DiagnosticClosedCause::Closed => 1,
        DiagnosticClosedCause::Replaced => 2,
        DiagnosticClosedCause::IncompleteShutdown => 3,
    }
}

fn recovered_outcome(
    outcome: DiagnosticSubmissionOutcome,
    status: DiagnosticSubmissionStatus,
    pointer: *const u8,
) -> bool {
    let actual_status = outcome.status();
    let borrowed = outcome.returned_record();
    let borrowed_matches =
        borrowed.is_some_and(|record| record_matches(record, pointer));
    require_debug(&outcome);
    let debug_matches = debug_omits_payload(&outcome);
    let Some(record) = outcome.into_returned_record() else {
        return false;
    };
    borrowed_matches
        && debug_matches
        && actual_status == status
        && record_matches(&record, pointer)
}

fn record_matches(record: &DiagnosticOutputRecord, pointer: *const u8) -> bool {
    let Ok(size) = u64::try_from(event::READY_LINE.len()) else {
        return false;
    };
    record.output().same_generation(output_identity())
        && record.stream().same_generation(event::stream_identity())
        && record.sequence().value() == 1
        && record.size().value() == size
        && record.bytes() == event::READY_LINE
        && record.bytes().as_ptr() == pointer
}

fn debug_omits_payload<T: Debug>(value: &T) -> bool {
    let debug = format!("{value:?}");
    !debug.contains("reovim-event") && !debug.contains("bytes")
}

fn error_and_debug_cases() -> bool {
    error_variants() && move_only_debug_values()
}

fn error_variants() -> bool {
    let Err(bound) = DiagnosticOutputCompletionBounds::new(0, 1) else {
        return false;
    };
    let output_errors = [
        DiagnosticOutputError::Bound(bound),
        DiagnosticOutputError::LimitsLifecycleMismatch,
        DiagnosticOutputError::IncoherentCarrier,
        DiagnosticOutputError::UnsupportedContract,
        DiagnosticOutputError::IncompatibleContract,
    ];
    let record_errors = [
        DiagnosticRecordError::Bound(bound),
        DiagnosticRecordError::SourceStreamMismatch,
        DiagnosticRecordError::LengthOverflow,
        DiagnosticRecordError::LengthMismatch,
        DiagnosticRecordError::CanonicalMismatch,
    ];
    output_errors.into_iter().all(|error| traits(error) && error_traits(&error))
        && record_errors
            .into_iter()
            .all(|error| traits(error) && error_traits(&error))
}

fn move_only_debug_values() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    require_debug(&record);
    if !debug_omits_payload(&record) {
        return false;
    }
    let outcome = DiagnosticSubmissionOutcome::dropped(
        DiagnosticDropCause::InternalFailure,
        record,
    );
    require_debug(&outcome);
    if !debug_omits_payload(&outcome) {
        return false;
    }
    let Some(record) = outcome.into_returned_record() else {
        return false;
    };
    record.bytes() == event::READY_LINE
}

const fn shutdown_policy_code(value: DiagnosticShutdownPolicy) -> u8 {
    match value {
        DiagnosticShutdownPolicy::Drain => 0,
        DiagnosticShutdownPolicy::Discard => 1,
    }
}

const fn readiness_code(value: DiagnosticOutputReadinessPredicate) -> u8 {
    match value {
        DiagnosticOutputReadinessPredicate::CurrentGenerationAdmission => 0,
    }
}

const _: fn(CarrierSafetyDescriptor) = require_carrier_safety;
const _: fn(CheckedByteCount) = require_checked_bytes;
const _: fn(CheckedCount) = require_checked_count;
const _: fn(CheckedDuration) = require_checked_duration;

const fn require_carrier_safety(_value: CarrierSafetyDescriptor) {}

const fn require_checked_bytes(_value: CheckedByteCount) {}

const fn require_checked_count(_value: CheckedCount) {}

const fn require_checked_duration(_value: CheckedDuration) {}
