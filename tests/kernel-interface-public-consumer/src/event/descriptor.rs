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
    EVENT_CONTRACT, REMEDIATION_CATALOG, STARTUP_CANONICAL_MAX_BYTES,
    STARTUP_FAMILY, STREAM_INSTANCE, lifecycle_owner, make_clock_descriptor,
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
    identities.contract() == EVENT_CONTRACT
        && identities.family() == STARTUP_FAMILY
        && identities.remediation_catalog() == REMEDIATION_CATALOG
        && identities.stream().same_generation(stream_identity())
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
        && invocation_code(admission.invocation()) == 0
        && effect_code(admission.effect()) == 1
        && invocation_code(snapshot.invocation()) == 0
        && effect_code(snapshot.effect()) == 0
        && descriptor.negotiate(EVENT_CONTRACT, 1).is_ok()
        && descriptor.negotiate(EVENT_CONTRACT, 2).is_ok()
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
        stream_identity(),
        lifecycle_owner(),
    );
    descriptor_code(&StartupEventDescriptor::new(
        contract, identities, clock, limits,
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
    descriptor_code(&StartupEventDescriptor::new(
        contract,
        make_identities(),
        clock,
        limits,
    ))
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
        stream_identity(),
        lifecycle_owner(),
    );
    let Ok(altered) = StartupEventDescriptor::new(
        descriptor.contract(),
        identities,
        descriptor.clock(),
        limits,
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
