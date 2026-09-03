//! Diagnostic shutdown values through the public contract.

use reovim_kernel_interface::bound::{
    CheckedByteCount, CheckedCount, ZeroPolicy,
};
use reovim_kernel_interface::cancellation::ExecutionOutcome;
use reovim_kernel_interface::diagnostic::{
    DiagnosticDestinationCause, DiagnosticDestinationCut,
    DiagnosticDestinationResult, DiagnosticOutputDescriptor,
    DiagnosticOutputOwnerState, DiagnosticOutputRecord,
    DiagnosticShutdownEntry, DiagnosticShutdownEntryError,
    DiagnosticShutdownPolicy, DiagnosticShutdownReportError,
    DiagnosticShutdownReportRef, DiagnosticShutdownStatistics,
};
use reovim_kernel_interface::effect::EffectCertainty;
use reovim_kernel_interface::event::{EventSequence, StartupEventRecord};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, ShutdownOutcome, WorkDisposition,
};

use super::{
    make_descriptor, make_descriptor_with, make_record_from_event,
    output_identity,
};
use crate::{error_traits, event, require_debug, traits};

const _: fn(
    DiagnosticDestinationResult,
    WorkDisposition,
    ExecutionOutcome,
) -> Result<DiagnosticShutdownEntry, DiagnosticShutdownEntryError> =
    use_entry_const_api;

const fn use_entry_const_api(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> Result<DiagnosticShutdownEntry, DiagnosticShutdownEntryError> {
    let entry =
        match DiagnosticShutdownEntry::new(destination, disposition, execution)
        {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
    let _evidence =
        (entry.destination(), entry.disposition(), entry.execution());
    Ok(entry)
}

const _: fn(DiagnosticShutdownStatistics) -> bool = use_statistics_const_api;

const fn use_statistics_const_api(
    statistics: DiagnosticShutdownStatistics,
) -> bool {
    let _values = (
        statistics.drained_records(),
        statistics.drained_source_bytes(),
        statistics.discarded_records(),
        statistics.discarded_source_bytes(),
        statistics.unresolved_records(),
        statistics.unresolved_source_bytes(),
        statistics.total_records(),
        statistics.total_source_bytes(),
    );
    true
}

const _: for<'a> fn(
    &DiagnosticOutputDescriptor,
    DiagnosticOutputOwnerState,
    &'a [DiagnosticShutdownEntry],
) -> Result<
    DiagnosticShutdownReportRef<'a>,
    DiagnosticShutdownReportError,
> = use_report_api;

fn use_report_api<'a>(
    descriptor: &DiagnosticOutputDescriptor,
    owner: DiagnosticOutputOwnerState,
    entries: &'a [DiagnosticShutdownEntry],
) -> Result<DiagnosticShutdownReportRef<'a>, DiagnosticShutdownReportError> {
    DiagnosticShutdownReportRef::new(descriptor, owner, entries)
}

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticOutputOwnerState,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownEntry,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownEntryError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownStatistics,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownReportRef<
            'static,
        >,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticShutdownReportError,
    >() && owner_state_cases()
}

const fn owner_state_cases() -> bool {
    owner_state_is(DiagnosticOutputOwnerState::NoOwnedExecution, true, true)
        && owner_state_is(DiagnosticOutputOwnerState::LiveOwned, false, false)
        && owner_state_is(DiagnosticOutputOwnerState::ExitedOwned, true, false)
        && owner_state_is(DiagnosticOutputOwnerState::JoinedOwned, true, true)
        && owner_state_is(
            DiagnosticOutputOwnerState::ContainingProcessReaped,
            true,
            true,
        )
}

const fn owner_state_is(
    state: DiagnosticOutputOwnerState,
    terminal: bool,
    release_complete: bool,
) -> bool {
    state.execution_is_terminal() == terminal
        && state.release_is_complete() == release_complete
}

#[derive(Clone, Copy)]
enum Prefix {
    Zero,
    Partial,
    Full,
}

const CAUSES: [DiagnosticDestinationCause; 12] = [
    DiagnosticDestinationCause::Success,
    DiagnosticDestinationCause::ClosingDiscard,
    DiagnosticDestinationCause::ReplacementDiscard,
    DiagnosticDestinationCause::Cancellation,
    DiagnosticDestinationCause::ShutdownDeadline,
    DiagnosticDestinationCause::WriteZero,
    DiagnosticDestinationCause::WriteFailure,
    DiagnosticDestinationCause::RetryExhaustion,
    DiagnosticDestinationCause::WriterContractViolation,
    DiagnosticDestinationCause::InternalFailureBeforeCall,
    DiagnosticDestinationCause::InternalFailureInCall,
    DiagnosticDestinationCause::SupervisorReaping,
];

const CUTS: [DiagnosticDestinationCut; 3] = [
    DiagnosticDestinationCut::BeforeCall,
    DiagnosticDestinationCut::InCall,
    DiagnosticDestinationCut::Returned,
];

const CERTAINTIES: [EffectCertainty; 3] = [
    EffectCertainty::Applied,
    EffectCertainty::DefinitelyNotApplied,
    EffectCertainty::OutcomeUnknown,
];

const PREFIXES: [Prefix; 3] = [Prefix::Zero, Prefix::Partial, Prefix::Full];

const DISPOSITIONS: [WorkDisposition; 3] = [
    WorkDisposition::Drained,
    WorkDisposition::Discarded,
    WorkDisposition::Unresolved,
];

const EXECUTIONS: [ExecutionOutcome; 4] = [
    ExecutionOutcome::NeverStarted,
    ExecutionOutcome::OwnedNonterminal,
    ExecutionOutcome::Stopped,
    ExecutionOutcome::ContainingProcessReaped,
];

pub fn exercise() -> bool {
    complete_entry_relation()
        && explicit_entry_journeys()
        && entry_validation_order()
        && entry_error_trait_cases()
        && empty_report_cases()
        && completed_report_cases()
        && mixed_report_case()
        && policy_relation_cases()
        && sequence_cases()
        && report_error_cases()
        && report_validation_order()
        && report_error_trait_cases()
}

fn complete_entry_relation() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_ready_output_record(&descriptor) else {
        return false;
    };
    CAUSES.into_iter().all(|cause| complete_cause_relation(&record, cause))
}

fn complete_cause_relation(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
) -> bool {
    CUTS.into_iter().all(|cut| complete_cut_relation(record, cause, cut))
}

fn complete_cut_relation(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> bool {
    CERTAINTIES.into_iter().all(|certainty| {
        complete_certainty_relation(record, cause, cut, certainty)
    })
}

fn complete_certainty_relation(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
) -> bool {
    PREFIXES.into_iter().all(|prefix| {
        complete_destination_relation(record, cause, cut, certainty, prefix)
    })
}

fn complete_destination_relation(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
    prefix: Prefix,
) -> bool {
    let Some(confirmed) = prefix_for(record, prefix) else {
        return false;
    };
    let Ok(destination) = DiagnosticDestinationResult::new(
        record, confirmed, certainty, cause, cut,
    ) else {
        return true;
    };
    DISPOSITIONS.into_iter().all(|disposition| {
        complete_disposition_relation(destination, disposition)
    })
}

fn complete_disposition_relation(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
) -> bool {
    EXECUTIONS.into_iter().all(|execution| {
        complete_entry_case(destination, disposition, execution)
    })
}

#[derive(Clone, Copy)]
enum EntryExpectation {
    Valid,
    DispositionExecution,
    ExecutionDestination,
    DispositionDestination,
    ReapingExecution,
}

fn complete_entry_case(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> bool {
    let expected = expected_entry(destination, disposition, execution);
    let result =
        DiagnosticShutdownEntry::new(destination, disposition, execution);
    entry_result_is(result, expected, destination, disposition, execution)
}

const fn expected_entry(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> EntryExpectation {
    if !disposition_execution_valid(disposition, execution) {
        return EntryExpectation::DispositionExecution;
    }
    if !execution_destination_valid(execution, destination) {
        return EntryExpectation::ExecutionDestination;
    }
    if !disposition_destination_valid(disposition, execution, destination) {
        return EntryExpectation::DispositionDestination;
    }
    if !reaping_execution_valid(destination.cause(), execution) {
        return EntryExpectation::ReapingExecution;
    }
    EntryExpectation::Valid
}

const fn disposition_execution_valid(
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> bool {
    match disposition {
        WorkDisposition::Drained => execution_is_terminal(execution),
        WorkDisposition::Discarded => execution_is_never_started(execution),
        WorkDisposition::Unresolved => {
            execution_is_owned_nonterminal(execution)
        }
    }
}

const fn execution_is_terminal(execution: ExecutionOutcome) -> bool {
    match execution {
        ExecutionOutcome::NeverStarted
        | ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => true,
        ExecutionOutcome::OwnedNonterminal => false,
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

const fn execution_is_owned_nonterminal(execution: ExecutionOutcome) -> bool {
    match execution {
        ExecutionOutcome::OwnedNonterminal => true,
        ExecutionOutcome::NeverStarted
        | ExecutionOutcome::Stopped
        | ExecutionOutcome::ContainingProcessReaped => false,
    }
}

const fn execution_destination_valid(
    execution: ExecutionOutcome,
    destination: DiagnosticDestinationResult,
) -> bool {
    if execution_is_never_started(execution) {
        return cut_is_before_call(destination.cut());
    }
    true
}

const fn cut_is_before_call(cut: DiagnosticDestinationCut) -> bool {
    match cut {
        DiagnosticDestinationCut::BeforeCall => true,
        DiagnosticDestinationCut::InCall
        | DiagnosticDestinationCut::Returned => false,
    }
}

const fn disposition_destination_valid(
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
    destination: DiagnosticDestinationResult,
) -> bool {
    match disposition {
        WorkDisposition::Drained => {
            drained_destination_valid(execution, destination.cause())
        }
        WorkDisposition::Discarded => discarded_destination_valid(destination),
        WorkDisposition::Unresolved => {
            !cause_is_closing_discard(destination.cause())
        }
    }
}

const fn drained_destination_valid(
    execution: ExecutionOutcome,
    cause: DiagnosticDestinationCause,
) -> bool {
    if execution_is_never_started(execution) {
        return cause_is_unstarted_drain(cause);
    }
    !cause_is_closing_discard(cause)
}

const fn cause_is_unstarted_drain(cause: DiagnosticDestinationCause) -> bool {
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

const fn cause_is_closing_discard(cause: DiagnosticDestinationCause) -> bool {
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

const fn discarded_destination_valid(
    destination: DiagnosticDestinationResult,
) -> bool {
    cut_is_before_call(destination.cut())
        && certainty_is_not_applied(destination.certainty())
        && destination.confirmed_prefix().value() == 0
        && cause_is_discard(destination.cause())
}

const fn certainty_is_not_applied(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::DefinitelyNotApplied => true,
        EffectCertainty::Applied | EffectCertainty::OutcomeUnknown => false,
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

const fn reaping_execution_valid(
    cause: DiagnosticDestinationCause,
    execution: ExecutionOutcome,
) -> bool {
    match cause {
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

const fn execution_is_reaped(execution: ExecutionOutcome) -> bool {
    match execution {
        ExecutionOutcome::ContainingProcessReaped => true,
        ExecutionOutcome::NeverStarted
        | ExecutionOutcome::OwnedNonterminal
        | ExecutionOutcome::Stopped => false,
    }
}

fn entry_result_is(
    result: Result<DiagnosticShutdownEntry, DiagnosticShutdownEntryError>,
    expected: EntryExpectation,
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> bool {
    match (expected, result) {
        (EntryExpectation::Valid, Ok(entry)) => {
            entry.destination() == destination
                && entry.disposition() == disposition
                && entry.execution() == execution
                && traits(entry)
        }
        (
            EntryExpectation::DispositionExecution,
            Err(DiagnosticShutdownEntryError::DispositionExecutionMismatch {
                disposition: actual_disposition,
                execution: actual_execution,
            }),
        ) => actual_disposition == disposition && actual_execution == execution,
        (
            EntryExpectation::ExecutionDestination,
            Err(DiagnosticShutdownEntryError::ExecutionDestinationMismatch {
                execution: actual_execution,
                cut: actual_cut,
            }),
        ) => actual_execution == execution && actual_cut == destination.cut(),
        (
            EntryExpectation::DispositionDestination,
            Err(DiagnosticShutdownEntryError::DispositionDestinationMismatch {
                disposition: actual_disposition,
                execution: actual_execution,
                certainty: actual_certainty,
                cause: actual_cause,
                cut: actual_cut,
                confirmed_prefix: actual_prefix,
            }),
        ) => {
            actual_disposition == disposition
                && actual_execution == execution
                && actual_certainty == destination.certainty()
                && actual_cause == destination.cause()
                && actual_cut == destination.cut()
                && actual_prefix == destination.confirmed_prefix()
        }
        (
            EntryExpectation::ReapingExecution,
            Err(DiagnosticShutdownEntryError::ReapingExecutionMismatch {
                execution: actual_execution,
            }),
        ) => actual_execution == execution,
        (
            EntryExpectation::Valid
            | EntryExpectation::DispositionExecution
            | EntryExpectation::ExecutionDestination
            | EntryExpectation::DispositionDestination
            | EntryExpectation::ReapingExecution,
            Err(_) | Ok(_),
        ) => false,
    }
}

fn explicit_entry_journeys() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_ready_output_record(&descriptor) else {
        return false;
    };
    drained_never_started_journeys(&record)
        && unresolved_certainty_journeys(&record)
        && reaping_journeys(&record)
}

fn drained_never_started_journeys(record: &DiagnosticOutputRecord) -> bool {
    let Some(cancellation) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::DefinitelyNotApplied,
        DiagnosticDestinationCause::Cancellation,
        DiagnosticDestinationCut::BeforeCall,
    ) else {
        return false;
    };
    let Some(internal) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::DefinitelyNotApplied,
        DiagnosticDestinationCause::InternalFailureBeforeCall,
        DiagnosticDestinationCut::BeforeCall,
    ) else {
        return false;
    };
    let Some(deadline) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::DefinitelyNotApplied,
        DiagnosticDestinationCause::ShutdownDeadline,
        DiagnosticDestinationCut::BeforeCall,
    ) else {
        return false;
    };
    entry_is_valid(
        cancellation,
        WorkDisposition::Drained,
        ExecutionOutcome::NeverStarted,
    ) && entry_is_valid(
        internal,
        WorkDisposition::Drained,
        ExecutionOutcome::NeverStarted,
    ) && entry_has_error(
        deadline,
        WorkDisposition::Drained,
        ExecutionOutcome::NeverStarted,
        EntryExpectation::DispositionDestination,
    ) && never_started_active_cut_rejections(record)
}

fn never_started_active_cut_rejections(
    record: &DiagnosticOutputRecord,
) -> bool {
    let Some(in_call) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::OutcomeUnknown,
        DiagnosticDestinationCause::Cancellation,
        DiagnosticDestinationCut::InCall,
    ) else {
        return false;
    };
    let Some(returned) = destination_result(
        record,
        Prefix::Full,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
    ) else {
        return false;
    };
    entry_has_error(
        in_call,
        WorkDisposition::Drained,
        ExecutionOutcome::NeverStarted,
        EntryExpectation::ExecutionDestination,
    ) && entry_has_error(
        returned,
        WorkDisposition::Drained,
        ExecutionOutcome::NeverStarted,
        EntryExpectation::ExecutionDestination,
    )
}

fn unresolved_certainty_journeys(record: &DiagnosticOutputRecord) -> bool {
    let Some(before_call) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::DefinitelyNotApplied,
        DiagnosticDestinationCause::Cancellation,
        DiagnosticDestinationCut::BeforeCall,
    ) else {
        return false;
    };
    let Some(applied) = destination_result(
        record,
        Prefix::Full,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
    ) else {
        return false;
    };
    let Some(returned_no_effect) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::DefinitelyNotApplied,
        DiagnosticDestinationCause::WriteFailure,
        DiagnosticDestinationCut::Returned,
    ) else {
        return false;
    };
    let Some(in_call) = destination_result(
        record,
        Prefix::Partial,
        EffectCertainty::OutcomeUnknown,
        DiagnosticDestinationCause::ShutdownDeadline,
        DiagnosticDestinationCut::InCall,
    ) else {
        return false;
    };
    unresolved_entry_is_valid(before_call)
        && unresolved_entry_is_valid(applied)
        && unresolved_entry_is_valid(returned_no_effect)
        && unresolved_entry_is_valid(in_call)
}

fn unresolved_entry_is_valid(destination: DiagnosticDestinationResult) -> bool {
    entry_is_valid(
        destination,
        WorkDisposition::Unresolved,
        ExecutionOutcome::OwnedNonterminal,
    )
}

fn reaping_journeys(record: &DiagnosticOutputRecord) -> bool {
    let Some(reaping) = destination_result(
        record,
        Prefix::Partial,
        EffectCertainty::OutcomeUnknown,
        DiagnosticDestinationCause::SupervisorReaping,
        DiagnosticDestinationCut::InCall,
    ) else {
        return false;
    };
    let Some(success) = destination_result(
        record,
        Prefix::Full,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
    ) else {
        return false;
    };
    entry_is_valid(
        reaping,
        WorkDisposition::Drained,
        ExecutionOutcome::ContainingProcessReaped,
    ) && entry_has_error(
        reaping,
        WorkDisposition::Drained,
        ExecutionOutcome::Stopped,
        EntryExpectation::ReapingExecution,
    ) && entry_is_valid(
        success,
        WorkDisposition::Drained,
        ExecutionOutcome::ContainingProcessReaped,
    )
}

fn entry_is_valid(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
) -> bool {
    complete_entry_case(destination, disposition, execution)
        && expected_entry(destination, disposition, execution).is_valid()
}

fn entry_has_error(
    destination: DiagnosticDestinationResult,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
    expected: EntryExpectation,
) -> bool {
    entry_result_is(
        DiagnosticShutdownEntry::new(destination, disposition, execution),
        expected,
        destination,
        disposition,
        execution,
    )
}

impl EntryExpectation {
    const fn is_valid(self) -> bool {
        match self {
            Self::Valid => true,
            Self::DispositionExecution
            | Self::ExecutionDestination
            | Self::DispositionDestination
            | Self::ReapingExecution => false,
        }
    }
}

fn entry_validation_order() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_ready_output_record(&descriptor) else {
        return false;
    };
    disposition_precedes_execution_destination(&record)
        && execution_precedes_disposition_destination(&record)
        && disposition_precedes_reaping(&record)
}

fn disposition_precedes_execution_destination(
    record: &DiagnosticOutputRecord,
) -> bool {
    let Some(destination) = destination_result(
        record,
        Prefix::Full,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
    ) else {
        return false;
    };
    entry_has_error(
        destination,
        WorkDisposition::Discarded,
        ExecutionOutcome::Stopped,
        EntryExpectation::DispositionExecution,
    )
}

fn execution_precedes_disposition_destination(
    record: &DiagnosticOutputRecord,
) -> bool {
    let Some(destination) = destination_result(
        record,
        Prefix::Partial,
        EffectCertainty::OutcomeUnknown,
        DiagnosticDestinationCause::SupervisorReaping,
        DiagnosticDestinationCut::InCall,
    ) else {
        return false;
    };
    entry_has_error(
        destination,
        WorkDisposition::Drained,
        ExecutionOutcome::NeverStarted,
        EntryExpectation::ExecutionDestination,
    )
}

fn disposition_precedes_reaping(record: &DiagnosticOutputRecord) -> bool {
    let Some(destination) = destination_result(
        record,
        Prefix::Partial,
        EffectCertainty::OutcomeUnknown,
        DiagnosticDestinationCause::SupervisorReaping,
        DiagnosticDestinationCut::InCall,
    ) else {
        return false;
    };
    entry_has_error(
        destination,
        WorkDisposition::Unresolved,
        ExecutionOutcome::OwnedNonterminal,
        EntryExpectation::ReapingExecution,
    )
}

fn entry_error_trait_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_ready_output_record(&descriptor) else {
        return false;
    };
    let Some(destination) = success_destination(&record) else {
        return false;
    };
    let errors = [
        DiagnosticShutdownEntryError::DispositionExecutionMismatch {
            disposition: WorkDisposition::Drained,
            execution: ExecutionOutcome::OwnedNonterminal,
        },
        DiagnosticShutdownEntryError::ExecutionDestinationMismatch {
            execution: ExecutionOutcome::NeverStarted,
            cut: destination.cut(),
        },
        DiagnosticShutdownEntryError::DispositionDestinationMismatch {
            disposition: WorkDisposition::Discarded,
            execution: ExecutionOutcome::NeverStarted,
            certainty: destination.certainty(),
            cause: destination.cause(),
            cut: destination.cut(),
            confirmed_prefix: destination.confirmed_prefix(),
        },
        DiagnosticShutdownEntryError::ReapingExecutionMismatch {
            execution: ExecutionOutcome::Stopped,
        },
    ];
    errors.into_iter().all(entry_error_has_traits)
}

fn entry_error_has_traits(error: DiagnosticShutdownEntryError) -> bool {
    require_debug(&error);
    traits(error) && error_traits(&error)
}

fn prefix_for(
    record: &DiagnosticOutputRecord,
    prefix: Prefix,
) -> Option<CheckedByteCount> {
    let value = match prefix {
        Prefix::Zero => 0,
        Prefix::Partial => record.size().value().checked_sub(1)?,
        Prefix::Full => record.size().value(),
    };
    CheckedByteCount::new(value, ZeroPolicy::Allow).ok()
}

fn destination_result(
    record: &DiagnosticOutputRecord,
    prefix: Prefix,
    certainty: EffectCertainty,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> Option<DiagnosticDestinationResult> {
    let confirmed = prefix_for(record, prefix)?;
    DiagnosticDestinationResult::new(record, confirmed, certainty, cause, cut)
        .ok()
}

fn success_destination(
    record: &DiagnosticOutputRecord,
) -> Option<DiagnosticDestinationResult> {
    destination_result(
        record,
        Prefix::Full,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
    )
}

fn make_ready_output_record(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticOutputRecord> {
    let record = event::make_ready_record()?;
    make_record_from_event(descriptor, &record)
}

fn make_output_record(
    descriptor: &DiagnosticOutputDescriptor,
    source: &StartupEventRecord<'_>,
) -> Option<DiagnosticOutputRecord> {
    make_record_from_event(descriptor, source)
}

#[derive(Clone, Copy)]
struct EntryEvidence {
    prefix: Prefix,
    certainty: EffectCertainty,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    disposition: WorkDisposition,
    execution: ExecutionOutcome,
}

fn entry_from_source(
    descriptor: &DiagnosticOutputDescriptor,
    source: &StartupEventRecord<'_>,
    evidence: EntryEvidence,
) -> Option<DiagnosticShutdownEntry> {
    let record = make_output_record(descriptor, source)?;
    let destination = destination_result(
        &record,
        evidence.prefix,
        evidence.certainty,
        evidence.cause,
        evidence.cut,
    )?;
    let moved_record = record;
    drop(moved_record);
    DiagnosticShutdownEntry::new(
        destination,
        evidence.disposition,
        evidence.execution,
    )
    .ok()
}

fn drained_entry(
    descriptor: &DiagnosticOutputDescriptor,
    source: &StartupEventRecord<'_>,
) -> Option<DiagnosticShutdownEntry> {
    entry_from_source(
        descriptor,
        source,
        EntryEvidence {
            prefix: Prefix::Full,
            certainty: EffectCertainty::Applied,
            cause: DiagnosticDestinationCause::Success,
            cut: DiagnosticDestinationCut::Returned,
            disposition: WorkDisposition::Drained,
            execution: ExecutionOutcome::Stopped,
        },
    )
}

fn ready_drained_entry(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_ready_record()?;
    drained_entry(descriptor, &source)
}

fn degraded_drained_entry(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_degraded_record()?;
    drained_entry(descriptor, &source)
}

fn failed_drained_entry(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_failed_record()?;
    drained_entry(descriptor, &source)
}

fn discarded_entry(
    descriptor: &DiagnosticOutputDescriptor,
    source: &StartupEventRecord<'_>,
    cause: DiagnosticDestinationCause,
) -> Option<DiagnosticShutdownEntry> {
    entry_from_source(
        descriptor,
        source,
        EntryEvidence {
            prefix: Prefix::Zero,
            certainty: EffectCertainty::DefinitelyNotApplied,
            cause,
            cut: DiagnosticDestinationCut::BeforeCall,
            disposition: WorkDisposition::Discarded,
            execution: ExecutionOutcome::NeverStarted,
        },
    )
}

fn ready_discarded_entry(
    descriptor: &DiagnosticOutputDescriptor,
    cause: DiagnosticDestinationCause,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_ready_record()?;
    discarded_entry(descriptor, &source, cause)
}

fn degraded_discarded_entry(
    descriptor: &DiagnosticOutputDescriptor,
    cause: DiagnosticDestinationCause,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_degraded_record()?;
    discarded_entry(descriptor, &source, cause)
}

fn unresolved_entry(
    descriptor: &DiagnosticOutputDescriptor,
    source: &StartupEventRecord<'_>,
) -> Option<DiagnosticShutdownEntry> {
    entry_from_source(
        descriptor,
        source,
        EntryEvidence {
            prefix: Prefix::Full,
            certainty: EffectCertainty::Applied,
            cause: DiagnosticDestinationCause::Success,
            cut: DiagnosticDestinationCut::Returned,
            disposition: WorkDisposition::Unresolved,
            execution: ExecutionOutcome::OwnedNonterminal,
        },
    )
}

fn ready_unresolved_entry(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_ready_record()?;
    unresolved_entry(descriptor, &source)
}

fn degraded_unresolved_entry(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_degraded_record()?;
    unresolved_entry(descriptor, &source)
}

fn failed_unresolved_entry(
    descriptor: &DiagnosticOutputDescriptor,
) -> Option<DiagnosticShutdownEntry> {
    let source = event::make_failed_record()?;
    unresolved_entry(descriptor, &source)
}

fn empty_report_cases() -> bool {
    empty_report_is(
        DiagnosticOutputOwnerState::NoOwnedExecution,
        ShutdownOutcome::Closed,
    ) && empty_report_is(
        DiagnosticOutputOwnerState::LiveOwned,
        ShutdownOutcome::IncompleteShutdown,
    ) && empty_report_is(
        DiagnosticOutputOwnerState::ExitedOwned,
        ShutdownOutcome::IncompleteShutdown,
    ) && empty_report_is(
        DiagnosticOutputOwnerState::JoinedOwned,
        ShutdownOutcome::Closed,
    ) && empty_report_is(
        DiagnosticOutputOwnerState::ContainingProcessReaped,
        ShutdownOutcome::Closed,
    )
}

fn empty_report_is(
    owner: DiagnosticOutputOwnerState,
    outcome: ShutdownOutcome,
) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let entries = [];
    let Ok(report) =
        DiagnosticShutdownReportRef::new(&descriptor, owner, &entries)
    else {
        return false;
    };
    let output_matches = report.output() == descriptor.lifecycle();
    let source_matches = report.source_stream() == descriptor.source_stream();
    let policy_matches = report.policy() == descriptor.shutdown_policy();
    output_matches
        && source_matches
        && policy_matches
        && report.outcome() == outcome
        && report.owner_at_latch() == owner
        && report.entries() == entries.as_slice()
        && statistics_are_zero(report.statistics())
        && traits(report)
}

fn statistics_are_zero(statistics: DiagnosticShutdownStatistics) -> bool {
    let Some(records) = checked_count(0) else {
        return false;
    };
    let Some(bytes) = checked_bytes(0) else {
        return false;
    };
    statistics.drained_records() == records
        && statistics.drained_source_bytes() == bytes
        && statistics.discarded_records() == records
        && statistics.discarded_source_bytes() == bytes
        && statistics.unresolved_records() == records
        && statistics.unresolved_source_bytes() == bytes
        && statistics.total_records() == records
        && statistics.total_source_bytes() == bytes
        && traits(statistics)
}

fn completed_report_cases() -> bool {
    completed_report_is(DiagnosticOutputOwnerState::NoOwnedExecution)
        && completed_report_is(DiagnosticOutputOwnerState::JoinedOwned)
        && completed_report_is(
            DiagnosticOutputOwnerState::ContainingProcessReaped,
        )
        && reaped_entry_with_joined_owner()
}

fn reaped_entry_with_joined_owner() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(source) = event::make_ready_record() else {
        return false;
    };
    let Some(entry) = entry_from_source(
        &descriptor,
        &source,
        EntryEvidence {
            prefix: Prefix::Partial,
            certainty: EffectCertainty::OutcomeUnknown,
            cause: DiagnosticDestinationCause::SupervisorReaping,
            cut: DiagnosticDestinationCut::InCall,
            disposition: WorkDisposition::Drained,
            execution: ExecutionOutcome::ContainingProcessReaped,
        },
    ) else {
        return false;
    };
    let entries = [entry];
    let owner = DiagnosticOutputOwnerState::JoinedOwned;
    let Ok(report) =
        DiagnosticShutdownReportRef::new(&descriptor, owner, &entries)
    else {
        return false;
    };
    report.owner_at_latch() == owner
        && report.outcome() == ShutdownOutcome::Closed
        && report.entries() == entries.as_slice()
}

fn completed_report_is(owner: DiagnosticOutputOwnerState) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&descriptor) else {
        return false;
    };
    let entries = [entry];
    let Ok(report) =
        DiagnosticShutdownReportRef::new(&descriptor, owner, &entries)
    else {
        return false;
    };
    let Some(one) = checked_count(1) else {
        return false;
    };
    let statistics = report.statistics();
    report.outcome() == ShutdownOutcome::Closed
        && statistics.drained_records() == one
        && statistics.drained_source_bytes()
            == entry.destination().total_bytes()
        && statistics.total_records() == one
        && statistics.total_source_bytes() == entry.destination().total_bytes()
}

fn mixed_report_case() -> bool {
    let Some(descriptor) =
        descriptor_for_policy(DiagnosticShutdownPolicy::Discard)
    else {
        return false;
    };
    let Some(drained) = ready_drained_entry(&descriptor) else {
        return false;
    };
    let Some(discarded) = degraded_discarded_entry(
        &descriptor,
        DiagnosticDestinationCause::ClosingDiscard,
    ) else {
        return false;
    };
    let Some(unresolved) = failed_unresolved_entry(&descriptor) else {
        return false;
    };
    let entries = [drained, discarded, unresolved];
    let Ok(report) = DiagnosticShutdownReportRef::new(
        &descriptor,
        DiagnosticOutputOwnerState::LiveOwned,
        &entries,
    ) else {
        return false;
    };
    report_metadata_is(&report, &descriptor, &entries)
        && mixed_statistics_are_exact(
            report.statistics(),
            drained,
            discarded,
            unresolved,
        )
        && report.outcome() == ShutdownOutcome::IncompleteShutdown
        && report.owner_at_latch() == DiagnosticOutputOwnerState::LiveOwned
        && traits(report)
}

fn report_metadata_is(
    report: &DiagnosticShutdownReportRef<'_>,
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
) -> bool {
    let output_matches = report.output() == descriptor.lifecycle();
    let source_matches = report.source_stream() == descriptor.source_stream();
    let policy_matches = report.policy() == descriptor.shutdown_policy();
    output_matches
        && source_matches
        && policy_matches
        && report.entries() == entries
}

fn mixed_statistics_are_exact(
    statistics: DiagnosticShutdownStatistics,
    drained: DiagnosticShutdownEntry,
    discarded: DiagnosticShutdownEntry,
    unresolved: DiagnosticShutdownEntry,
) -> bool {
    let Some(one) = checked_count(1) else {
        return false;
    };
    let Some(three) = checked_count(3) else {
        return false;
    };
    let drained_bytes = drained.destination().total_bytes();
    let discarded_bytes = discarded.destination().total_bytes();
    let unresolved_bytes = unresolved.destination().total_bytes();
    let Ok(partial) = drained_bytes.checked_add(discarded_bytes) else {
        return false;
    };
    let Ok(total) = partial.checked_add(unresolved_bytes) else {
        return false;
    };
    statistics.drained_records() == one
        && statistics.drained_source_bytes() == drained_bytes
        && statistics.discarded_records() == one
        && statistics.discarded_source_bytes() == discarded_bytes
        && statistics.unresolved_records() == one
        && statistics.unresolved_source_bytes() == unresolved_bytes
        && statistics.total_records() == three
        && statistics.total_source_bytes() == total
}

const fn descriptor_for_policy(
    policy: DiagnosticShutdownPolicy,
) -> Option<DiagnosticOutputDescriptor> {
    make_descriptor_with(
        output_identity(),
        event::stream_identity(),
        1_024,
        4,
        4_096,
        policy,
    )
}

fn policy_relation_cases() -> bool {
    closing_policy_cases()
        && replacement_policy_cases()
        && deadline_policy_cases()
        && discarded_interruption_rejections()
}

fn closing_policy_cases() -> bool {
    discard_cause_policy_cases(DiagnosticDestinationCause::ClosingDiscard)
}

fn replacement_policy_cases() -> bool {
    discard_cause_policy_cases(DiagnosticDestinationCause::ReplacementDiscard)
}

fn discard_cause_policy_cases(cause: DiagnosticDestinationCause) -> bool {
    let Some(drain) = descriptor_for_policy(DiagnosticShutdownPolicy::Drain)
    else {
        return false;
    };
    let Some(entry) = ready_discarded_entry(&drain, cause) else {
        return false;
    };
    let entries = [entry];
    policy_error_is(&drain, &entries, DiagnosticShutdownPolicy::Drain, cause)
        && policy_report_is_valid(DiagnosticShutdownPolicy::Discard, &entries)
}

fn deadline_policy_cases() -> bool {
    let Some(drain) = descriptor_for_policy(DiagnosticShutdownPolicy::Drain)
    else {
        return false;
    };
    let cause = DiagnosticDestinationCause::ShutdownDeadline;
    let Some(entry) = ready_discarded_entry(&drain, cause) else {
        return false;
    };
    let entries = [entry];
    policy_report_is_valid(DiagnosticShutdownPolicy::Drain, &entries)
        && policy_error_for_policy_is(
            DiagnosticShutdownPolicy::Discard,
            &entries,
            cause,
        )
}

fn policy_report_is_valid(
    policy: DiagnosticShutdownPolicy,
    entries: &[DiagnosticShutdownEntry],
) -> bool {
    let Some(descriptor) = descriptor_for_policy(policy) else {
        return false;
    };
    let Ok(report) = DiagnosticShutdownReportRef::new(
        &descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    ) else {
        return false;
    };
    report.policy() == policy && report.outcome() == ShutdownOutcome::Closed
}

fn policy_error_for_policy_is(
    policy: DiagnosticShutdownPolicy,
    entries: &[DiagnosticShutdownEntry],
    cause: DiagnosticDestinationCause,
) -> bool {
    let Some(descriptor) = descriptor_for_policy(policy) else {
        return false;
    };
    policy_error_is(&descriptor, entries, policy, cause)
}

fn policy_error_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    policy: DiagnosticShutdownPolicy,
    cause: DiagnosticDestinationCause,
) -> bool {
    policy_error_at_is(
        descriptor,
        entries,
        policy,
        cause,
        entries[0].destination().sequence(),
    )
}

fn policy_error_at_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    policy: DiagnosticShutdownPolicy,
    cause: DiagnosticDestinationCause,
    expected_sequence: EventSequence,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::PolicyDiscardMismatch {
            sequence,
            policy: actual_policy,
            cause: actual_cause,
        }) => {
            sequence == expected_sequence
                && actual_policy == policy
                && actual_cause == cause
        }
        Err(_) | Ok(_) => false,
    }
}

fn discarded_interruption_rejections() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_ready_output_record(&descriptor) else {
        return false;
    };
    discarded_cause_rejected(&record, DiagnosticDestinationCause::Cancellation)
        && discarded_cause_rejected(
            &record,
            DiagnosticDestinationCause::InternalFailureBeforeCall,
        )
}

fn discarded_cause_rejected(
    record: &DiagnosticOutputRecord,
    cause: DiagnosticDestinationCause,
) -> bool {
    let Some(destination) = destination_result(
        record,
        Prefix::Zero,
        EffectCertainty::DefinitelyNotApplied,
        cause,
        DiagnosticDestinationCut::BeforeCall,
    ) else {
        return false;
    };
    entry_has_error(
        destination,
        WorkDisposition::Discarded,
        ExecutionOutcome::NeverStarted,
        EntryExpectation::DispositionDestination,
    )
}

fn sequence_cases() -> bool {
    sequence_gap_is_valid()
        && duplicate_sequence_is_rejected()
        && reversed_sequence_is_rejected()
}

fn sequence_gap_is_valid() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&descriptor) else {
        return false;
    };
    let Some(third) = failed_drained_entry(&descriptor) else {
        return false;
    };
    let entries = [first, third];
    DiagnosticShutdownReportRef::new(
        &descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        &entries,
    )
    .is_ok()
}

fn duplicate_sequence_is_rejected() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&descriptor) else {
        return false;
    };
    let entries = [entry, entry];
    sequence_error_is(
        &descriptor,
        &entries,
        entry.destination().sequence(),
        entry.destination().sequence(),
    )
}

fn reversed_sequence_is_rejected() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&descriptor) else {
        return false;
    };
    let Some(second) = degraded_drained_entry(&descriptor) else {
        return false;
    };
    let entries = [second, first];
    sequence_error_is(
        &descriptor,
        &entries,
        second.destination().sequence(),
        first.destination().sequence(),
    )
}

fn sequence_error_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    previous: EventSequence,
    current: EventSequence,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::SequenceNotIncreasing {
            previous: actual_previous,
            current: actual_current,
        }) => actual_previous == previous && actual_current == current,
        Err(_) | Ok(_) => false,
    }
}

fn report_error_cases() -> bool {
    record_capacity_error()
        && output_mismatch_error()
        && source_stream_mismatch_error()
        && record_byte_capacity_error()
        && source_byte_capacity_error()
        && unresolved_owner_errors()
}

fn record_capacity_error() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&base) else {
        return false;
    };
    let Some(second) = degraded_drained_entry(&base) else {
        return false;
    };
    let Some(descriptor) = report_descriptor(
        output_identity(),
        event::stream_identity(),
        1_024,
        1,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [first, second];
    let Some(requested) = checked_count(2) else {
        return false;
    };
    let Some(maximum) = checked_count(1) else {
        return false;
    };
    record_capacity_result_is(&descriptor, &entries, requested, maximum)
}

fn record_capacity_result_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    requested: CheckedCount,
    maximum: CheckedCount,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::RecordCapacityExceeded {
            requested: actual_requested,
            maximum: actual_maximum,
        }) => actual_requested == requested && actual_maximum == maximum,
        Err(_) | Ok(_) => false,
    }
}

fn output_mismatch_error() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&base) else {
        return false;
    };
    let expected = foreign_output();
    let Some(descriptor) = report_descriptor(
        expected,
        event::stream_identity(),
        1_024,
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [entry];
    output_mismatch_result_is(
        &descriptor,
        &entries,
        entry.destination().sequence(),
        expected,
        entry.destination().output(),
    )
}

fn output_mismatch_result_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    sequence: EventSequence,
    expected: LifecycleIdentity,
    actual: LifecycleIdentity,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::OutputMismatch {
            sequence: actual_sequence,
            expected: actual_expected,
            actual: actual_output,
        }) => {
            actual_sequence == sequence
                && actual_expected == expected
                && actual_output == actual
        }
        Err(_) | Ok(_) => false,
    }
}

fn source_stream_mismatch_error() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&base) else {
        return false;
    };
    let expected = foreign_stream();
    let Some(descriptor) = report_descriptor(
        output_identity(),
        expected,
        1_024,
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [entry];
    source_mismatch_result_is(
        &descriptor,
        &entries,
        entry.destination().sequence(),
        expected,
        entry.destination().stream(),
    )
}

fn source_mismatch_result_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    sequence: EventSequence,
    expected: LifecycleIdentity,
    actual: LifecycleIdentity,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::SourceStreamMismatch {
            sequence: actual_sequence,
            expected: actual_expected,
            actual: actual_stream,
        }) => {
            actual_sequence == sequence
                && actual_expected == expected
                && actual_stream == actual
        }
        Err(_) | Ok(_) => false,
    }
}

fn record_byte_capacity_error() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&base) else {
        return false;
    };
    let bytes = entry.destination().total_bytes();
    let Some(maximum_value) = bytes.value().checked_sub(1) else {
        return false;
    };
    let Some(descriptor) = report_descriptor(
        output_identity(),
        event::stream_identity(),
        maximum_value,
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let Some(maximum) = checked_bytes(maximum_value) else {
        return false;
    };
    let entries = [entry];
    record_byte_result_is(
        &descriptor,
        &entries,
        entry.destination().sequence(),
        bytes,
        maximum,
    )
}

fn record_byte_result_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    sequence: EventSequence,
    bytes: CheckedByteCount,
    maximum: CheckedByteCount,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::RecordByteCapacityExceeded {
            sequence: actual_sequence,
            bytes: actual_bytes,
            maximum: actual_maximum,
        }) => {
            actual_sequence == sequence
                && actual_bytes == bytes
                && actual_maximum == maximum
        }
        Err(_) | Ok(_) => false,
    }
}

fn source_byte_capacity_error() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&base) else {
        return false;
    };
    let Some(second) = degraded_drained_entry(&base) else {
        return false;
    };
    let accumulated = first.destination().total_bytes();
    let next = second.destination().total_bytes();
    let Ok(total) = accumulated.checked_add(next) else {
        return false;
    };
    let Some(maximum_value) = total.value().checked_sub(1) else {
        return false;
    };
    let maximum_record = accumulated.value().max(next.value());
    let Some(descriptor) = report_descriptor(
        output_identity(),
        event::stream_identity(),
        maximum_record,
        4,
        maximum_value,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let Some(maximum) = checked_bytes(maximum_value) else {
        return false;
    };
    let entries = [first, second];
    source_byte_result_is(
        &descriptor,
        &entries,
        second.destination().sequence(),
        accumulated,
        next,
        maximum,
    )
}

fn source_byte_result_is(
    descriptor: &DiagnosticOutputDescriptor,
    entries: &[DiagnosticShutdownEntry],
    sequence: EventSequence,
    accumulated: CheckedByteCount,
    next: CheckedByteCount,
    maximum: CheckedByteCount,
) -> bool {
    let result = DiagnosticShutdownReportRef::new(
        descriptor,
        DiagnosticOutputOwnerState::NoOwnedExecution,
        entries,
    );
    match result {
        Err(DiagnosticShutdownReportError::SourceByteCapacityExceeded {
            sequence: actual_sequence,
            accumulated: actual_accumulated,
            next: actual_next,
            maximum: actual_maximum,
        }) => {
            actual_sequence == sequence
                && actual_accumulated == accumulated
                && actual_next == next
                && actual_maximum == maximum
        }
        Err(_) | Ok(_) => false,
    }
}

fn unresolved_owner_errors() -> bool {
    let owners = [
        DiagnosticOutputOwnerState::NoOwnedExecution,
        DiagnosticOutputOwnerState::ExitedOwned,
        DiagnosticOutputOwnerState::JoinedOwned,
        DiagnosticOutputOwnerState::ContainingProcessReaped,
    ];
    owners.into_iter().all(unresolved_owner_error)
}

fn unresolved_owner_error(owner: DiagnosticOutputOwnerState) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_unresolved_entry(&descriptor) else {
        return false;
    };
    let entries = [entry];
    let Some(unresolved) = checked_count(1) else {
        return false;
    };
    let result = DiagnosticShutdownReportRef::new(&descriptor, owner, &entries);
    match result {
        Err(DiagnosticShutdownReportError::UnresolvedOwnerMismatch {
            owner: actual_owner,
            unresolved: actual_unresolved,
        }) => actual_owner == owner && actual_unresolved == unresolved,
        Err(_) | Ok(_) => false,
    }
}

const fn report_descriptor(
    output: LifecycleIdentity,
    stream: LifecycleIdentity,
    maximum_record_bytes: u64,
    maximum_records: u64,
    maximum_source_bytes: u64,
    policy: DiagnosticShutdownPolicy,
) -> Option<DiagnosticOutputDescriptor> {
    make_descriptor_with(
        output,
        stream,
        maximum_record_bytes,
        maximum_records,
        maximum_source_bytes,
        policy,
    )
}

const fn foreign_output() -> LifecycleIdentity {
    let current = output_identity();
    LifecycleIdentity::new(
        current.instance() + 1,
        current.incarnation(),
        current.generation(),
    )
}

const fn foreign_stream() -> LifecycleIdentity {
    let current = event::stream_identity();
    LifecycleIdentity::new(
        current.instance(),
        current.incarnation(),
        Generation::new(current.generation().value() + 1),
    )
}

fn checked_count(value: u64) -> Option<CheckedCount> {
    CheckedCount::new(value, ZeroPolicy::Allow).ok()
}

fn checked_bytes(value: u64) -> Option<CheckedByteCount> {
    CheckedByteCount::new(value, ZeroPolicy::Allow).ok()
}

fn report_validation_order() -> bool {
    capacity_precedes_traversal()
        && output_precedes_source()
        && source_precedes_record_bytes()
        && record_bytes_precede_sequence()
        && sequence_precedes_policy()
        && policy_precedes_aggregate()
        && aggregate_precedes_owner()
        && first_entry_precedes_later_error()
}

fn capacity_precedes_traversal() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&base) else {
        return false;
    };
    let Some(second) = degraded_drained_entry(&base) else {
        return false;
    };
    let Some(descriptor) = report_descriptor(
        foreign_output(),
        foreign_stream(),
        1,
        1,
        1,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [first, second];
    let Some(requested) = checked_count(2) else {
        return false;
    };
    let Some(maximum) = checked_count(1) else {
        return false;
    };
    record_capacity_result_is(&descriptor, &entries, requested, maximum)
}

fn output_precedes_source() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&base) else {
        return false;
    };
    let expected = foreign_output();
    let Some(descriptor) = report_descriptor(
        expected,
        foreign_stream(),
        1_024,
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [entry];
    output_mismatch_result_is(
        &descriptor,
        &entries,
        entry.destination().sequence(),
        expected,
        entry.destination().output(),
    )
}

fn source_precedes_record_bytes() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(entry) = ready_drained_entry(&base) else {
        return false;
    };
    let bytes = entry.destination().total_bytes();
    let Some(maximum) = bytes.value().checked_sub(1) else {
        return false;
    };
    let expected = foreign_stream();
    let Some(descriptor) = report_descriptor(
        output_identity(),
        expected,
        maximum,
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [entry];
    source_mismatch_result_is(
        &descriptor,
        &entries,
        entry.destination().sequence(),
        expected,
        entry.destination().stream(),
    )
}

fn record_bytes_precede_sequence() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(first) = failed_drained_entry(&base) else {
        return false;
    };
    let Some(second) = degraded_drained_entry(&base) else {
        return false;
    };
    let bytes = second.destination().total_bytes();
    let maximum = first.destination().total_bytes();
    if bytes.value() <= maximum.value() {
        return false;
    }
    let Some(descriptor) = report_descriptor(
        output_identity(),
        event::stream_identity(),
        maximum.value(),
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let entries = [first, second];
    record_byte_result_is(
        &descriptor,
        &entries,
        second.destination().sequence(),
        bytes,
        maximum,
    )
}

fn sequence_precedes_policy() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(first) = failed_drained_entry(&descriptor) else {
        return false;
    };
    let Some(second) = degraded_discarded_entry(
        &descriptor,
        DiagnosticDestinationCause::ClosingDiscard,
    ) else {
        return false;
    };
    let entries = [first, second];
    sequence_error_is(
        &descriptor,
        &entries,
        first.destination().sequence(),
        second.destination().sequence(),
    )
}

fn policy_precedes_aggregate() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&base) else {
        return false;
    };
    let cause = DiagnosticDestinationCause::ClosingDiscard;
    let Some(second) = degraded_discarded_entry(&base, cause) else {
        return false;
    };
    let Some(descriptor) = aggregate_limited_descriptor(first, second) else {
        return false;
    };
    let entries = [first, second];
    policy_error_at_is(
        &descriptor,
        &entries,
        DiagnosticShutdownPolicy::Drain,
        cause,
        second.destination().sequence(),
    )
}

fn aggregate_precedes_owner() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let Some(first) = ready_drained_entry(&base) else {
        return false;
    };
    let Some(second) = degraded_unresolved_entry(&base) else {
        return false;
    };
    let Some(descriptor) = aggregate_limited_descriptor(first, second) else {
        return false;
    };
    let accumulated = first.destination().total_bytes();
    let next = second.destination().total_bytes();
    let Ok(total) = accumulated.checked_add(next) else {
        return false;
    };
    let Some(maximum_value) = total.value().checked_sub(1) else {
        return false;
    };
    let Some(maximum) = checked_bytes(maximum_value) else {
        return false;
    };
    let entries = [first, second];
    source_byte_result_is(
        &descriptor,
        &entries,
        second.destination().sequence(),
        accumulated,
        next,
        maximum,
    )
}

fn aggregate_limited_descriptor(
    first: DiagnosticShutdownEntry,
    second: DiagnosticShutdownEntry,
) -> Option<DiagnosticOutputDescriptor> {
    let first_bytes = first.destination().total_bytes();
    let second_bytes = second.destination().total_bytes();
    let total = first_bytes.checked_add(second_bytes).ok()?;
    let maximum_source = total.value().checked_sub(1)?;
    let maximum_record = first_bytes.value().max(second_bytes.value());
    report_descriptor(
        output_identity(),
        event::stream_identity(),
        maximum_record,
        4,
        maximum_source,
        DiagnosticShutdownPolicy::Drain,
    )
}

fn first_entry_precedes_later_error() -> bool {
    let Some(base) = make_descriptor() else {
        return false;
    };
    let cause = DiagnosticDestinationCause::ClosingDiscard;
    let Some(first) = ready_discarded_entry(&base, cause) else {
        return false;
    };
    let Some(foreign) = report_descriptor(
        foreign_output(),
        event::stream_identity(),
        1_024,
        4,
        4_096,
        DiagnosticShutdownPolicy::Drain,
    ) else {
        return false;
    };
    let Some(second) = degraded_drained_entry(&foreign) else {
        return false;
    };
    let entries = [first, second];
    policy_error_is(&base, &entries, DiagnosticShutdownPolicy::Drain, cause)
}

fn report_error_trait_cases() -> bool {
    let Some(one_count) = checked_count(1) else {
        return false;
    };
    let Some(two_count) = checked_count(2) else {
        return false;
    };
    let Some(one_byte) = checked_bytes(1) else {
        return false;
    };
    let Some(two_bytes) = checked_bytes(2) else {
        return false;
    };
    let first = EventSequence::first();
    let Ok(second) = first.next() else {
        return false;
    };
    let errors = [
        DiagnosticShutdownReportError::EntryCountUnrepresentable,
        DiagnosticShutdownReportError::RecordCapacityExceeded {
            requested: two_count,
            maximum: one_count,
        },
        DiagnosticShutdownReportError::OutputMismatch {
            sequence: first,
            expected: output_identity(),
            actual: foreign_output(),
        },
        DiagnosticShutdownReportError::SourceStreamMismatch {
            sequence: first,
            expected: event::stream_identity(),
            actual: foreign_stream(),
        },
        DiagnosticShutdownReportError::RecordByteCapacityExceeded {
            sequence: first,
            bytes: two_bytes,
            maximum: one_byte,
        },
        DiagnosticShutdownReportError::SequenceNotIncreasing {
            previous: second,
            current: first,
        },
        DiagnosticShutdownReportError::PolicyDiscardMismatch {
            sequence: first,
            policy: DiagnosticShutdownPolicy::Drain,
            cause: DiagnosticDestinationCause::ClosingDiscard,
        },
        DiagnosticShutdownReportError::SourceByteCapacityExceeded {
            sequence: second,
            accumulated: one_byte,
            next: two_bytes,
            maximum: two_bytes,
        },
        DiagnosticShutdownReportError::UnresolvedOwnerMismatch {
            owner: DiagnosticOutputOwnerState::JoinedOwned,
            unresolved: one_count,
        },
    ];
    errors.into_iter().all(report_error_has_traits)
}

fn report_error_has_traits(error: DiagnosticShutdownReportError) -> bool {
    require_debug(&error);
    traits(error) && error_traits(&error)
}
