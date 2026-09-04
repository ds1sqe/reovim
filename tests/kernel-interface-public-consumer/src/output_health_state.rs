//! State-facing `OutputHealth` values through the public contract.

mod counters;
mod snapshot;

use alloc::string::ToString;
use reovim_kernel_interface::diagnostic::DiagnosticOutputDescriptor;
use reovim_kernel_interface::lifecycle::{Generation, LifecycleIdentity};
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::output_health::{
    OutputHealthAcceptance, OutputHealthAnomalyRecord, OutputHealthDescriptor,
    OutputHealthError, OutputHealthHighWater, OutputHealthImportCounterKind,
    OutputHealthLease, OutputHealthLeaseScope, OutputHealthLimits,
    OutputHealthProposal, OutputHealthProposalScope, OutputHealthRecordOutcome,
    OutputHealthRecordResultKind, OutputHealthRetainedRange,
    OutputHealthSequence, OutputHealthSequencePosition,
    OutputHealthSnapshotLimits,
};

use crate::{error_traits, traits};

const HEALTH_CONTRACT: u128 = 0x91;
const FIXED_BYTES: u64 = 512;
const ANOMALY_BYTES: u64 = 128;
const RETAINED_RECORDS: u64 = 4;
const SNAPSHOT_BYTES: u64 = ANOMALY_BYTES * RETAINED_RECORDS;

const _: fn(
    ContractSelection,
    LifecycleIdentity,
    LifecycleIdentity,
    OutputHealthLimits,
) -> Result<OutputHealthDescriptor, OutputHealthError> =
    OutputHealthDescriptor::new;
const _: fn(
    &OutputHealthDescriptor,
    &DiagnosticOutputDescriptor,
) -> Result<OutputHealthLeaseScope, OutputHealthError> =
    OutputHealthLeaseScope::new;
const _: fn(
    &OutputHealthDescriptor,
    OutputHealthSequence,
    OutputHealthProposal,
) -> Result<OutputHealthAnomalyRecord, OutputHealthError> =
    OutputHealthAnomalyRecord::new;

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthLimits,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthSnapshotLimits,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthSequence,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthSequencePosition,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthHighWater,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthRetainedRange,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthLeaseScope,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthProposalScope,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthAcceptance,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthRecordOutcome,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthAnomalyRecord,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthAcceptedCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthDropCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthClosedCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthEffectCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthEvictionCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthRecordResultCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthImportCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthCounters,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthRecordResultKind,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthImportCounterKind,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthSnapshotRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthSnapshotStatistics,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthSnapshotCoverage,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::
            OutputHealthSnapshotMetadata,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthSnapshot<'static>,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::output_health::OutputHealthError,
    >()
        && proposal_scope_variants()
        && sequence_position_variants()
        && acceptance_variants()
        && outcome_variants()
        && record_result_variants()
        && import_kind_variants()
}

pub fn exercise() -> bool {
    descriptor_and_limits()
        && limit_errors()
        && sequence_and_range()
        && scope_classification()
        && scripted_lease()
        && anomaly_validation()
        && error_traits_case()
        && counters::exercise()
        && snapshot::exercise()
}

pub fn make_health()
-> Option<(OutputHealthDescriptor, DiagnosticOutputDescriptor)> {
    let output = crate::diagnostic::make_descriptor()?;
    let stream = output.source_stream();
    let snapshot =
        OutputHealthSnapshotLimits::new(RETAINED_RECORDS, SNAPSHOT_BYTES)
            .ok()?;
    let limits = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        ANOMALY_BYTES,
        RETAINED_RECORDS,
        snapshot,
        3,
    )
    .ok()?;
    let contract = ContractSelection::new(HEALTH_CONTRACT, 1, 1, 2).ok()?;
    let descriptor =
        OutputHealthDescriptor::new(contract, stream, output.owner(), limits)
            .ok()?;
    Some((descriptor, output))
}

pub fn make_anomaly(
    descriptor: &OutputHealthDescriptor,
    output: &DiagnosticOutputDescriptor,
    sequence: u64,
) -> Option<OutputHealthAnomalyRecord> {
    let sequence = OutputHealthSequence::new(sequence).ok()?;
    let proposal = OutputHealthProposal::activation_failure(
        output,
        reovim_kernel_interface::output_health::
            OutputHealthActivationFailureCause::InternalFailure,
        reovim_kernel_interface::lifecycle::RollbackOutcome::Reverted,
    );
    OutputHealthAnomalyRecord::new(descriptor, sequence, proposal).ok()
}

fn descriptor_and_limits() -> bool {
    let Some((descriptor, output)) = make_health() else {
        return false;
    };
    let limits = descriptor.limits();
    descriptor.contract().identity() == HEALTH_CONTRACT
        && descriptor.contract().selected() == 1
        && descriptor.stream().same_generation(output.source_stream())
        && descriptor.lifecycle_owner().same_generation(output.owner())
        && limits.stream().same_generation(descriptor.stream())
        && limits.record_work().per_object().value() == 8
        && limits.temporary_bytes().aggregate().value() == 0
        && limits.fixed_resident_bytes().aggregate().value() == FIXED_BYTES
        && limits.anomaly_records().aggregate().value() == RETAINED_RECORDS
        && limits.anomaly_bytes().per_object().value() == ANOMALY_BYTES
        && limits.anomaly_bytes().aggregate().value() == SNAPSHOT_BYTES
        && limits.resident_bytes().aggregate().value()
            == FIXED_BYTES + SNAPSHOT_BYTES
        && limits.snapshot_records().value() == RETAINED_RECORDS
        && limits.snapshot_bytes().value() == SNAPSHOT_BYTES
        && limits.snapshot().records().value() == RETAINED_RECORDS
        && limits.import_attempts().value() == 3
        && descriptor.record_operation() == descriptor.snapshot_operation()
        && descriptor.negotiate(HEALTH_CONTRACT, 1).is_ok()
        && output_error_is(
            &descriptor.negotiate(HEALTH_CONTRACT + 1, 1),
            OutputHealthError::UnsupportedContract,
        )
        && output_error_is(
            &descriptor.negotiate(HEALTH_CONTRACT, 3),
            OutputHealthError::IncompatibleContract,
        )
        && traits(descriptor)
}

fn limit_errors() -> bool {
    let Some(output) = crate::diagnostic::make_descriptor() else {
        return false;
    };
    let stream = output.source_stream();
    let Ok(snapshot) = OutputHealthSnapshotLimits::new(4, 512) else {
        return false;
    };
    let zero_work = OutputHealthLimits::new(
        stream,
        0,
        FIXED_BYTES,
        ANOMALY_BYTES,
        4,
        snapshot,
        3,
    );
    let zero_fixed =
        OutputHealthLimits::new(stream, 8, 0, ANOMALY_BYTES, 4, snapshot, 3);
    let zero_anomaly =
        OutputHealthLimits::new(stream, 8, FIXED_BYTES, 0, 4, snapshot, 3);
    let zero_retained = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        ANOMALY_BYTES,
        0,
        snapshot,
        3,
    );
    let zero_attempts = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        ANOMALY_BYTES,
        4,
        snapshot,
        0,
    );
    let Ok(too_many) = OutputHealthSnapshotLimits::new(5, 512) else {
        return false;
    };
    let too_many_records = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        ANOMALY_BYTES,
        4,
        too_many,
        3,
    );
    let Ok(too_small) = OutputHealthSnapshotLimits::new(4, ANOMALY_BYTES - 1)
    else {
        return false;
    };
    let too_few_bytes = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        ANOMALY_BYTES,
        4,
        too_small,
        3,
    );
    let Ok(maximum_snapshot) = OutputHealthSnapshotLimits::new(1, u64::MAX)
    else {
        return false;
    };
    let multiply_overflow = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        u64::MAX,
        2,
        maximum_snapshot,
        3,
    );
    output_bound_error(&zero_work)
        && output_bound_error(&zero_fixed)
        && output_bound_error(&zero_anomaly)
        && output_bound_error(&zero_retained)
        && output_bound_error(&zero_attempts)
        && OutputHealthSnapshotLimits::new(0, 512).is_err()
        && OutputHealthSnapshotLimits::new(4, 0).is_err()
        && output_error_is(
            &too_many_records,
            OutputHealthError::IncoherentSnapshotLimits,
        )
        && output_error_is(
            &too_few_bytes,
            OutputHealthError::IncoherentSnapshotLimits,
        )
        && output_bound_error(&multiply_overflow)
        && descriptor_stream_mismatch(&output)
}

fn descriptor_stream_mismatch(output: &DiagnosticOutputDescriptor) -> bool {
    let stream = output.source_stream();
    let replaced = LifecycleIdentity::new(
        stream.instance(),
        stream.incarnation(),
        Generation::new(stream.generation().value() + 1),
    );
    let Ok(snapshot) = OutputHealthSnapshotLimits::new(4, 512) else {
        return false;
    };
    let Ok(limits) = OutputHealthLimits::new(
        stream,
        8,
        FIXED_BYTES,
        ANOMALY_BYTES,
        4,
        snapshot,
        3,
    ) else {
        return false;
    };
    let Ok(contract) = ContractSelection::new(HEALTH_CONTRACT, 1, 1, 2) else {
        return false;
    };
    output_error_is(
        &OutputHealthDescriptor::new(
            contract,
            replaced,
            output.owner(),
            limits,
        ),
        OutputHealthError::LimitsStreamMismatch,
    )
}

fn sequence_and_range() -> bool {
    let first = OutputHealthSequence::first();
    let Ok(second) = first.next() else {
        return false;
    };
    let Ok(range) = OutputHealthRetainedRange::non_empty(first, second) else {
        return false;
    };
    let maximum = OutputHealthSequence::new(u64::MAX);
    first.value() == 1
        && second.value() == 2
        && first.is_before(second)
        && range.earliest() == Some(first)
        && range.latest() == Some(second)
        && !range.is_empty()
        && OutputHealthRetainedRange::empty().is_empty()
        && output_error_is(
            &OutputHealthRetainedRange::non_empty(second, first),
            OutputHealthError::InvalidRetainedRange,
        )
        && output_error_is(
            &OutputHealthSequence::new(0),
            OutputHealthError::UnassignedSequence,
        )
        && output_error_is(
            &maximum.and_then(OutputHealthSequence::next),
            OutputHealthError::SequenceExhausted,
        )
        && OutputHealthHighWater::empty().latest().is_none()
        && OutputHealthHighWater::assigned(second).latest() == Some(second)
        && next_position_is(
            OutputHealthHighWater::empty().next_position(),
            first,
        )
}

fn scope_classification() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let Ok(scope) = OutputHealthLeaseScope::new(&health, &output) else {
        return false;
    };
    let current = anomaly_proposal(&output);
    let stale_output = changed_output(&output, false, true);
    let invalid_output = changed_output(&output, true, false);
    let invalid_stream = changed_stream(&output);
    let (Some(stale), Some(invalid), Some(foreign)) =
        (stale_output, invalid_output, invalid_stream)
    else {
        return false;
    };
    scope.stream().same_generation(health.stream())
        && scope.output().same_generation(output.lifecycle())
        && scope.lifecycle_owner().same_generation(health.lifecycle_owner())
        && scope.classify(current) == OutputHealthProposalScope::Current
        && scope.classify(anomaly_proposal(&stale))
            == OutputHealthProposalScope::Stale
        && scope.classify(anomaly_proposal(&invalid))
            == OutputHealthProposalScope::Invalid
        && scope.classify(anomaly_proposal(&foreign))
            == OutputHealthProposalScope::Invalid
        && output_error_is(
            &OutputHealthLeaseScope::new(&health, &foreign),
            OutputHealthError::SourceStreamMismatch,
        )
}

fn changed_output(
    current: &DiagnosticOutputDescriptor,
    incarnation: bool,
    generation: bool,
) -> Option<DiagnosticOutputDescriptor> {
    let identity = current.lifecycle();
    let next_incarnation = identity.incarnation() + u128::from(incarnation);
    let next_generation = identity.generation().value() + u64::from(generation);
    let changed = LifecycleIdentity::new(
        identity.instance(),
        next_incarnation,
        Generation::new(next_generation),
    );
    crate::diagnostic::make_descriptor_for(
        changed,
        current.source_stream(),
        1_024,
    )
}

const fn changed_stream(
    current: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticOutputDescriptor> {
    let stream = current.source_stream();
    let changed = LifecycleIdentity::new(
        stream.instance(),
        stream.incarnation() + 1,
        stream.generation(),
    );
    crate::diagnostic::make_descriptor_for(current.lifecycle(), changed, 1_024)
}

const fn anomaly_proposal(
    output: &DiagnosticOutputDescriptor,
) -> OutputHealthProposal {
    OutputHealthProposal::activation_failure(
        output,
        reovim_kernel_interface::output_health::
            OutputHealthActivationFailureCause::InternalFailure,
        reovim_kernel_interface::lifecycle::RollbackOutcome::Reverted,
    )
}

fn scripted_lease() -> bool {
    let Some((descriptor, output)) = make_health() else {
        return false;
    };
    let Ok(scope) = OutputHealthLeaseScope::new(&descriptor, &output) else {
        return false;
    };
    let proposal = anomaly_proposal(&output);
    let outcomes = [
        OutputHealthRecordOutcome::Accepted(
            OutputHealthAcceptance::CounterOnly,
        ),
        OutputHealthRecordOutcome::Accepted(
            OutputHealthAcceptance::AnomalyRetained(
                OutputHealthSequence::first(),
            ),
        ),
        OutputHealthRecordOutcome::Pressure,
        OutputHealthRecordOutcome::Closed,
        OutputHealthRecordOutcome::SequenceExhausted,
    ];
    outcomes.into_iter().all(|outcome| {
        let lease = ScriptedLease { descriptor, scope, outcome };
        lease.descriptor() == &descriptor
            && lease.scope() == scope
            && lease.record(proposal) == outcome
    })
}

#[derive(Clone, Copy)]
struct ScriptedLease {
    descriptor: OutputHealthDescriptor,
    scope: OutputHealthLeaseScope,
    outcome: OutputHealthRecordOutcome,
}

impl ::reovim_kernel_interface::output_health::OutputHealthLease
    for ScriptedLease
{
    fn descriptor(&self) -> &OutputHealthDescriptor {
        &self.descriptor
    }

    fn scope(&self) -> OutputHealthLeaseScope {
        self.scope
    }

    fn record(
        &self,
        proposal: OutputHealthProposal,
    ) -> OutputHealthRecordOutcome {
        match self.scope.classify(proposal) {
            OutputHealthProposalScope::Current => self.outcome,
            OutputHealthProposalScope::Invalid => {
                OutputHealthRecordOutcome::Invalid
            }
            OutputHealthProposalScope::Stale => {
                OutputHealthRecordOutcome::Stale
            }
        }
    }
}

fn anomaly_validation() -> bool {
    let Some((health, output)) = make_health() else {
        return false;
    };
    let Some(anomaly) = make_anomaly(&health, &output, 1) else {
        return false;
    };
    let Some(record) = crate::diagnostic::make_record(&output) else {
        return false;
    };
    let counter_only = OutputHealthProposal::submission_accepted(&record);
    anomaly.sequence() == OutputHealthSequence::first()
        && anomaly.proposal().is_anomaly_candidate()
        && anomaly.resident_bytes().value() == ANOMALY_BYTES
        && output_error_is(
            &OutputHealthAnomalyRecord::new(
                &health,
                OutputHealthSequence::first(),
                counter_only,
            ),
            OutputHealthError::CounterOnlyProposal,
        )
        && traits(anomaly)
}

fn error_traits_case() -> bool {
    let values = [
        OutputHealthError::LimitsStreamMismatch,
        OutputHealthError::IncoherentSnapshotLimits,
        OutputHealthError::UnsupportedContract,
        OutputHealthError::IncompatibleContract,
        OutputHealthError::UnassignedSequence,
        OutputHealthError::SequenceExhausted,
        OutputHealthError::SourceStreamMismatch,
        OutputHealthError::CounterOnlyProposal,
        OutputHealthError::InvalidRetainedRange,
        OutputHealthError::SnapshotBoundExceeded,
        OutputHealthError::InvalidSnapshotMetadata,
        OutputHealthError::InvalidSnapshotRecords,
    ];
    values
        .into_iter()
        .all(|value| error_traits(&value) && !value.to_string().is_empty())
}

pub fn output_error_is<T>(
    result: &Result<T, OutputHealthError>,
    expected: OutputHealthError,
) -> bool {
    match result {
        Err(error) => *error == expected,
        Ok(_) => false,
    }
}

pub const fn output_bound_error<T>(
    result: &Result<T, OutputHealthError>,
) -> bool {
    match result {
        Err(OutputHealthError::Bound(_)) => true,
        Err(_) | Ok(_) => false,
    }
}

pub const fn next_position_is(
    position: OutputHealthSequencePosition,
    expected: OutputHealthSequence,
) -> bool {
    match position {
        OutputHealthSequencePosition::Next(sequence) => {
            sequence.value() == expected.value()
        }
        OutputHealthSequencePosition::Exhausted => false,
    }
}

const fn proposal_scope_variants() -> bool {
    scope_code(OutputHealthProposalScope::Current) == 0
        && scope_code(OutputHealthProposalScope::Invalid) == 1
        && scope_code(OutputHealthProposalScope::Stale) == 2
}

const fn scope_code(value: OutputHealthProposalScope) -> u8 {
    match value {
        OutputHealthProposalScope::Current => 0,
        OutputHealthProposalScope::Invalid => 1,
        OutputHealthProposalScope::Stale => 2,
    }
}

const fn sequence_position_variants() -> bool {
    position_code(OutputHealthSequencePosition::Next(
        OutputHealthSequence::first(),
    )) == 0
        && position_code(OutputHealthSequencePosition::Exhausted) == 1
}

const fn position_code(value: OutputHealthSequencePosition) -> u8 {
    match value {
        OutputHealthSequencePosition::Next(_) => 0,
        OutputHealthSequencePosition::Exhausted => 1,
    }
}

const fn acceptance_variants() -> bool {
    acceptance_code(OutputHealthAcceptance::CounterOnly) == 0
        && acceptance_code(OutputHealthAcceptance::AnomalyRetained(
            OutputHealthSequence::first(),
        )) == 1
}

const fn acceptance_code(value: OutputHealthAcceptance) -> u8 {
    match value {
        OutputHealthAcceptance::CounterOnly => 0,
        OutputHealthAcceptance::AnomalyRetained(_) => 1,
    }
}

const fn outcome_variants() -> bool {
    outcome_code(OutputHealthRecordOutcome::Accepted(
        OutputHealthAcceptance::CounterOnly,
    )) == 0
        && outcome_code(OutputHealthRecordOutcome::Pressure) == 1
        && outcome_code(OutputHealthRecordOutcome::Invalid) == 2
        && outcome_code(OutputHealthRecordOutcome::Stale) == 3
        && outcome_code(OutputHealthRecordOutcome::Closed) == 4
        && outcome_code(OutputHealthRecordOutcome::SequenceExhausted) == 5
}

const fn outcome_code(value: OutputHealthRecordOutcome) -> u8 {
    match value {
        OutputHealthRecordOutcome::Accepted(_) => 0,
        OutputHealthRecordOutcome::Pressure => 1,
        OutputHealthRecordOutcome::Invalid => 2,
        OutputHealthRecordOutcome::Stale => 3,
        OutputHealthRecordOutcome::Closed => 4,
        OutputHealthRecordOutcome::SequenceExhausted => 5,
    }
}

const fn record_result_variants() -> bool {
    record_result_code(OutputHealthRecordResultKind::Pressure) == 0
        && record_result_code(OutputHealthRecordResultKind::Invalid) == 1
        && record_result_code(OutputHealthRecordResultKind::Stale) == 2
        && record_result_code(OutputHealthRecordResultKind::Closed) == 3
        && record_result_code(OutputHealthRecordResultKind::SequenceExhaustion)
            == 4
}

const fn record_result_code(value: OutputHealthRecordResultKind) -> u8 {
    match value {
        OutputHealthRecordResultKind::Pressure => 0,
        OutputHealthRecordResultKind::Invalid => 1,
        OutputHealthRecordResultKind::Stale => 2,
        OutputHealthRecordResultKind::Closed => 3,
        OutputHealthRecordResultKind::SequenceExhaustion => 4,
    }
}

const fn import_kind_variants() -> bool {
    import_kind_code(OutputHealthImportCounterKind::Accepted) == 0
        && import_kind_code(OutputHealthImportCounterKind::Rejected) == 1
        && import_kind_code(OutputHealthImportCounterKind::Dropped) == 2
        && import_kind_code(OutputHealthImportCounterKind::Closed) == 3
        && import_kind_code(OutputHealthImportCounterKind::AttemptLimit) == 4
}

const fn import_kind_code(value: OutputHealthImportCounterKind) -> u8 {
    match value {
        OutputHealthImportCounterKind::Accepted => 0,
        OutputHealthImportCounterKind::Rejected => 1,
        OutputHealthImportCounterKind::Dropped => 2,
        OutputHealthImportCounterKind::Closed => 3,
        OutputHealthImportCounterKind::AttemptLimit => 4,
    }
}
