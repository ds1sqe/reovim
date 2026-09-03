use crate::effect::EffectCertainty;

use super::{
    CheckedByteCount, DiagnosticDestinationCause, DiagnosticDestinationCut,
    DiagnosticDestinationError,
};

pub(super) const fn validate(
    confirmed: CheckedByteCount,
    total: CheckedByteCount,
    certainty: EffectCertainty,
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> Result<(), DiagnosticDestinationError> {
    if confirmed.value() > total.value() {
        return Err(DiagnosticDestinationError::PrefixExceedsRecord {
            confirmed,
            total,
        });
    }
    if !cause_matches_cut(cause, cut) {
        return Err(DiagnosticDestinationError::CauseCutMismatch {
            cause,
            cut,
        });
    }
    if !certainty_matches(cause, cut, certainty) {
        return Err(DiagnosticDestinationError::CertaintyMismatch {
            cause,
            cut,
            certainty,
        });
    }
    if !prefix_matches(cause, cut, certainty, confirmed, total) {
        return Err(DiagnosticDestinationError::PrefixMismatch {
            cause,
            cut,
            confirmed,
            total,
        });
    }
    Ok(())
}

const fn cause_matches_cut(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
) -> bool {
    match (cause, cut) {
        (
            DiagnosticDestinationCause::Success
            | DiagnosticDestinationCause::WriteZero
            | DiagnosticDestinationCause::WriteFailure
            | DiagnosticDestinationCause::RetryExhaustion
            | DiagnosticDestinationCause::WriterContractViolation
            | DiagnosticDestinationCause::InternalFailureBeforeCall,
            DiagnosticDestinationCut::Returned,
        )
        | (
            DiagnosticDestinationCause::ClosingDiscard
            | DiagnosticDestinationCause::ReplacementDiscard
            | DiagnosticDestinationCause::InternalFailureBeforeCall,
            DiagnosticDestinationCut::BeforeCall,
        )
        | (
            DiagnosticDestinationCause::Cancellation
            | DiagnosticDestinationCause::ShutdownDeadline
            | DiagnosticDestinationCause::SupervisorReaping,
            _,
        )
        | (
            DiagnosticDestinationCause::InternalFailureInCall,
            DiagnosticDestinationCut::InCall,
        ) => {}
        _ => return false,
    }
    true
}

const fn certainty_matches(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
) -> bool {
    match cut {
        DiagnosticDestinationCut::BeforeCall => {
            is_definitely_not_applied(certainty)
        }
        DiagnosticDestinationCut::InCall => is_outcome_unknown(certainty),
        DiagnosticDestinationCut::Returned => {
            returned_certainty_matches(cause, certainty)
        }
    }
}

const fn returned_certainty_matches(
    cause: DiagnosticDestinationCause,
    certainty: EffectCertainty,
) -> bool {
    match cause {
        DiagnosticDestinationCause::Success => is_applied(certainty),
        DiagnosticDestinationCause::WriterContractViolation => {
            is_outcome_unknown(certainty)
        }
        DiagnosticDestinationCause::Cancellation
        | DiagnosticDestinationCause::ShutdownDeadline
        | DiagnosticDestinationCause::WriteZero
        | DiagnosticDestinationCause::WriteFailure
        | DiagnosticDestinationCause::RetryExhaustion
        | DiagnosticDestinationCause::InternalFailureBeforeCall
        | DiagnosticDestinationCause::SupervisorReaping => {
            is_not_applied_or_unknown(certainty)
        }
        DiagnosticDestinationCause::ClosingDiscard
        | DiagnosticDestinationCause::ReplacementDiscard
        | DiagnosticDestinationCause::InternalFailureInCall => false,
    }
}

const fn is_applied(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::Applied => {}
        EffectCertainty::DefinitelyNotApplied
        | EffectCertainty::OutcomeUnknown => return false,
    }
    true
}

const fn is_definitely_not_applied(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::DefinitelyNotApplied => {}
        EffectCertainty::Applied | EffectCertainty::OutcomeUnknown => {
            return false;
        }
    }
    true
}

const fn is_outcome_unknown(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::OutcomeUnknown => {}
        EffectCertainty::Applied | EffectCertainty::DefinitelyNotApplied => {
            return false;
        }
    }
    true
}

const fn is_not_applied_or_unknown(certainty: EffectCertainty) -> bool {
    match certainty {
        EffectCertainty::DefinitelyNotApplied
        | EffectCertainty::OutcomeUnknown => {}
        EffectCertainty::Applied => return false,
    }
    true
}

const fn prefix_matches(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    certainty: EffectCertainty,
    confirmed: CheckedByteCount,
    total: CheckedByteCount,
) -> bool {
    match certainty {
        EffectCertainty::Applied => confirmed.value() == total.value(),
        EffectCertainty::DefinitelyNotApplied => confirmed.value() == 0,
        EffectCertainty::OutcomeUnknown => {
            unknown_prefix_matches(cause, cut, confirmed, total)
        }
    }
}

const fn unknown_prefix_matches(
    cause: DiagnosticDestinationCause,
    cut: DiagnosticDestinationCut,
    confirmed: CheckedByteCount,
    total: CheckedByteCount,
) -> bool {
    if confirmed.value() >= total.value() {
        return false;
    }
    match (cause, cut) {
        (_, DiagnosticDestinationCut::InCall)
        | (
            DiagnosticDestinationCause::WriterContractViolation,
            DiagnosticDestinationCut::Returned,
        ) => true,
        (_, DiagnosticDestinationCut::Returned) => confirmed.value() != 0,
        (_, DiagnosticDestinationCut::BeforeCall) => false,
    }
}
