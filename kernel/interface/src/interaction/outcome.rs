//! Coherent terminal Interaction Outcome values.

use core::fmt;

use crate::buffer::plan::{
    BufferHistoryTransitionIdentity, BufferTransactionIdentity,
};
use crate::buffer::{BufferStateStamp, BufferVersion};
use crate::effect::EffectCertainty;
use crate::semantic::stamp::SemanticIntentStamp;

use super::identity::InteractionReconcileIdentity;
use super::session::InteractionRequestStamp;

/// Semantic cause of one terminal Interaction disposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeCause {
    /// Session authority denied the request.
    Denied,
    /// The requested operation or capability is unsupported.
    Unsupported,
    /// Contract or carrier selections are incompatible.
    Incompatible,
    /// The semantic Intent is invalid under selected policy.
    InvalidIntent,
    /// One observed authority or state generation is stale.
    StaleState,
    /// The selected Representation is not editable.
    ReadOnly,
    /// No valid source correspondence exists.
    NoCorrespondence,
    /// More than one correspondence exists without a selected resolution.
    AmbiguousCorrespondence,
    /// Buffer Preconditions or version assumptions conflict.
    Conflict,
    /// Bounded capacity prevented admission or completion.
    Pressure,
    /// Cooperative cancellation affected the request.
    Cancelled,
    /// The separately governed deadline was reached.
    Deadline,
    /// The caller ended its wait without proving terminal execution.
    CallerTimeout,
    /// A newer incompatible generation replaced the request target.
    Replaced,
    /// Session shutdown has begun.
    Closing,
    /// The Session is closed.
    Closed,
    /// Selected Domain or Representation policy failed.
    PolicyFailure,
    /// Selected provider returned a value outside its contract.
    ProviderContractViolation,
    /// Runtime delivery failed after work may have completed.
    DeliveryFailure,
    /// An internal invariant failure prevented a proved disposition.
    InvariantFailure,
}

/// Exact accepted Transaction and resulting Buffer state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionAcceptedState {
    transaction: BufferTransactionIdentity,
    history: BufferHistoryTransitionIdentity,
    result: BufferStateStamp,
}

impl InteractionAcceptedState {
    /// Returns the accepted Transaction identity.
    #[must_use]
    pub const fn transaction(self) -> BufferTransactionIdentity {
        self.transaction
    }

    /// Returns the accepted History transition identity.
    #[must_use]
    pub const fn history(self) -> BufferHistoryTransitionIdentity {
        self.history
    }

    /// Returns the exact resulting Buffer state.
    #[must_use]
    pub const fn result(self) -> BufferStateStamp {
        self.result
    }
}

/// Observable state of one terminal Interaction Outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeStatus {
    /// One local Transaction was atomically accepted.
    Accepted,
    /// No local Transaction was accepted.
    NotAccepted,
    /// Effect knowledge requires bounded reconciliation.
    ReconciliationRequired,
}

/// Typed incoherence in an Interaction Outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeError {
    /// Request-to-semantic identity mapping differs from the retained Intent.
    SemanticBindingMismatch,
    /// Result names another Resource Snapshot.
    SourceMismatch,
    /// Result names another Buffer incarnation or generation.
    BufferMismatch,
    /// Resulting Buffer version could not advance without overflow.
    BufferVersionOverflow,
    /// Resulting Buffer version is not the exact next version.
    BufferVersionNotAdvanced {
        /// Version required after one accepted Transaction.
        expected: BufferVersion,
        /// Version supplied by the Outcome.
        actual: BufferVersion,
    },
    /// Resulting History position could not advance without overflow.
    HistoryPositionOverflow,
    /// Resulting History position is not the exact next position.
    HistoryPositionNotAdvanced {
        /// Position required after one accepted Transaction.
        expected: u64,
        /// Position supplied by the Outcome.
        actual: u64,
    },
    /// This cause cannot produce an `OutcomeUnknown` local disposition.
    CauseCannotRequireReconciliation,
}

impl fmt::Display for InteractionOutcomeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SemanticBindingMismatch => {
                "Interaction Outcome semantic binding differs"
            }
            Self::SourceMismatch => {
                "accepted Interaction Outcome changes its source Snapshot"
            }
            Self::BufferMismatch => {
                "accepted Interaction Outcome changes Buffer incarnation"
            }
            Self::BufferVersionOverflow => {
                "accepted Interaction Buffer version overflowed"
            }
            Self::BufferVersionNotAdvanced { .. } => {
                "accepted Interaction Buffer version did not advance once"
            }
            Self::HistoryPositionOverflow => {
                "accepted Interaction History position overflowed"
            }
            Self::HistoryPositionNotAdvanced { .. } => {
                "accepted Interaction History position did not advance once"
            }
            Self::CauseCannotRequireReconciliation => {
                "Interaction cause cannot require reconciliation"
            }
        })
    }
}

impl core::error::Error for InteractionOutcomeError {}

/// One coherent terminal semantic disposition for an Interaction Intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionOutcome {
    request: InteractionRequestStamp,
    status: InteractionOutcomeStatus,
    accepted: Option<InteractionAcceptedState>,
    cause: Option<InteractionOutcomeCause>,
    reconcile: Option<InteractionReconcileIdentity>,
}

impl InteractionOutcome {
    /// Constructs a confirmed accepted local Transaction result.
    ///
    /// # Errors
    ///
    /// Returns a mapping, source, Buffer, version, or History mismatch.
    pub const fn accepted(
        request: InteractionRequestStamp,
        semantic: &SemanticIntentStamp,
        result: BufferStateStamp,
    ) -> Result<Self, InteractionOutcomeError> {
        let accepted = match validate_accepted(&request, semantic, &result) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(Self {
            request,
            status: InteractionOutcomeStatus::Accepted,
            accepted: Some(accepted),
            cause: None,
            reconcile: None,
        })
    }

    /// Constructs a proved definitely-not-applied terminal result.
    #[must_use]
    pub const fn not_accepted(
        request: InteractionRequestStamp,
        cause: InteractionOutcomeCause,
    ) -> Self {
        Self {
            request,
            status: InteractionOutcomeStatus::NotAccepted,
            accepted: None,
            cause: Some(cause),
            reconcile: None,
        }
    }

    /// Constructs an uncertain terminal result with bounded reconciliation.
    ///
    /// # Errors
    ///
    /// Returns a cause that cannot be uncertain at a local effect cut.
    pub const fn reconciliation_required(
        request: InteractionRequestStamp,
        cause: InteractionOutcomeCause,
        reconcile: InteractionReconcileIdentity,
    ) -> Result<Self, InteractionOutcomeError> {
        if !may_require_reconciliation(cause) {
            return Err(
                InteractionOutcomeError::CauseCannotRequireReconciliation,
            );
        }
        Ok(Self {
            request,
            status: InteractionOutcomeStatus::ReconciliationRequired,
            accepted: None,
            cause: Some(cause),
            reconcile: Some(reconcile),
        })
    }

    /// Returns the exact request receiving this terminal disposition.
    #[must_use]
    pub const fn request(self) -> InteractionRequestStamp {
        self.request
    }

    /// Returns accepted, not accepted, or reconciliation-required state.
    #[must_use]
    pub const fn status(self) -> InteractionOutcomeStatus {
        self.status
    }

    /// Returns exact local effect knowledge.
    #[must_use]
    pub const fn effect_certainty(self) -> EffectCertainty {
        match self.status {
            InteractionOutcomeStatus::Accepted => EffectCertainty::Applied,
            InteractionOutcomeStatus::NotAccepted => {
                EffectCertainty::DefinitelyNotApplied
            }
            InteractionOutcomeStatus::ReconciliationRequired => {
                EffectCertainty::OutcomeUnknown
            }
        }
    }

    /// Returns the accepted Transaction result when present.
    #[must_use]
    pub const fn accepted_state(self) -> Option<InteractionAcceptedState> {
        self.accepted
    }

    /// Returns the non-success cause when present.
    #[must_use]
    pub const fn cause(self) -> Option<InteractionOutcomeCause> {
        self.cause
    }

    /// Returns the bounded reconciliation identity when required.
    #[must_use]
    pub const fn reconcile(self) -> Option<InteractionReconcileIdentity> {
        self.reconcile
    }
}

const fn validate_accepted(
    request: &InteractionRequestStamp,
    semantic: &SemanticIntentStamp,
    result: &BufferStateStamp,
) -> Result<InteractionAcceptedState, InteractionOutcomeError> {
    if !request.semantic().matches_intent(semantic) {
        return Err(InteractionOutcomeError::SemanticBindingMismatch);
    }
    let target = *semantic.target();
    if !target.source().same_snapshot(result.source()) {
        return Err(InteractionOutcomeError::SourceMismatch);
    }
    if !target.buffer().same_generation(result.buffer()) {
        return Err(InteractionOutcomeError::BufferMismatch);
    }
    let expected_version = match target.version().value().checked_add(1) {
        Some(value) => BufferVersion::new(value),
        None => return Err(InteractionOutcomeError::BufferVersionOverflow),
    };
    if result.version().value() != expected_version.value() {
        return Err(InteractionOutcomeError::BufferVersionNotAdvanced {
            expected: expected_version,
            actual: result.version(),
        });
    }
    if !target
        .history()
        .lifecycle()
        .same_generation(result.history().lifecycle())
    {
        return Err(InteractionOutcomeError::BufferMismatch);
    }
    let Some(expected_history) =
        target.history().position().value().checked_add(1)
    else {
        return Err(InteractionOutcomeError::HistoryPositionOverflow);
    };
    let actual_history = result.history().position().value();
    if actual_history != expected_history {
        return Err(InteractionOutcomeError::HistoryPositionNotAdvanced {
            expected: expected_history,
            actual: actual_history,
        });
    }
    Ok(InteractionAcceptedState {
        transaction: request.semantic().transaction(),
        history: request.semantic().history(),
        result: *result,
    })
}

const fn may_require_reconciliation(cause: InteractionOutcomeCause) -> bool {
    reconciliation_cause_code(cause) == 1
}

const fn reconciliation_cause_code(cause: InteractionOutcomeCause) -> u8 {
    match cause {
        InteractionOutcomeCause::Cancelled
        | InteractionOutcomeCause::Deadline
        | InteractionOutcomeCause::CallerTimeout
        | InteractionOutcomeCause::DeliveryFailure
        | InteractionOutcomeCause::InvariantFailure => 1,
        _ => 0,
    }
}
