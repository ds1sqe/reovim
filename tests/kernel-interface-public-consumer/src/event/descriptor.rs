use reovim_kernel_interface::carrier::{
    BorrowModel, CarrierSafetyDescriptor, ConcurrencySafety, CopyModel,
    DropModel, InvocationModel, InvocationSafety, LifetimeModel, MemorySafety,
    OwnershipModel, SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary,
    UnwindModel,
};
use reovim_kernel_interface::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};
use reovim_kernel_interface::event::{
    Actionability, BootstrapHealth, EventError, EventRejectCause,
    RemediationCode, Severity, StartupComponentCounts, StartupEventDescriptor,
    StartupEventIdentities, StartupEventLimits, StartupProposal,
};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, ReadinessOutcome, RollbackOutcome,
};
use reovim_kernel_interface::operation::{
    ContractSelection, EffectClass, InvocationClass,
};

use super::{
    CANONICAL_REFERENCE_TYPE, EVENT_CONTRACT, REMEDIATION_CATALOG,
    STARTUP_CANONICAL_MAX_BYTES, STARTUP_FAMILY, STREAM_INSTANCE,
    lifecycle_owner, make_canonical_reference_safety, make_clock_descriptor,
    make_descriptor, make_identities, make_limits, stream_identity,
};

pub const fn const_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    identity_cases(descriptor.identities())
        && limit_cases(descriptor.limits())
        && operation_cases(&descriptor)
        && descriptor_error_cases(&descriptor)
        && proposal_cases(&descriptor)
}

const fn identity_cases(identities: StartupEventIdentities) -> bool {
    let stream = stream_identity();
    let next_generation = LifecycleIdentity::new(
        stream.instance(),
        stream.incarnation(),
        Generation::new(stream.generation().value() + 1),
    );
    identities.contract() == EVENT_CONTRACT
        && identities.family() == STARTUP_FAMILY
        && identities.remediation_catalog() == REMEDIATION_CATALOG
        && identities.canonical_reference() == CANONICAL_REFERENCE_TYPE
        && identities.stream().same_generation(stream)
        && identities.stream().same_incarnation(next_generation)
        && !identities.stream().same_generation(next_generation)
        && identities.lifecycle_owner().same_generation(lifecycle_owner())
}

const fn limit_cases(limits: &StartupEventLimits) -> bool {
    let counts = limits.component_counts();
    let work = limits.synchronous_work();
    let temporary = limits.temporary_bytes();
    let canonical = limits.canonical_bytes();
    let ring = limits.ring_bytes();

    counts.per_object().value() == 8
        && counts.aggregate().value() == 16
        && counts.charge_owner().value() == STREAM_INSTANCE
        && counts.admit_per_object(counts.per_object()).is_ok()
        && counts
            .admit_aggregate(counts.per_object(), counts.per_object())
            .is_ok()
        && work.per_object().value() == 32
        && temporary.per_object().value() == 2_048
        && canonical.per_object().value() == STARTUP_CANONICAL_MAX_BYTES
        && ring.per_object().value() == super::EVENT_RING_MAX_BYTES
        && limits.snapshot_records().value() == 4
        && limits.snapshot_bytes().value() == 2_048
}

const fn operation_cases(descriptor: &StartupEventDescriptor) -> bool {
    let admission = descriptor.admission_operation();
    let snapshot = descriptor.snapshot_operation();

    descriptor.contract().identity() == EVENT_CONTRACT
        && descriptor.clock().maximum_elapsed().nanoseconds() == 1_000
        && descriptor.family_schema_version() == 1
        && descriptor.remediation_catalog_version() == 1
        && descriptor.canonical_reference_safety().type_id().value()
            == CANONICAL_REFERENCE_TYPE
        && canonical_reference_safety_cases(descriptor)
        && invocation_code(admission.invocation()) == 0
        && effect_code(admission.effect()) == 1
        && invocation_code(snapshot.invocation()) == 0
        && effect_code(snapshot.effect()) == 0
        && descriptor.negotiate(EVENT_CONTRACT, 1).is_ok()
        && descriptor.negotiate(EVENT_CONTRACT, 2).is_ok()
}

const fn canonical_reference_safety_cases(
    descriptor: &StartupEventDescriptor,
) -> bool {
    let safety = descriptor.canonical_reference_safety();
    let memory = safety.memory();
    let concurrency = safety.concurrency();
    let invocation = safety.invocation();
    safety.schema_versions().minimum().value() == 1
        && safety.schema_versions().maximum().value() == 2
        && ownership_code(memory.ownership()) == 1
        && borrow_code(memory.borrowing()) == 1
        && copy_code(memory.copying()) == 1
        && drop_code(memory.dropping()) == 0
        && lifetime_code(memory.lifetime()) == 2
        && transfer_code(concurrency.thread_transfer()) == 0
        && sharing_code(concurrency.sharing()) == 1
        && carrier_invocation_code(invocation.invocation()) == 0
        && unwind_code(invocation.unwind()) == 0
        && mismatch_code(safety.mismatch()) == 0
        && unsafe_boundary_code(safety.unsafe_boundary()) == 0
}

const fn ownership_code(value: OwnershipModel) -> u8 {
    match value {
        OwnershipModel::Owned => 0,
        OwnershipModel::Borrowed => 1,
    }
}

const fn borrow_code(value: BorrowModel) -> u8 {
    match value {
        BorrowModel::Forbidden => 0,
        BorrowModel::Shared => 1,
        BorrowModel::Exclusive => 2,
    }
}

const fn copy_code(value: CopyModel) -> u8 {
    match value {
        CopyModel::MoveOnly => 0,
        CopyModel::Copy => 1,
    }
}

const fn drop_code(value: DropModel) -> u8 {
    match value {
        DropModel::Trivial => 0,
        DropModel::ReleasesOwnedState => 1,
    }
}

const fn lifetime_code(value: LifetimeModel) -> u8 {
    match value {
        LifetimeModel::Value => 0,
        LifetimeModel::InstanceBound => 1,
        LifetimeModel::IncarnationBound => 2,
    }
}

const fn transfer_code(value: ThreadTransfer) -> u8 {
    match value {
        ThreadTransfer::Transferable => 0,
        ThreadTransfer::ThreadAffine => 1,
    }
}

const fn sharing_code(value: SharingModel) -> u8 {
    match value {
        SharingModel::Exclusive => 0,
        SharingModel::Immutable => 1,
        SharingModel::Synchronized => 2,
    }
}

const fn carrier_invocation_code(value: InvocationModel) -> u8 {
    match value {
        InvocationModel::DataOnly => 0,
        InvocationModel::NoUserCallback => 1,
        InvocationModel::UserCallback => 2,
    }
}

const fn unwind_code(value: UnwindModel) -> u8 {
    match value {
        UnwindModel::NotInvocable => 0,
        UnwindModel::Contained => 1,
        UnwindModel::ForbiddenAcrossBoundary => 2,
    }
}

const fn mismatch_code(value: TypeMismatch) -> u8 {
    let TypeMismatch::Reject = value;
    0
}

const fn unsafe_boundary_code(value: UnsafeBoundary) -> u8 {
    match value {
        UnsafeBoundary::None => 0,
        UnsafeBoundary::Reviewed(_) => 1,
    }
}

const fn invocation_code(value: InvocationClass) -> u8 {
    match value {
        InvocationClass::NonBlockingInvocation => 0,
        InvocationClass::CallerMayWait => 1,
    }
}

const fn effect_code(value: EffectClass) -> u8 {
    match value {
        EffectClass::LocalOnly => 0,
        EffectClass::ExternalObservation => 1,
        EffectClass::ExternalMutation => 2,
    }
}

const fn descriptor_error_cases(descriptor: &StartupEventDescriptor) -> bool {
    unit_error_code(descriptor.negotiate(0xff, 1)) == 4
        && unit_error_code(descriptor.negotiate(EVENT_CONTRACT, 3)) == 5
        && mismatched_contract_code() == 3
        && mismatched_owner_code() == 2
        && canonical_reference_negative_cases(descriptor)
        && invalid_limit_code() == 1
}

const fn mismatched_contract_code() -> u8 {
    let Ok(contract) = ContractSelection::new(EVENT_CONTRACT, 1, 1, 2) else {
        return 0;
    };
    let Some(clock) = make_clock_descriptor() else {
        return 0;
    };
    let Some(limits) = make_limits() else { return 0 };
    let identities = StartupEventIdentities::new(
        0xff,
        STARTUP_FAMILY,
        REMEDIATION_CATALOG,
        CANONICAL_REFERENCE_TYPE,
        stream_identity(),
        lifecycle_owner(),
    );
    let Some(safety) = make_canonical_reference_safety(SharingModel::Immutable)
    else {
        return 0;
    };
    descriptor_code(&StartupEventDescriptor::new(
        contract, identities, clock, limits, safety,
    ))
}

const fn mismatched_owner_code() -> u8 {
    let Ok(contract) = ContractSelection::new(EVENT_CONTRACT, 1, 1, 2) else {
        return 0;
    };
    let Some(clock) = make_clock_descriptor() else {
        return 0;
    };
    let Ok(limits) = StartupEventLimits::new(0xff, 8, 16, 32, 2_048, 4, 2_048)
    else {
        return 0;
    };
    let Some(safety) = make_canonical_reference_safety(SharingModel::Immutable)
    else {
        return 0;
    };
    descriptor_code(&StartupEventDescriptor::new(
        contract,
        make_identities(),
        clock,
        limits,
        safety,
    ))
}

const fn canonical_reference_negative_cases(
    descriptor: &StartupEventDescriptor,
) -> bool {
    identity_and_version_negative_cases(descriptor)
        && coherent_memory_negative_cases(descriptor)
        && concurrency_negative_cases(descriptor)
        && coherent_invocation_negative_cases(descriptor)
        && unsafe_boundary_negative_case(descriptor)
        && singleton_mismatch_axis_is_forced(descriptor)
}

const fn identity_and_version_negative_cases(
    descriptor: &StartupEventDescriptor,
) -> bool {
    let Some(exact) = version_range(1, 2) else {
        return false;
    };
    let Some(changed_minimum) = version_range(2, 2) else {
        return false;
    };
    let Some(changed_maximum) = version_range(1, 3) else {
        return false;
    };
    rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE + 1,
        exact,
        exact_memory(),
        exact_concurrency(),
        exact_carrier_invocation(),
        UnsafeBoundary::None,
    ) && rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE,
        changed_minimum,
        exact_memory(),
        exact_concurrency(),
        exact_carrier_invocation(),
        UnsafeBoundary::None,
    ) && rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE,
        changed_maximum,
        exact_memory(),
        exact_concurrency(),
        exact_carrier_invocation(),
        UnsafeBoundary::None,
    )
}

const fn coherent_memory_negative_cases(
    descriptor: &StartupEventDescriptor,
) -> bool {
    rejects_memory(descriptor, owned_reference_memory())
        && rejects_memory(descriptor, forbidden_borrow_memory())
        && rejects_memory(descriptor, exclusive_borrow_memory())
        && rejects_memory(descriptor, move_only_borrow_memory())
        && rejects_memory(descriptor, releasing_owned_memory())
        && rejects_memory(descriptor, value_lifetime_memory())
        && rejects_memory(descriptor, instance_lifetime_memory())
}

const fn owned_reference_memory() -> MemorySafety {
    MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    )
}

const fn forbidden_borrow_memory() -> MemorySafety {
    // Foundation permits forbidden borrowing here only with owned memory.
    MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Forbidden,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    )
}

const fn exclusive_borrow_memory() -> MemorySafety {
    MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Exclusive,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    )
}

const fn move_only_borrow_memory() -> MemorySafety {
    MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    )
}

const fn releasing_owned_memory() -> MemorySafety {
    // Releasing state requires an owned, move-only carrier.
    MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::MoveOnly,
        DropModel::ReleasesOwnedState,
        LifetimeModel::IncarnationBound,
    )
}

const fn value_lifetime_memory() -> MemorySafety {
    // A self-contained value lifetime requires owned memory.
    MemorySafety::new(
        OwnershipModel::Owned,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::Value,
    )
}

const fn instance_lifetime_memory() -> MemorySafety {
    MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::InstanceBound,
    )
}

const fn concurrency_negative_cases(
    descriptor: &StartupEventDescriptor,
) -> bool {
    rejects_concurrency(
        descriptor,
        ConcurrencySafety::new(
            ThreadTransfer::ThreadAffine,
            SharingModel::Immutable,
        ),
    ) && rejects_concurrency(
        descriptor,
        ConcurrencySafety::new(
            ThreadTransfer::Transferable,
            SharingModel::Exclusive,
        ),
    ) && rejects_concurrency(
        descriptor,
        ConcurrencySafety::new(
            ThreadTransfer::Transferable,
            SharingModel::Synchronized,
        ),
    )
}

const fn coherent_invocation_negative_cases(
    descriptor: &StartupEventDescriptor,
) -> bool {
    // Callable carriers require either contained or forbidden unwind.
    rejects_invocation(
        descriptor,
        InvocationModel::NoUserCallback,
        UnwindModel::Contained,
    ) && rejects_invocation(
        descriptor,
        InvocationModel::NoUserCallback,
        UnwindModel::ForbiddenAcrossBoundary,
    ) && rejects_invocation(
        descriptor,
        InvocationModel::UserCallback,
        UnwindModel::Contained,
    ) && rejects_invocation(
        descriptor,
        InvocationModel::UserCallback,
        UnwindModel::ForbiddenAcrossBoundary,
    )
}

const fn unsafe_boundary_negative_case(
    descriptor: &StartupEventDescriptor,
) -> bool {
    let boundary = UnsafeBoundary::Reviewed(ContractId::new(0x61));
    let Some(versions) = version_range(1, 2) else {
        return false;
    };
    rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE,
        versions,
        exact_memory(),
        exact_concurrency(),
        exact_carrier_invocation(),
        boundary,
    )
}

const fn singleton_mismatch_axis_is_forced(
    descriptor: &StartupEventDescriptor,
) -> bool {
    // Reject is the only TypeMismatch value in the public vocabulary.
    mismatch_code(descriptor.canonical_reference_safety().mismatch()) == 0
}

const fn rejects_memory(
    descriptor: &StartupEventDescriptor,
    memory: MemorySafety,
) -> bool {
    let Some(versions) = version_range(1, 2) else {
        return false;
    };
    rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE,
        versions,
        memory,
        exact_concurrency(),
        exact_carrier_invocation(),
        UnsafeBoundary::None,
    )
}

const fn rejects_concurrency(
    descriptor: &StartupEventDescriptor,
    concurrency: ConcurrencySafety,
) -> bool {
    let Some(versions) = version_range(1, 2) else {
        return false;
    };
    rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE,
        versions,
        exact_memory(),
        concurrency,
        exact_carrier_invocation(),
        UnsafeBoundary::None,
    )
}

const fn rejects_invocation(
    descriptor: &StartupEventDescriptor,
    invocation: InvocationModel,
    unwind: UnwindModel,
) -> bool {
    let Some(versions) = version_range(1, 2) else {
        return false;
    };
    let invocation = InvocationSafety::new(invocation, unwind);
    rejects_carrier(
        descriptor,
        CANONICAL_REFERENCE_TYPE,
        versions,
        exact_memory(),
        exact_concurrency(),
        invocation,
        UnsafeBoundary::None,
    )
}

const fn rejects_carrier(
    descriptor: &StartupEventDescriptor,
    type_id: u128,
    versions: CompatibleVersionRange,
    memory: MemorySafety,
    concurrency: ConcurrencySafety,
    invocation: InvocationSafety,
    boundary: UnsafeBoundary,
) -> bool {
    let Ok(safety) = CarrierSafetyDescriptor::new(
        ContractId::new(type_id),
        versions,
        memory,
        concurrency,
        invocation,
        TypeMismatch::Reject,
        boundary,
    ) else {
        return false;
    };
    let Some(limits) = make_limits() else {
        return false;
    };
    let result = StartupEventDescriptor::new(
        descriptor.contract(),
        descriptor.identities(),
        descriptor.clock(),
        limits,
        safety,
    );
    incoherent_carrier_result(&result)
}

const fn incoherent_carrier_result(
    result: &Result<StartupEventDescriptor, EventError>,
) -> bool {
    match result {
        Err(EventError::IncoherentCarrier) => true,
        Err(_) | Ok(_) => false,
    }
}

const fn version_range(
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

const fn exact_memory() -> MemorySafety {
    MemorySafety::new(
        OwnershipModel::Borrowed,
        BorrowModel::Shared,
        CopyModel::Copy,
        DropModel::Trivial,
        LifetimeModel::IncarnationBound,
    )
}

const fn exact_concurrency() -> ConcurrencySafety {
    ConcurrencySafety::new(
        ThreadTransfer::Transferable,
        SharingModel::Immutable,
    )
}

const fn exact_carrier_invocation() -> InvocationSafety {
    InvocationSafety::new(InvocationModel::DataOnly, UnwindModel::NotInvocable)
}

const fn invalid_limit_code() -> u8 {
    match StartupEventLimits::new(STREAM_INSTANCE, 9, 8, 32, 2_048, 4, 2_048) {
        Err(EventError::Bound(_)) => 1,
        Err(_) | Ok(_) => 0,
    }
}

const fn descriptor_code(
    result: &Result<StartupEventDescriptor, EventError>,
) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(*error),
        Ok(_) => 0,
    }
}

const fn unit_error_code(result: Result<(), EventError>) -> u8 {
    match result {
        Err(error) => super::outcomes::event_error_code(error),
        Ok(()) => 0,
    }
}

const fn proposal_cases(descriptor: &StartupEventDescriptor) -> bool {
    let Ok(ready_counts) = StartupComponentCounts::new(3, 0, 0) else {
        return false;
    };
    let Ok(degraded_counts) = StartupComponentCounts::new(2, 1, 0) else {
        return false;
    };
    let Ok(failed_counts) = StartupComponentCounts::new(1, 1, 1) else {
        return false;
    };
    let Ok(ready) = make_proposal(
        descriptor,
        ReadinessOutcome::Ready,
        BootstrapHealth::Clear,
        ready_counts,
    ) else {
        return false;
    };
    let Ok(degraded) = make_proposal(
        descriptor,
        ReadinessOutcome::DiagnosticsDegraded,
        BootstrapHealth::Imported,
        degraded_counts,
    ) else {
        return false;
    };
    let Ok(failed) = make_proposal(
        descriptor,
        ReadinessOutcome::ActivationFailed(RollbackOutcome::Reverted),
        BootstrapHealth::Pending,
        failed_counts,
    ) else {
        return false;
    };

    count_cases(ready_counts, degraded_counts, failed_counts)
        && ready_cases(ready, descriptor)
        && operator_cases(degraded, RemediationCode::DiagnosticPathDegraded)
        && operator_cases(failed, RemediationCode::ActivationFailed)
        && incomplete_rollback_case(descriptor, failed_counts)
        && proposal_error_cases(descriptor, ready_counts, degraded_counts)
}

pub const fn make_proposal(
    descriptor: &StartupEventDescriptor,
    readiness: ReadinessOutcome,
    health: BootstrapHealth,
    counts: StartupComponentCounts,
) -> Result<StartupProposal, EventRejectCause> {
    StartupProposal::new(
        descriptor,
        lifecycle_owner(),
        readiness,
        counts,
        health,
    )
}

const fn count_cases(
    ready: StartupComponentCounts,
    degraded: StartupComponentCounts,
    failed: StartupComponentCounts,
) -> bool {
    ready.active().value() == 3
        && ready.degraded().value() == 0
        && ready.unavailable().value() == 0
        && ready.total().value() == 3
        && degraded.total().value() == 3
        && failed.total().value() == 3
}

const fn ready_cases(
    proposal: StartupProposal,
    descriptor: &StartupEventDescriptor,
) -> bool {
    proposal.lifecycle_owner().same_generation(lifecycle_owner())
        && readiness_code(proposal.readiness()) == 0
        && proposal.counts().total().value() == 3
        && health_code(proposal.bootstrap_health()) == 0
        && severity_code(proposal.severity()) == 0
        && actionability_code(proposal.actionability()) == 0
        && proposal.validate_for(descriptor).is_ok()
}

const fn operator_cases(
    proposal: StartupProposal,
    expected: RemediationCode,
) -> bool {
    let Actionability::Operator(remediation) = proposal.actionability() else {
        return false;
    };
    remediation.catalog() == REMEDIATION_CATALOG
        && remediation.catalog_version() == 1
        && remediation_code(remediation.code()) == remediation_code(expected)
}

const fn incomplete_rollback_case(
    descriptor: &StartupEventDescriptor,
    counts: StartupComponentCounts,
) -> bool {
    let result = make_proposal(
        descriptor,
        ReadinessOutcome::ActivationFailed(RollbackOutcome::IncompleteShutdown),
        BootstrapHealth::Pending,
        counts,
    );
    match result {
        Ok(proposal) => readiness_code(proposal.readiness()) == 2,
        Err(_) => false,
    }
}

const fn proposal_error_cases(
    descriptor: &StartupEventDescriptor,
    ready_counts: StartupComponentCounts,
    degraded_counts: StartupComponentCounts,
) -> bool {
    let owner = lifecycle_owner();
    let stale = LifecycleIdentity::new(
        owner.instance(),
        owner.incarnation(),
        Generation::new(99),
    );
    let owner_error = StartupProposal::new(
        descriptor,
        stale,
        ReadinessOutcome::Ready,
        ready_counts,
        BootstrapHealth::Clear,
    );
    let Ok(large) = StartupComponentCounts::new(9, 0, 0) else {
        return false;
    };
    let bound_error = make_proposal(
        descriptor,
        ReadinessOutcome::Ready,
        BootstrapHealth::Clear,
        large,
    );
    let structure_error = make_proposal(
        descriptor,
        ReadinessOutcome::Ready,
        BootstrapHealth::Clear,
        degraded_counts,
    );
    let empty_degradation = make_proposal(
        descriptor,
        ReadinessOutcome::DiagnosticsDegraded,
        BootstrapHealth::Imported,
        ready_counts,
    );
    let failed_without_unavailable = make_proposal(
        descriptor,
        ReadinessOutcome::ActivationFailed(RollbackOutcome::Reverted),
        BootstrapHealth::Pending,
        degraded_counts,
    );
    proposal_code(owner_error) == 2
        && proposal_code(bound_error) == 4
        && proposal_code(structure_error) == 3
        && proposal_code(empty_degradation) == 3
        && proposal_code(failed_without_unavailable) == 3
        && actionability_error(descriptor, degraded_counts)
}

const fn proposal_code(
    result: Result<StartupProposal, EventRejectCause>,
) -> u8 {
    match result {
        Err(error) => reject_code(error),
        Ok(_) => 0,
    }
}

const fn actionability_error(
    descriptor: &StartupEventDescriptor,
    counts: StartupComponentCounts,
) -> bool {
    let Some(limits) = make_limits() else { return false };
    let identities = StartupEventIdentities::new(
        EVENT_CONTRACT,
        STARTUP_FAMILY,
        REMEDIATION_CATALOG + 1,
        CANONICAL_REFERENCE_TYPE,
        stream_identity(),
        lifecycle_owner(),
    );
    let Some(safety) = make_canonical_reference_safety(SharingModel::Immutable)
    else {
        return false;
    };
    let Ok(altered) = StartupEventDescriptor::new(
        descriptor.contract(),
        identities,
        descriptor.clock(),
        limits,
        safety,
    ) else {
        return false;
    };
    let Ok(proposal) = make_proposal(
        descriptor,
        ReadinessOutcome::DiagnosticsDegraded,
        BootstrapHealth::Imported,
        counts,
    ) else {
        return false;
    };
    match proposal.validate_for(&altered) {
        Err(EventRejectCause::InvalidActionability) => true,
        Err(_) | Ok(()) => false,
    }
}

const fn reject_code(cause: EventRejectCause) -> u8 {
    match cause {
        EventRejectCause::StreamNotReady => 1,
        EventRejectCause::InvalidAuthority => 2,
        EventRejectCause::InvalidStructure => 3,
        EventRejectCause::BoundExceeded => 4,
        EventRejectCause::InvalidActionability => 5,
        EventRejectCause::Clock(_) => 6,
    }
}

pub const fn readiness_code(value: ReadinessOutcome) -> u8 {
    match value {
        ReadinessOutcome::Ready => 0,
        ReadinessOutcome::DiagnosticsDegraded => 1,
        ReadinessOutcome::ActivationFailed(_) => 2,
    }
}

const fn health_code(value: BootstrapHealth) -> u8 {
    match value {
        BootstrapHealth::Clear => 0,
        BootstrapHealth::Imported => 1,
        BootstrapHealth::Pending => 2,
    }
}

const fn severity_code(value: Severity) -> u8 {
    match value {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

const fn actionability_code(value: Actionability) -> u8 {
    match value {
        Actionability::None => 0,
        Actionability::Operator(_) => 1,
    }
}

const fn remediation_code(value: RemediationCode) -> u8 {
    match value {
        RemediationCode::DiagnosticPathDegraded => 0,
        RemediationCode::ActivationFailed => 1,
    }
}
