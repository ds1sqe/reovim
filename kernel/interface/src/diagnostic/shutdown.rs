use reovim_foundation::bound::{CheckedByteCount, CheckedCount, ZeroPolicy};

use crate::cancellation::ExecutionOutcome;
use crate::effect::EffectCertainty;
use crate::lifecycle::{ShutdownOutcome, WorkDisposition};

use super::{
    DiagnosticDestinationCause, DiagnosticDestinationCut,
    DiagnosticDestinationResult, DiagnosticOutputDescriptor,
    DiagnosticOutputOwnerState, DiagnosticShutdownEntry,
    DiagnosticShutdownEntryError, DiagnosticShutdownPolicy,
    DiagnosticShutdownReportError, DiagnosticShutdownStatistics,
};

pub(super) const fn validate_entry(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> Result<(), DiagnosticShutdownEntryError> {
    if !disposition_execution_matches(disposition, execution) {
        return Err(
            DiagnosticShutdownEntryError::DispositionExecutionMismatch {
                disposition,
                execution,
            },
        );
    }
    if !execution_destination_matches(execution, destination) {
        return Err(
            DiagnosticShutdownEntryError::ExecutionDestinationMismatch {
                execution,
                cut: destination.cut(),
            },
        );
    }
    if !disposition_destination_matches(disposition, execution, destination) {
        return Err(
            DiagnosticShutdownEntryError::DispositionDestinationMismatch {
                disposition,
                execution,
                certainty: destination.certainty(),
                cause: destination.cause(),
                cut: destination.cut(),
                confirmed_prefix: destination.confirmed_prefix(),
            },
        );
    }
    if !reaping_execution_matches(destination, execution) {
        return Err(DiagnosticShutdownEntryError::ReapingExecutionMismatch {
            execution,
        });
    }
    Ok(())
}

const fn disposition_execution_matches(
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> bool {
    match disposition {
        WorkDisposition::Drained => execution.is_terminal(),
        WorkDisposition::Discarded => execution_is_never_started(execution),
        WorkDisposition::Unresolved => execution_is_nonterminal(execution),
    }
}

const fn execution_destination_matches(
    execution: ExecutionOutcome,
    destination: DiagnosticDestinationResult,
) -> bool {
    match execution {
        ExecutionOutcome::NeverStarted => cut_is_before_call(destination.cut()),
        ExecutionOutcome::OwnedNonterminal
        | ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => true,
    }
}

const fn disposition_destination_matches(
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
    destination: DiagnosticDestinationResult,
) -> bool {
    match disposition {
        WorkDisposition::Drained => {
            drained_destination_matches(execution, destination.cause())
        }
        WorkDisposition::Discarded => {
            discarded_destination_matches(destination)
        }
        WorkDisposition::Unresolved => {
            !cause_is_closing_or_replacement(destination.cause())
        }
    }
}

const fn drained_destination_matches(
    execution: ExecutionOutcome,
    cause: DiagnosticDestinationCause,
) -> bool {
    if cause_is_closing_or_replacement(cause) {
        return false;
    }
    match execution {
        ExecutionOutcome::NeverStarted => cause_is_never_started_drain(cause),
        ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => true,
        ExecutionOutcome::OwnedNonterminal => false,
    }
}

const fn discarded_destination_matches(
    destination: DiagnosticDestinationResult,
) -> bool {
    if !cut_is_before_call(destination.cut()) {
        return false;
    }
    if !certainty_is_not_applied(destination.certainty()) {
        return false;
    }
    if destination.confirmed_prefix().value() != 0 {
        return false;
    }
    cause_is_discard(destination.cause())
}

const fn reaping_execution_matches(
    destination: DiagnosticDestinationResult,
    execution: ExecutionOutcome,
) -> bool {
    match destination.cause() {
        DiagnosticDestinationCause::SupervisorReaping => {
            execution_is_reaped(execution)
        }
        DiagnosticDestinationCause::Success
        | DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::WriterContractViolation
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::InternalFailureInCall => true,
    }
}

const fn execution_is_never_started(execution: ExecutionOutcome) -> bool {
    match execution {
        ExecutionOutcome::NeverStarted => true,
        ExecutionOutcome::OwnedNonterminal
        | ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => false,
    }
}

const fn execution_is_nonterminal(execution: ExecutionOutcome) -> bool {
    match execution {
        ExecutionOutcome::OwnedNonterminal => true,
        ExecutionOutcome::NeverStarted
        | ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => false,
    }
}

const fn execution_is_reaped(execution: ExecutionOutcome) -> bool {
    match execution {
        ExecutionOutcome::ContainingProcessReaped => true,
        ExecutionOutcome::NeverStarted
        | ExecutionOutcome::OwnedNonterminal
        | ExecutionOutcome::Stopped => false,
    }
}

const fn cut_is_before_call(cut: DiagnosticDestinationCut) -> bool {
    match cut {
        DiagnosticDestinationCut::BeforeCall => true,
        DiagnosticDestinationCut::InCall
        | DiagnosticDestinationCut::Returned => false,
    }
}

const fn certainty_is_not_applied(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::DefinitelyNotApplied => true,
        EffectCertainty::Applied | EffectCertainty::OutcomeUnknown => false,
    }
}

const fn cause_is_never_started_drain(
    cause: DiagnosticDestinationCause,
) -> bool {
    match cause {
        DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::InternalFailureBeforeCall => true,
        DiagnosticDestinationCause::Success
        | DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::WriterContractViolation
        | DiagnosticDestinationCause::InternalFailureInCall
        | DiagnosticDestinationCause::SupervisorReaping => false,
    }
}

const fn cause_is_closing_or_replacement(
    cause: DiagnosticDestinationCause,
) -> bool {
    match cause {
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard => true,
        DiagnosticDestinationCause::Success
        | DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::WriterContractViolation
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::InternalFailureInCall
        | DiagnosticDestinationCause::SupervisorReaping => false,
    }
}

const fn cause_is_discard(cause: DiagnosticDestinationCause) -> bool {
    match cause {
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::ShutdownDeadline => true,
        DiagnosticDestinationCause::Success
        | DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::WriterContractViolation
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::InternalFailureInCall
        | DiagnosticDestinationCause::SupervisorReaping => false,
    }
}

pub(super) fn validate_report(
    descriptor: &DiagnosticOutputDescriptor,
    owner: DiagnosticOutputOwnerState,
    entries: &[DiagnosticShutdownEntry],
) -> Result<DiagnosticShutdownStatistics, DiagnosticShutdownReportError> {
    let records = checked_entry_count(entries.len())?;
    let maximum_records = descriptor.limits().shutdown_report_records();
    if records.value() > maximum_records.value() {
        return Err(DiagnosticShutdownReportError::RecordCapacityExceeded {
            requested: records,
            maximum: maximum_records,
        });
    }
    let one = checked_count(1)?;
    let zero_count = checked_count(0)?;
    let zero_bytes = checked_bytes(0)?;
    let record_bytes = maximum_record_bytes(descriptor)?;
    let maximum_bytes = descriptor.limits().shutdown_report_record_bytes();
    let mut statistics = DiagnosticShutdownStatistics {
        drained_records: zero_count,
        drained_source_bytes: zero_bytes,
        discarded_records: zero_count,
        discarded_source_bytes: zero_bytes,
        unresolved_records: zero_count,
        unresolved_source_bytes: zero_bytes,
        total_records: zero_count,
        total_source_bytes: zero_bytes,
    };
    let mut previous = None;
    for entry in entries {
        validate_report_entry(descriptor, previous, *entry, record_bytes)?;
        statistics = add_statistics(statistics, *entry, one, maximum_bytes)?;
        previous = Some(entry.destination().sequence());
    }
    if !owner_accepts_unresolved(owner, statistics.unresolved_records()) {
        return Err(DiagnosticShutdownReportError::UnresolvedOwnerMismatch {
            owner,
            unresolved: statistics.unresolved_records(),
        });
    }
    Ok(statistics)
}

fn checked_entry_count(
    length: usize,
) -> Result<CheckedCount, DiagnosticShutdownReportError> {
    let Ok(value) = u64::try_from(length) else {
        return Err(DiagnosticShutdownReportError::EntryCountUnrepresentable);
    };
    checked_count(value)
}

const fn checked_count(
    value: u64,
) -> Result<CheckedCount, DiagnosticShutdownReportError> {
    let Ok(value) = CheckedCount::new(value, ZeroPolicy::Allow) else {
        return Err(DiagnosticShutdownReportError::EntryCountUnrepresentable);
    };
    Ok(value)
}

const fn checked_bytes(
    value: u64,
) -> Result<CheckedByteCount, DiagnosticShutdownReportError> {
    let Ok(value) = CheckedByteCount::new(value, ZeroPolicy::Allow) else {
        return Err(DiagnosticShutdownReportError::EntryCountUnrepresentable);
    };
    Ok(value)
}

const fn maximum_record_bytes(
    descriptor: &DiagnosticOutputDescriptor,
) -> Result<CheckedByteCount, DiagnosticShutdownReportError> {
    let maximum = descriptor.limits().byte_capacity().per_object().value();
    checked_bytes(maximum)
}

const fn validate_report_entry(
    descriptor: &DiagnosticOutputDescriptor,
    previous: Option<crate::event::EventSequence>,
    entry: DiagnosticShutdownEntry,
    maximum_record_bytes: CheckedByteCount,
) -> Result<(), DiagnosticShutdownReportError> {
    let destination = entry.destination();
    let sequence = destination.sequence();
    if !descriptor.lifecycle().same_generation(destination.output()) {
        return Err(DiagnosticShutdownReportError::OutputMismatch {
            sequence,
            expected: descriptor.lifecycle(),
            actual: destination.output(),
        });
    }
    if !descriptor.source_stream().same_generation(destination.stream()) {
        return Err(DiagnosticShutdownReportError::SourceStreamMismatch {
            sequence,
            expected: descriptor.source_stream(),
            actual: destination.stream(),
        });
    }
    if destination.total_bytes().value() > maximum_record_bytes.value() {
        return Err(
            DiagnosticShutdownReportError::RecordByteCapacityExceeded {
                sequence,
                bytes: destination.total_bytes(),
                maximum: maximum_record_bytes,
            },
        );
    }
    if let Some(previous) = previous
        && !previous.is_before(sequence)
    {
        return Err(DiagnosticShutdownReportError::SequenceNotIncreasing {
            previous,
            current: sequence,
        });
    }
    if !policy_discard_matches(descriptor.shutdown_policy(), entry) {
        return Err(DiagnosticShutdownReportError::PolicyDiscardMismatch {
            sequence,
            policy: descriptor.shutdown_policy(),
            cause: destination.cause(),
        });
    }
    Ok(())
}

const fn policy_discard_matches(
    policy: DiagnosticShutdownPolicy,
    entry: DiagnosticShutdownEntry,
) -> bool {
    match entry.disposition() {
        WorkDisposition::Discarded => {
            discard_cause_matches_policy(policy, entry.destination().cause())
        }
        WorkDisposition::Drained | WorkDisposition::Unresolved => true,
    }
}

const fn discard_cause_matches_policy(
    policy: DiagnosticShutdownPolicy,
    cause: DiagnosticDestinationCause,
) -> bool {
    match (policy, cause) {
        (
            DiagnosticShutdownPolicy::Discard,
            DiagnosticDestinationCause::ClosingDiscard
            | DiagnosticDestinationCause::ReplacementDiscard,
        )
        | (
            DiagnosticShutdownPolicy::Drain,
            DiagnosticDestinationCause::ShutdownDeadline,
        ) => {}
        _ => return false,
    }
    true
}

fn add_statistics(
    mut statistics: DiagnosticShutdownStatistics,
    entry: DiagnosticShutdownEntry,
    one: CheckedCount,
    maximum: CheckedByteCount,
) -> Result<DiagnosticShutdownStatistics, DiagnosticShutdownReportError> {
    let sequence = entry.destination().sequence();
    let next = entry.destination().total_bytes();
    ensure_source_capacity(
        statistics.total_source_bytes,
        next,
        maximum,
        sequence,
    )?;
    statistics.total_records = add_count(statistics.total_records, one)?;
    statistics.total_source_bytes =
        add_bytes(statistics.total_source_bytes, next, maximum, sequence)?;
    match entry.disposition() {
        WorkDisposition::Drained => {
            statistics.drained_records =
                add_count(statistics.drained_records, one)?;
            statistics.drained_source_bytes = add_bytes(
                statistics.drained_source_bytes,
                next,
                maximum,
                sequence,
            )?;
        }
        WorkDisposition::Discarded => {
            statistics.discarded_records =
                add_count(statistics.discarded_records, one)?;
            statistics.discarded_source_bytes = add_bytes(
                statistics.discarded_source_bytes,
                next,
                maximum,
                sequence,
            )?;
        }
        WorkDisposition::Unresolved => {
            statistics.unresolved_records =
                add_count(statistics.unresolved_records, one)?;
            statistics.unresolved_source_bytes = add_bytes(
                statistics.unresolved_source_bytes,
                next,
                maximum,
                sequence,
            )?;
        }
    }
    Ok(statistics)
}

const fn ensure_source_capacity(
    accumulated: CheckedByteCount,
    next: CheckedByteCount,
    maximum: CheckedByteCount,
    sequence: crate::event::EventSequence,
) -> Result<(), DiagnosticShutdownReportError> {
    let Some(remaining) = maximum.value().checked_sub(accumulated.value())
    else {
        return Err(source_capacity_error(
            sequence,
            accumulated,
            next,
            maximum,
        ));
    };
    if next.value() > remaining {
        return Err(source_capacity_error(
            sequence,
            accumulated,
            next,
            maximum,
        ));
    }
    Ok(())
}

const fn add_count(
    current: CheckedCount,
    one: CheckedCount,
) -> Result<CheckedCount, DiagnosticShutdownReportError> {
    let Ok(value) = current.checked_add(one) else {
        return Err(DiagnosticShutdownReportError::EntryCountUnrepresentable);
    };
    Ok(value)
}

const fn add_bytes(
    current: CheckedByteCount,
    next: CheckedByteCount,
    maximum: CheckedByteCount,
    sequence: crate::event::EventSequence,
) -> Result<CheckedByteCount, DiagnosticShutdownReportError> {
    let Ok(value) = current.checked_add(next) else {
        return Err(source_capacity_error(sequence, current, next, maximum));
    };
    Ok(value)
}

const fn source_capacity_error(
    sequence: crate::event::EventSequence,
    accumulated: CheckedByteCount,
    next: CheckedByteCount,
    maximum: CheckedByteCount,
) -> DiagnosticShutdownReportError {
    DiagnosticShutdownReportError::SourceByteCapacityExceeded {
        sequence,
        accumulated,
        next,
        maximum,
    }
}

const fn owner_accepts_unresolved(
    owner: DiagnosticOutputOwnerState,
    unresolved: CheckedCount,
) -> bool {
    if unresolved.value() == 0 {
        return true;
    }
    match owner {
        DiagnosticOutputOwnerState::LiveOwned => true,
        DiagnosticOutputOwnerState::NoOwnedExecution
        | DiagnosticOutputOwnerState::ExitedOwned
        | DiagnosticOutputOwnerState::JoinedOwned
        | DiagnosticOutputOwnerState::ContainingProcessReaped => false,
    }
}

pub(super) const fn shutdown_outcome(
    owner: DiagnosticOutputOwnerState,
    unresolved: CheckedCount,
) -> ShutdownOutcome {
    if unresolved.value() != 0 || !owner.release_is_complete() {
        return ShutdownOutcome::IncompleteShutdown;
    }
    ShutdownOutcome::Closed
}
