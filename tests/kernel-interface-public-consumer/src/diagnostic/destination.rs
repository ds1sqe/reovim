//! Destination-result value matrix through the public contract.

use alloc::string::ToString;
use reovim_kernel_interface::bound::{CheckedByteCount, ZeroPolicy};
use reovim_kernel_interface::diagnostic::{
    DiagnosticDestinationCause, DiagnosticDestinationCut,
    DiagnosticDestinationError, DiagnosticDestinationResult,
    DiagnosticOutputRecord,
};
use reovim_kernel_interface::effect::EffectCertainty;

use super::{make_descriptor, make_record};
use crate::{error_traits, event, require_debug, traits};

pub const fn const_cases() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCut,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationResult,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::diagnostic::DiagnosticDestinationError,
    >() && all_cut_variants()
        && all_cause_variants()
}

const _: fn(
    &DiagnosticOutputRecord,
    CheckedByteCount,
    EffectCertainty,
    DiagnosticDestinationCause,
    DiagnosticDestinationCut,
)
    -> Result<DiagnosticDestinationResult, DiagnosticDestinationError> =
    use_result_const_api;

const fn use_result_const_api(
    record: &DiagnosticOutputRecord,
    confirmed: CheckedByteCount,
    certainty: EffectCertainty,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> Result<DiagnosticDestinationResult, DiagnosticDestinationError> {
    let result = match DiagnosticDestinationResult::new(
        record, confirmed, certainty, cause, cut,
    ) {
        Ok(value) => value,
        Err(error) => return Err(error),
    };
    let _metadata = (
        result.output(),
        result.stream(),
        result.sequence(),
        result.total_bytes(),
        result.confirmed_prefix(),
        result.certainty(),
        result.cause(),
        result.cut(),
    );
    Ok(result)
}

const fn all_cut_variants() -> bool {
    cut_code(DiagnosticDestinationCut::BeforeCall) == 0
        && cut_code(DiagnosticDestinationCut::InCall) == 1
        && cut_code(DiagnosticDestinationCut::Returned) == 2
}

const fn cut_code(value: DiagnosticDestinationCut) -> u8 {
    match value {
        DiagnosticDestinationCut::BeforeCall => 0,
        DiagnosticDestinationCut::InCall => 1,
        DiagnosticDestinationCut::Returned => 2,
    }
}

const fn all_cause_variants() -> bool {
    cause_code(DiagnosticDestinationCause::Success) == 0
        && cause_code(DiagnosticDestinationCause::ClosingDiscard) == 1
        && cause_code(DiagnosticDestinationCause::ReplacementDiscard) == 2
        && cause_code(DiagnosticDestinationCause::Cancellation) == 3
        && cause_code(DiagnosticDestinationCause::ShutdownDeadline) == 4
        && cause_code(DiagnosticDestinationCause::WriteZero) == 5
        && cause_code(DiagnosticDestinationCause::WriteFailure) == 6
        && cause_code(DiagnosticDestinationCause::RetryExhaustion) == 7
        && cause_code(DiagnosticDestinationCause::WriterContractViolation) == 8
        && cause_code(DiagnosticDestinationCause::InternalFailureBeforeCall)
            == 9
        && cause_code(DiagnosticDestinationCause::InternalFailureInCall) == 10
        && cause_code(DiagnosticDestinationCause::SupervisorReaping) == 11
}

const fn cause_code(value: DiagnosticDestinationCause) -> u8 {
    match value {
        DiagnosticDestinationCause::Success => 0,
        DiagnosticDestinationCause::ClosingDiscard => 1,
        DiagnosticDestinationCause::ReplacementDiscard => 2,
        DiagnosticDestinationCause::Cancellation => 3,
        DiagnosticDestinationCause::ShutdownDeadline => 4,
        DiagnosticDestinationCause::WriteZero => 5,
        DiagnosticDestinationCause::WriteFailure => 6,
        DiagnosticDestinationCause::RetryExhaustion => 7,
        DiagnosticDestinationCause::WriterContractViolation => 8,
        DiagnosticDestinationCause::InternalFailureBeforeCall => 9,
        DiagnosticDestinationCause::InternalFailureInCall => 10,
        DiagnosticDestinationCause::SupervisorReaping => 11,
    }
}

pub fn exercise() -> bool {
    valid_matrix()
        && complete_relation_matrix()
        && identity_and_no_borrow_case()
        && adjacent_error_cases()
        && validation_order_cases()
        && error_trait_cases()
}

#[derive(Clone, Copy)]
struct Case {
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
    prefix: Prefix,
}

#[derive(Clone, Copy)]
enum Prefix {
    Zero,
    Partial,
    Full,
    Exceeds,
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

const PREFIXES: [Prefix; 4] =
    [Prefix::Zero, Prefix::Partial, Prefix::Full, Prefix::Exceeds];

fn valid_matrix() -> bool {
    terminal_cases()
        && interruption_cases(DiagnosticDestinationCause::Cancellation)
        && interruption_cases(DiagnosticDestinationCause::ShutdownDeadline)
        && returned_failure_cases(DiagnosticDestinationCause::WriteZero)
        && returned_failure_cases(DiagnosticDestinationCause::WriteFailure)
        && returned_failure_cases(DiagnosticDestinationCause::RetryExhaustion)
        && writer_contract_cases()
        && internal_before_call_cases()
        && internal_in_call_cases()
        && interruption_cases(DiagnosticDestinationCause::SupervisorReaping)
}

fn complete_relation_matrix() -> bool {
    CAUSES.into_iter().all(complete_cause_matrix)
}

fn complete_cause_matrix(cause: DiagnosticDestinationCause) -> bool {
    CUTS.into_iter().all(|cut| complete_cut_matrix(cause, cut))
}

fn complete_cut_matrix(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> bool {
    CERTAINTIES
        .into_iter()
        .all(|certainty| complete_certainty_matrix(cause, cut, certainty))
}

fn complete_certainty_matrix(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
) -> bool {
    PREFIXES.into_iter().all(|prefix| {
        complete_matrix_case(Case { cause, cut, certainty, prefix })
    })
}

fn complete_matrix_case(case: Case) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let Some(confirmed) = prefix_for(&record, case.prefix) else {
        return false;
    };
    let result = DiagnosticDestinationResult::new(
        &record,
        confirmed,
        case.certainty,
        case.cause,
        case.cut,
    );
    let context = ErrorContext {
        confirmed,
        total: record.size(),
        certainty: case.certainty,
        cause: case.cause,
        cut: case.cut,
    };
    expected_error(case).map_or_else(
        || {
            result.is_ok_and(|value| {
                result_matches(&value, &record, case, confirmed)
            })
        },
        |expected| matches_expected_error(result, expected, context),
    )
}

const fn expected_error(case: Case) -> Option<ErrorKind> {
    match case.prefix {
        Prefix::Exceeds => return Some(ErrorKind::PrefixBound),
        Prefix::Zero | Prefix::Partial | Prefix::Full => {}
    }
    if !cause_cut_valid(case.cause, case.cut) {
        return Some(ErrorKind::CauseCut);
    }
    if !certainty_valid(case.cause, case.cut, case.certainty) {
        return Some(ErrorKind::EffectCertainty);
    }
    if !prefix_valid(case) {
        return Some(ErrorKind::Prefix);
    }
    None
}

const fn cause_cut_valid(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> bool {
    match cause {
        DiagnosticDestinationCause::Success
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::WriterContractViolation => {
            cut_is_returned(cut)
        }
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard => {
            cut_is_before_call(cut)
        }
        DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::SupervisorReaping => true,
        DiagnosticDestinationCause::InternalFailureBeforeCall => {
            cut_is_before_call_or_returned(cut)
        }
        DiagnosticDestinationCause::InternalFailureInCall => {
            cut_is_in_call(cut)
        }
    }
}

const fn cut_is_returned(cut: DiagnosticDestinationCut) -> bool {
    match cut {
        DiagnosticDestinationCut::Returned => {}
        DiagnosticDestinationCut::BeforeCall
        | DiagnosticDestinationCut::InCall => return false,
    }
    true
}

const fn cut_is_before_call(cut: DiagnosticDestinationCut) -> bool {
    match cut {
        DiagnosticDestinationCut::BeforeCall => {}
        DiagnosticDestinationCut::InCall
        | DiagnosticDestinationCut::Returned => return false,
    }
    true
}

const fn cut_is_before_call_or_returned(cut: DiagnosticDestinationCut) -> bool {
    match cut {
        DiagnosticDestinationCut::BeforeCall
        | DiagnosticDestinationCut::Returned => {}
        DiagnosticDestinationCut::InCall => return false,
    }
    true
}

const fn cut_is_in_call(cut: DiagnosticDestinationCut) -> bool {
    match cut {
        DiagnosticDestinationCut::InCall => {}
        DiagnosticDestinationCut::BeforeCall
        | DiagnosticDestinationCut::Returned => return false,
    }
    true
}

const fn certainty_valid(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
) -> bool {
    match cut {
        DiagnosticDestinationCut::BeforeCall => {
            certainty_is_definitely_not_applied(certainty)
        }
        DiagnosticDestinationCut::InCall => {
            certainty_is_outcome_unknown(certainty)
        }
        DiagnosticDestinationCut::Returned => {
            returned_certainty_valid(cause, certainty)
        }
    }
}

const fn returned_certainty_valid(
    cause: DiagnosticDestinationCause,
    certainty: EffectCertainty,
) -> bool {
    match cause {
        DiagnosticDestinationCause::Success => certainty_is_applied(certainty),
        DiagnosticDestinationCause::WriterContractViolation => {
            certainty_is_outcome_unknown(certainty)
        }
        DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::SupervisorReaping => {
            certainty_is_not_applied_or_unknown(certainty)
        }
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::InternalFailureInCall => false,
    }
}

const fn certainty_is_applied(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::Applied => {}
        EffectCertainty::DefinitelyNotApplied
        | EffectCertainty::OutcomeUnknown => return false,
    }
    true
}

const fn certainty_is_definitely_not_applied(
    certainty: EffectCertainty,
) -> bool {
    match certainty {
        EffectCertainty::DefinitelyNotApplied => {}
        EffectCertainty::Applied | EffectCertainty::OutcomeUnknown => {
            return false;
        }
    }
    true
}

const fn certainty_is_outcome_unknown(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::OutcomeUnknown => {}
        EffectCertainty::Applied | EffectCertainty::DefinitelyNotApplied => {
            return false;
        }
    }
    true
}

const fn certainty_is_not_applied_or_unknown(
    certainty: EffectCertainty,
) -> bool {
    match certainty {
        EffectCertainty::DefinitelyNotApplied
        | EffectCertainty::OutcomeUnknown => {}
        EffectCertainty::Applied => return false,
    }
    true
}

const fn prefix_valid(case: Case) -> bool {
    match case.certainty {
        EffectCertainty::Applied => prefix_is_full(case.prefix),
        EffectCertainty::DefinitelyNotApplied => prefix_is_zero(case.prefix),
        EffectCertainty::OutcomeUnknown => unknown_prefix_valid(case),
    }
}

const fn prefix_is_full(prefix: Prefix) -> bool {
    match prefix {
        Prefix::Full => {}
        Prefix::Zero | Prefix::Partial | Prefix::Exceeds => return false,
    }
    true
}

const fn prefix_is_zero(prefix: Prefix) -> bool {
    match prefix {
        Prefix::Zero => {}
        Prefix::Partial | Prefix::Full | Prefix::Exceeds => return false,
    }
    true
}

const fn unknown_prefix_valid(case: Case) -> bool {
    match case.prefix {
        Prefix::Partial => true,
        Prefix::Zero => {
            cut_is_in_call(case.cut) || writer_contract_returned(case)
        }
        Prefix::Full | Prefix::Exceeds => false,
    }
}

const fn writer_contract_returned(case: Case) -> bool {
    match (case.cause, case.cut) {
        (
            DiagnosticDestinationCause::WriterContractViolation,
            DiagnosticDestinationCut::Returned,
        ) => {}
        _ => return false,
    }
    true
}

fn terminal_cases() -> bool {
    let cases = [
        Case {
            cause: DiagnosticDestinationCause::Success,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::Applied,
            prefix: Prefix::Full,
        },
        Case {
            cause: DiagnosticDestinationCause::ClosingDiscard,
            cut: DiagnosticDestinationCut::BeforeCall,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
        Case {
            cause: DiagnosticDestinationCause::ReplacementDiscard,
            cut: DiagnosticDestinationCut::BeforeCall,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
    ];
    cases.into_iter().all(valid_case)
}

fn interruption_cases(cause: DiagnosticDestinationCause) -> bool {
    let cases = [
        Case {
            cause,
            cut: DiagnosticDestinationCut::BeforeCall,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
        Case {
            cause,
            cut: DiagnosticDestinationCut::InCall,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Zero,
        },
        Case {
            cause,
            cut: DiagnosticDestinationCut::InCall,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Partial,
        },
        Case {
            cause,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
        Case {
            cause,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Partial,
        },
    ];
    cases.into_iter().all(valid_case)
}

fn returned_failure_cases(cause: DiagnosticDestinationCause) -> bool {
    let cases = [
        Case {
            cause,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
        Case {
            cause,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Partial,
        },
    ];
    cases.into_iter().all(valid_case)
}

fn writer_contract_cases() -> bool {
    let cases = [
        Case {
            cause: DiagnosticDestinationCause::WriterContractViolation,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Zero,
        },
        Case {
            cause: DiagnosticDestinationCause::WriterContractViolation,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Partial,
        },
    ];
    cases.into_iter().all(valid_case)
}

fn internal_before_call_cases() -> bool {
    let cases = [
        Case {
            cause: DiagnosticDestinationCause::InternalFailureBeforeCall,
            cut: DiagnosticDestinationCut::BeforeCall,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
        Case {
            cause: DiagnosticDestinationCause::InternalFailureBeforeCall,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::DefinitelyNotApplied,
            prefix: Prefix::Zero,
        },
        Case {
            cause: DiagnosticDestinationCause::InternalFailureBeforeCall,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Partial,
        },
    ];
    cases.into_iter().all(valid_case)
}

fn internal_in_call_cases() -> bool {
    let cases = [
        Case {
            cause: DiagnosticDestinationCause::InternalFailureInCall,
            cut: DiagnosticDestinationCut::InCall,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Zero,
        },
        Case {
            cause: DiagnosticDestinationCause::InternalFailureInCall,
            cut: DiagnosticDestinationCut::InCall,
            certainty: EffectCertainty::OutcomeUnknown,
            prefix: Prefix::Partial,
        },
    ];
    cases.into_iter().all(valid_case)
}

fn valid_case(case: Case) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let Some(prefix) = prefix_for(&record, case.prefix) else {
        return false;
    };
    let Ok(result) = DiagnosticDestinationResult::new(
        &record,
        prefix,
        case.certainty,
        case.cause,
        case.cut,
    ) else {
        return false;
    };
    result_matches(&result, &record, case, prefix)
}

fn prefix_for(
    record: &DiagnosticOutputRecord,
    prefix: Prefix,
) -> Option<CheckedByteCount> {
    let value = match prefix {
        Prefix::Zero => 0,
        Prefix::Partial => record.size().value().checked_sub(1)?,
        Prefix::Full => record.size().value(),
        Prefix::Exceeds => record.size().value().checked_add(1)?,
    };
    CheckedByteCount::new(value, ZeroPolicy::Allow).ok()
}

fn result_matches(
    result: &DiagnosticDestinationResult,
    record: &DiagnosticOutputRecord,
    case: Case,
    prefix: CheckedByteCount,
) -> bool {
    let exact_total = result.total_bytes() == record.size();
    result.output() == record.output()
        && result.stream() == record.stream()
        && result.sequence() == record.sequence()
        && exact_total
        && result.confirmed_prefix() == prefix
        && result.certainty() == case.certainty
        && result.cause() == case.cause
        && result.cut() == case.cut
        && traits(*result)
}

fn identity_and_no_borrow_case() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let total = record.size();
    let Ok(result) = DiagnosticDestinationResult::new(
        &record,
        total,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
    ) else {
        return false;
    };
    let moved_record = record;
    let identities_match = result.output() == moved_record.output()
        && result.stream() == moved_record.stream()
        && result.sequence() == moved_record.sequence();
    identities_match
        && result.total_bytes() == moved_record.size()
        && moved_record.bytes() == event::READY_LINE
}

fn adjacent_error_cases() -> bool {
    success_neighbors()
        && discard_neighbors()
        && interruption_neighbors()
        && returned_failure_neighbors()
        && writer_contract_neighbors()
        && internal_failure_neighbors()
}

fn success_neighbors() -> bool {
    error_case(
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::BeforeCall,
        EffectCertainty::Applied,
        Prefix::Full,
        ErrorKind::CauseCut,
    ) && error_case(
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::OutcomeUnknown,
        Prefix::Full,
        ErrorKind::EffectCertainty,
    ) && error_case(
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::Applied,
        Prefix::Partial,
        ErrorKind::Prefix,
    )
}

fn discard_neighbors() -> bool {
    error_case(
        DiagnosticDestinationCause::ClosingDiscard,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Zero,
        ErrorKind::CauseCut,
    ) && error_case(
        DiagnosticDestinationCause::ReplacementDiscard,
        DiagnosticDestinationCut::BeforeCall,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    ) && error_case(
        DiagnosticDestinationCause::ClosingDiscard,
        DiagnosticDestinationCut::BeforeCall,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Partial,
        ErrorKind::Prefix,
    )
}

fn interruption_neighbors() -> bool {
    interruption_neighbor_cases(DiagnosticDestinationCause::Cancellation)
        && interruption_neighbor_cases(
            DiagnosticDestinationCause::ShutdownDeadline,
        )
        && interruption_neighbor_cases(
            DiagnosticDestinationCause::SupervisorReaping,
        )
}

fn interruption_neighbor_cases(cause: DiagnosticDestinationCause) -> bool {
    error_case(
        cause,
        DiagnosticDestinationCut::BeforeCall,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    ) && error_case(
        cause,
        DiagnosticDestinationCut::InCall,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    ) && error_case(
        cause,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::Prefix,
    )
}

fn returned_failure_neighbors() -> bool {
    returned_failure_neighbor_cases(DiagnosticDestinationCause::WriteZero)
        && returned_failure_neighbor_cases(
            DiagnosticDestinationCause::WriteFailure,
        )
        && returned_failure_neighbor_cases(
            DiagnosticDestinationCause::RetryExhaustion,
        )
}

fn returned_failure_neighbor_cases(cause: DiagnosticDestinationCause) -> bool {
    error_case(
        cause,
        DiagnosticDestinationCut::InCall,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::CauseCut,
    ) && error_case(
        cause,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::Applied,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    ) && error_case(
        cause,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Partial,
        ErrorKind::Prefix,
    )
}

fn writer_contract_neighbors() -> bool {
    error_case(
        DiagnosticDestinationCause::WriterContractViolation,
        DiagnosticDestinationCut::InCall,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::CauseCut,
    ) && error_case(
        DiagnosticDestinationCause::WriterContractViolation,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    ) && error_case(
        DiagnosticDestinationCause::WriterContractViolation,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::OutcomeUnknown,
        Prefix::Full,
        ErrorKind::Prefix,
    )
}

fn internal_failure_neighbors() -> bool {
    error_case(
        DiagnosticDestinationCause::InternalFailureBeforeCall,
        DiagnosticDestinationCut::InCall,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::CauseCut,
    ) && error_case(
        DiagnosticDestinationCause::InternalFailureInCall,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::CauseCut,
    ) && error_case(
        DiagnosticDestinationCause::InternalFailureInCall,
        DiagnosticDestinationCut::InCall,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    )
}

#[derive(Clone, Copy)]
enum ErrorKind {
    PrefixBound,
    CauseCut,
    EffectCertainty,
    Prefix,
}

fn error_case(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
    prefix: Prefix,
    expected: ErrorKind,
) -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let Some(confirmed) = prefix_for(&record, prefix) else {
        return false;
    };
    let result = DiagnosticDestinationResult::new(
        &record, confirmed, certainty, cause, cut,
    );
    matches_expected_error(
        result,
        expected,
        ErrorContext { confirmed, total: record.size(), certainty, cause, cut },
    )
}

#[derive(Clone, Copy)]
struct ErrorContext {
    confirmed: CheckedByteCount,
    total: CheckedByteCount,
    certainty: EffectCertainty,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
}

fn matches_expected_error(
    result: Result<DiagnosticDestinationResult, DiagnosticDestinationError>,
    expected: ErrorKind,
    context: ErrorContext,
) -> bool {
    let Err(error) = result else { return false };
    match (expected, error) {
        (
            ErrorKind::PrefixBound,
            DiagnosticDestinationError::PrefixExceedsRecord {
                confirmed,
                total,
            },
        ) => confirmed == context.confirmed && total == context.total,
        (
            ErrorKind::CauseCut,
            DiagnosticDestinationError::CauseCutMismatch { cause, cut },
        ) => cause == context.cause && cut == context.cut,
        (
            ErrorKind::EffectCertainty,
            DiagnosticDestinationError::CertaintyMismatch {
                cause,
                cut,
                certainty,
            },
        ) => {
            cause == context.cause
                && cut == context.cut
                && certainty == context.certainty
        }
        (
            ErrorKind::Prefix,
            DiagnosticDestinationError::PrefixMismatch {
                cause,
                cut,
                confirmed,
                total,
            },
        ) => {
            cause == context.cause
                && cut == context.cut
                && confirmed == context.confirmed
                && total == context.total
        }
        (
            ErrorKind::PrefixBound
            | ErrorKind::CauseCut
            | ErrorKind::EffectCertainty
            | ErrorKind::Prefix,
            _,
        ) => false,
    }
}

fn validation_order_cases() -> bool {
    prefix_bound_precedes_relations()
        && cause_cut_precedes_certainty()
        && certainty_precedes_prefix()
}

fn prefix_bound_precedes_relations() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let Ok(one) = CheckedByteCount::new(1, ZeroPolicy::Reject) else {
        return false;
    };
    let Ok(confirmed) = record.size().checked_add(one) else {
        return false;
    };
    let result = DiagnosticDestinationResult::new(
        &record,
        confirmed,
        EffectCertainty::Applied,
        DiagnosticDestinationCause::ClosingDiscard,
        DiagnosticDestinationCut::Returned,
    );
    match result {
        Err(DiagnosticDestinationError::PrefixExceedsRecord {
            confirmed: actual,
            total,
        }) => actual == confirmed && total == record.size(),
        Err(_) | Ok(_) => false,
    }
}

fn cause_cut_precedes_certainty() -> bool {
    error_case(
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::BeforeCall,
        EffectCertainty::DefinitelyNotApplied,
        Prefix::Zero,
        ErrorKind::CauseCut,
    )
}

fn certainty_precedes_prefix() -> bool {
    error_case(
        DiagnosticDestinationCause::Success,
        DiagnosticDestinationCut::Returned,
        EffectCertainty::OutcomeUnknown,
        Prefix::Zero,
        ErrorKind::EffectCertainty,
    )
}

fn error_trait_cases() -> bool {
    let Some(descriptor) = make_descriptor() else {
        return false;
    };
    let Some(record) = make_record(&descriptor) else {
        return false;
    };
    let Some(partial) = prefix_for(&record, Prefix::Partial) else {
        return false;
    };
    let errors = [
        DiagnosticDestinationError::PrefixExceedsRecord {
            confirmed: record.size(),
            total: partial,
        },
        DiagnosticDestinationError::CauseCutMismatch {
            cause: DiagnosticDestinationCause::Success,
            cut: DiagnosticDestinationCut::BeforeCall,
        },
        DiagnosticDestinationError::CertaintyMismatch {
            cause: DiagnosticDestinationCause::Success,
            cut: DiagnosticDestinationCut::Returned,
            certainty: EffectCertainty::OutcomeUnknown,
        },
        DiagnosticDestinationError::PrefixMismatch {
            cause: DiagnosticDestinationCause::Success,
            cut: DiagnosticDestinationCut::Returned,
            confirmed: partial,
            total: record.size(),
        },
    ];
    errors.into_iter().all(error_has_traits)
}

fn error_has_traits(error: DiagnosticDestinationError) -> bool {
    require_debug(&error);
    traits(error) && error_traits(&error) && !error.to_string().is_empty()
}
