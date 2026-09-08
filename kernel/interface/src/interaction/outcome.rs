//! Coherent terminal Interaction Outcome values.

use core::fmt;

use crate::buffer::plan::{
    BufferHistoryTransitionIdentity, BufferTransactionIdentity,
};
use crate::buffer::{
    BufferHistoryDirection, BufferHistoryEdge, BufferStateStamp, BufferVersion,
};
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
    /// No retained transaction is eligible in the requested History direction.
    NoHistory,
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
///
/// Reports authority state, not a command to move a client Position/Selection.
/// Each client owns its response and any cross-schema interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionAcceptedState {
    transaction: BufferTransactionIdentity,
    history: BufferHistoryTransitionIdentity,
    result: BufferStateStamp,
    direction: Option<BufferHistoryDirection>,
    edge: BufferHistoryEdge,
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

    /// Returns undo/redo direction, or `None` for a newly accepted edit.
    #[must_use]
    pub const fn history_direction(self) -> Option<BufferHistoryDirection> {
        self.direction
    }

    /// Returns the exact created or traversed History edge.
    #[must_use]
    pub const fn history_edge(self) -> BufferHistoryEdge {
        self.edge
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
    /// Current/result nodes do not match the selected non-self History edge.
    HistoryEdgeMismatch,
    /// The selected child differs from the explicitly requested redo branch.
    HistoryBranchMismatch,
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
            Self::HistoryEdgeMismatch => "accepted History edge differs",
            Self::HistoryBranchMismatch => "accepted redo branch differs",
            Self::CauseCannotRequireReconciliation => {
                "Interaction cause cannot require reconciliation"
            }
        })
    }
}

impl core::error::Error for InteractionOutcomeError {}

/// One terminal disposition retaining its exact immutable request binding.
///
/// The default request is an edit. History uses its own request type rather
/// than inventing a Domain Intent or Representation attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionOutcome<Request = InteractionRequestStamp> {
    request: Request,
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
        Ok(Self::from_accepted(request, &accepted))
    }
}

impl<Request: Copy> InteractionOutcome<Request> {
    pub(super) const fn from_accepted(
        request: Request,
        accepted: &InteractionAcceptedState,
    ) -> Self {
        Self {
            request,
            status: InteractionOutcomeStatus::Accepted,
            accepted: Some(*accepted),
            cause: None,
            reconcile: None,
        }
    }

    /// Constructs a proved definitely-not-applied terminal result.
    #[must_use]
    pub const fn not_accepted(
        request: Request,
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
        request: Request,
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
    pub const fn request(self) -> Request {
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
    let Ok(edge) = BufferHistoryEdge::new(
        semantic.target().history().node(),
        result.history().node(),
    ) else {
        return Err(InteractionOutcomeError::HistoryEdgeMismatch);
    };
    accepted_transition(
        semantic.target(),
        result,
        request.semantic().transaction(),
        request.semantic().history(),
        edge,
        None,
    )
}

pub(super) const fn accepted_transition(
    target: &BufferStateStamp,
    result: &BufferStateStamp,
    transaction: BufferTransactionIdentity,
    history: BufferHistoryTransitionIdentity,
    edge: BufferHistoryEdge,
    direction: Option<BufferHistoryDirection>,
) -> Result<InteractionAcceptedState, InteractionOutcomeError> {
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
    // New edits create a child; redo traverses the same parent-to-child shape.
    let traversal = match direction {
        Some(value) => value,
        None => BufferHistoryDirection::Redo,
    };
    if edge.from(traversal).value() != target.history().node().value()
        || edge.to(traversal).value() != result.history().node().value()
    {
        return Err(InteractionOutcomeError::HistoryEdgeMismatch);
    }
    Ok(InteractionAcceptedState {
        transaction,
        history,
        result: *result,
        direction,
        edge,
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
