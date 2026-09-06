//! Intent admission, cancellation, Outcome delivery, and reconciliation.

use core::fmt;

use crate::cancellation::{CancellationCause, CancellationOutcome};
use crate::effect::EffectCertainty;

use super::super::descriptor::{
    InteractionCapabilitySupport, InteractionSessionDescriptor,
};
use super::super::identity::InteractionReconcileIdentity;
use super::super::intent::InteractionIntent;
use super::super::outcome::{
    InteractionAcceptedState, InteractionOutcome, InteractionOutcomeCause,
    InteractionOutcomeStatus,
};
use super::super::session::{
    InteractionRequestStamp, InteractionSessionHandle,
};

/// Whether Intent ownership transferred to deferred Runtime work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionIntentAdmissionStatus {
    /// Runtime owns the Intent until its terminal Outcome resolves.
    Admitted,
    /// Runtime retained nothing and returned the complete Intent.
    NotAdmitted,
}

/// Typed incoherence in definite Intent non-admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionIntentAdmissionError {
    /// Immediate Outcome belongs to another request.
    OutcomeRequestMismatch,
    /// Immediate non-admission cannot report accepted or uncertain effect.
    OutcomeNotDefinitelyRejected,
}

impl fmt::Display for InteractionIntentAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutcomeRequestMismatch => {
                "non-admission Outcome belongs to another request"
            }
            Self::OutcomeNotDefinitelyRejected => {
                "non-admission Outcome is not definitely rejected"
            }
        })
    }
}

impl core::error::Error for InteractionIntentAdmissionError {}

/// Definite non-admission retaining Intent and terminal Outcome.
pub struct InteractionIntentNonAdmission {
    outcome: InteractionOutcome,
    intent: InteractionIntent,
}

impl InteractionIntentNonAdmission {
    /// Binds one immediate non-acceptance to the unchanged Intent.
    ///
    /// The returned binding owns either the valid value or both rejected
    /// inputs.
    #[must_use]
    pub const fn bind(
        outcome: InteractionOutcome,
        intent: InteractionIntent,
    ) -> InteractionIntentNonAdmissionBindingOutcome {
        let cause = if !outcome.request().same_request(intent.stamp()) {
            Some(InteractionIntentAdmissionError::OutcomeRequestMismatch)
        } else if outcome.status() as u8
            != InteractionOutcomeStatus::NotAccepted as u8
        {
            Some(InteractionIntentAdmissionError::OutcomeNotDefinitelyRejected)
        } else {
            None
        };
        match cause {
            Some(cause) => {
                InteractionIntentNonAdmissionBindingOutcome::Rejected(
                    InteractionIntentNonAdmissionRejection {
                        cause,
                        outcome,
                        intent,
                    },
                )
            }
            None => InteractionIntentNonAdmissionBindingOutcome::Bound(Self {
                outcome,
                intent,
            }),
        }
    }

    /// Returns the immediate terminal non-acceptance.
    #[must_use]
    pub const fn outcome(&self) -> InteractionOutcome {
        self.outcome
    }

    /// Borrows the unchanged returned Intent.
    #[must_use]
    pub const fn intent(&self) -> &InteractionIntent {
        &self.intent
    }

    /// Releases both complete values.
    #[must_use]
    pub fn into_parts(self) -> (InteractionOutcome, InteractionIntent) {
        (self.outcome, self.intent)
    }
}

impl fmt::Debug for InteractionIntentNonAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionIntentNonAdmission")
            .field("outcome", &self.outcome)
            .field("intent", &self.intent)
            .finish()
    }
}

/// Owning result of binding one immediate non-admission.
pub enum InteractionIntentNonAdmissionBindingOutcome {
    /// Outcome and returned Intent are coherent.
    Bound(InteractionIntentNonAdmission),
    /// Binding failed while preserving both complete inputs.
    Rejected(InteractionIntentNonAdmissionRejection),
}

impl fmt::Debug for InteractionIntentNonAdmissionBindingOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bound(value) => {
                formatter.debug_tuple("Bound").field(value).finish()
            }
            Self::Rejected(value) => {
                formatter.debug_tuple("Rejected").field(value).finish()
            }
        }
    }
}

/// Rejected non-admission construction retaining both complete values.
pub struct InteractionIntentNonAdmissionRejection {
    cause: InteractionIntentAdmissionError,
    outcome: InteractionOutcome,
    intent: InteractionIntent,
}

impl InteractionIntentNonAdmissionRejection {
    /// Returns the construction failure.
    #[must_use]
    pub const fn cause(&self) -> InteractionIntentAdmissionError {
        self.cause
    }

    /// Borrows the unchanged returned Intent.
    #[must_use]
    pub const fn intent(&self) -> &InteractionIntent {
        &self.intent
    }

    /// Returns the unchanged Outcome.
    #[must_use]
    pub const fn outcome(&self) -> InteractionOutcome {
        self.outcome
    }

    /// Releases both complete values.
    #[must_use]
    pub fn into_parts(self) -> (InteractionOutcome, InteractionIntent) {
        (self.outcome, self.intent)
    }
}

impl fmt::Debug for InteractionIntentNonAdmissionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionIntentNonAdmissionRejection")
            .field("cause", &self.cause)
            .field("outcome", &self.outcome)
            .field("intent", &self.intent)
            .finish()
    }
}

/// Exact ownership result of one Intent submission.
pub struct InteractionIntentAdmission {
    admitted: Option<InteractionRequestStamp>,
    returned: Option<InteractionIntentNonAdmission>,
}

impl InteractionIntentAdmission {
    /// Reports successful transfer of one exact Intent to Runtime ownership.
    #[must_use]
    pub const fn admitted(intent: &InteractionIntent) -> Self {
        Self { admitted: Some(intent.stamp()), returned: None }
    }

    /// Returns one unchanged Intent with immediate definite non-acceptance.
    #[must_use]
    pub const fn not_admitted(
        intent: InteractionIntent,
        cause: InteractionOutcomeCause,
    ) -> Self {
        let outcome = InteractionOutcome::not_accepted(intent.stamp(), cause);
        Self {
            admitted: None,
            returned: Some(InteractionIntentNonAdmission { outcome, intent }),
        }
    }

    /// Reports whether Intent ownership transferred to Runtime.
    #[must_use]
    pub const fn status(&self) -> InteractionIntentAdmissionStatus {
        if self.admitted.is_some() {
            InteractionIntentAdmissionStatus::Admitted
        } else {
            InteractionIntentAdmissionStatus::NotAdmitted
        }
    }

    /// Returns the admitted request stamp when ownership transferred.
    #[must_use]
    pub const fn admitted_request(&self) -> Option<InteractionRequestStamp> {
        self.admitted
    }

    /// Borrows the complete non-admission when ownership returned.
    #[must_use]
    pub const fn non_admission(
        &self,
    ) -> Option<&InteractionIntentNonAdmission> {
        self.returned.as_ref()
    }

    /// Releases a complete non-admission when present.
    #[must_use]
    pub fn into_non_admission(self) -> Option<InteractionIntentNonAdmission> {
        self.returned
    }
}

impl fmt::Debug for InteractionIntentAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionIntentAdmission")
            .field("status", &self.status())
            .field("admitted", &self.admitted)
            .field("returned", &self.returned)
            .finish()
    }
}

/// Typed structural failure for a cancellation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCancellationRequestError {
    /// The request targets another Session generation.
    SessionMismatch,
    /// Session does not support cancellation.
    Unsupported,
}

impl fmt::Display for InteractionCancellationRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => {
                "cancellation request targets another Session"
            }
            Self::Unsupported => "Session does not support cancellation",
        })
    }
}

impl core::error::Error for InteractionCancellationRequestError {}

/// Copyable request for cooperative cancellation of admitted work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionCancellationRequest {
    request: InteractionRequestStamp,
    cause: CancellationCause,
}

impl InteractionCancellationRequest {
    /// Constructs cancellation under one exact Session capability.
    ///
    /// # Errors
    ///
    /// Returns Session-generation or unsupported-capability failure.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        request: InteractionRequestStamp,
        cause: CancellationCause,
    ) -> Result<Self, InteractionCancellationRequestError> {
        if !request.session().matches(session.stamp()) {
            return Err(InteractionCancellationRequestError::SessionMismatch);
        }
        if !is_supported(session.capabilities().cancellation()) {
            return Err(InteractionCancellationRequestError::Unsupported);
        }
        Ok(Self { request, cause })
    }

    /// Returns the exact target request.
    #[must_use]
    pub const fn request(self) -> InteractionRequestStamp {
        self.request
    }

    /// Returns why cooperative cancellation was requested.
    #[must_use]
    pub const fn cause(self) -> CancellationCause {
        self.cause
    }
}

/// Immediate acknowledgement of one cancellation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCancellationResult {
    /// Cancellation was newly requested with independent terminality.
    Requested(CancellationOutcome),
    /// The same cancellation request was already visible.
    AlreadyRequested(CancellationOutcome),
    /// The request already has a retained terminal Outcome.
    AlreadyTerminal,
    /// Authority was denied without revealing request existence.
    Denied,
    /// No admitted request matches the exact stamp.
    UnknownRequest,
    /// The request targets another Session generation.
    StaleSession,
    /// Session shutdown is resolving admitted work.
    Closing,
    /// The Session is closed.
    Closed,
}

/// Caller-owned non-waiting terminal Outcome lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionOutcomeRequest {
    request: InteractionRequestStamp,
}

impl InteractionOutcomeRequest {
    /// Constructs one exact Outcome lookup.
    ///
    /// # Errors
    ///
    /// Returns when the request belongs to another Session generation.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        request: InteractionRequestStamp,
    ) -> Result<Self, InteractionOutcomeRequestError> {
        if !request.session().matches(session.stamp()) {
            return Err(InteractionOutcomeRequestError::SessionMismatch);
        }
        Ok(Self { request })
    }

    /// Returns the exact Session generation.
    #[must_use]
    pub const fn session(self) -> InteractionSessionHandle {
        self.request.session()
    }

    /// Returns the exact request stamp.
    #[must_use]
    pub const fn request(self) -> InteractionRequestStamp {
        self.request
    }
}

/// Typed Outcome lookup request mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeRequestError {
    /// Request belongs to another Session generation.
    SessionMismatch,
}

impl fmt::Display for InteractionOutcomeRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Outcome request targets another Session")
    }
}

impl core::error::Error for InteractionOutcomeRequestError {}

/// Reason one non-waiting lookup transfers no terminal Outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeUnavailable {
    /// The admitted operation remains owned and nonterminal.
    Pending,
    /// No admitted request matches the supplied stamp.
    UnknownRequest,
    /// The terminal Outcome was already transferred.
    AlreadyDelivered,
    /// Retention loss requires the supplied reconciliation identity.
    Gap(InteractionReconcileIdentity),
    /// A newer Session generation fenced this lookup.
    Replaced,
    /// Session shutdown is resolving terminal delivery.
    Closing,
    /// The Session is closed with no matching retained Outcome.
    Closed,
}

/// Delivered or unavailable status of one Outcome lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeDeliveryStatus {
    /// One exact terminal Outcome transfers.
    Delivered,
    /// No Outcome transfers for the stated reason.
    Unavailable(InteractionOutcomeUnavailable),
}

/// Non-waiting transfer of one terminal Outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionOutcomeDelivery {
    outcome: Option<InteractionOutcome>,
    unavailable: Option<InteractionOutcomeUnavailable>,
}

impl InteractionOutcomeDelivery {
    /// Transfers one exact terminal Outcome.
    #[must_use]
    pub const fn delivered(outcome: InteractionOutcome) -> Self {
        Self { outcome: Some(outcome), unavailable: None }
    }

    /// Reports why no terminal Outcome transfers.
    #[must_use]
    pub const fn unavailable(cause: InteractionOutcomeUnavailable) -> Self {
        Self { outcome: None, unavailable: Some(cause) }
    }

    /// Returns the exact delivery disposition.
    #[must_use]
    pub const fn status(self) -> InteractionOutcomeDeliveryStatus {
        match self.unavailable {
            Some(cause) => InteractionOutcomeDeliveryStatus::Unavailable(cause),
            None => InteractionOutcomeDeliveryStatus::Delivered,
        }
    }

    /// Returns the delivered terminal Outcome when present.
    #[must_use]
    pub const fn outcome(self) -> Option<InteractionOutcome> {
        self.outcome
    }
}

/// Typed structural failure for one reconciliation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionReconcileRequestError {
    /// The request belongs to another Session generation.
    SessionMismatch,
    /// Session does not support Outcome reconciliation.
    Unsupported,
}

impl fmt::Display for InteractionReconcileRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => "reconciliation targets another Session",
            Self::Unsupported => {
                "Session does not support Outcome reconciliation"
            }
        })
    }
}

impl core::error::Error for InteractionReconcileRequestError {}

/// Caller-owned exact Outcome reconciliation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionReconcileRequest {
    request: InteractionRequestStamp,
    reconcile: InteractionReconcileIdentity,
}

impl InteractionReconcileRequest {
    /// Constructs one reconciliation request under Session capability.
    ///
    /// # Errors
    ///
    /// Returns Session-generation or unsupported-capability failure.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        request: InteractionRequestStamp,
        reconcile: InteractionReconcileIdentity,
    ) -> Result<Self, InteractionReconcileRequestError> {
        if !request.session().matches(session.stamp()) {
            return Err(InteractionReconcileRequestError::SessionMismatch);
        }
        if !is_supported(session.capabilities().reconciliation()) {
            return Err(InteractionReconcileRequestError::Unsupported);
        }
        Ok(Self { request, reconcile })
    }

    /// Returns the exact original Interaction request.
    #[must_use]
    pub const fn request(self) -> InteractionRequestStamp {
        self.request
    }

    /// Returns the exact reconciliation identity.
    #[must_use]
    pub const fn reconcile(self) -> InteractionReconcileIdentity {
        self.reconcile
    }
}

/// Resolved effect knowledge for an `OutcomeUnknown` terminal result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionReconciledEffect {
    certainty: EffectCertainty,
    accepted: Option<InteractionAcceptedState>,
    cause: Option<InteractionOutcomeCause>,
}

impl InteractionReconciledEffect {
    /// Constructs a proved applied local result.
    #[must_use]
    pub const fn applied(accepted: InteractionAcceptedState) -> Self {
        Self {
            certainty: EffectCertainty::Applied,
            accepted: Some(accepted),
            cause: None,
        }
    }

    /// Constructs a proved definitely-not-applied result.
    #[must_use]
    pub const fn definitely_not_applied(
        cause: InteractionOutcomeCause,
    ) -> Self {
        Self {
            certainty: EffectCertainty::DefinitelyNotApplied,
            accepted: None,
            cause: Some(cause),
        }
    }

    /// Returns resolved local effect knowledge.
    #[must_use]
    pub const fn certainty(self) -> EffectCertainty {
        self.certainty
    }

    /// Returns accepted state when application was proved.
    #[must_use]
    pub const fn accepted(self) -> Option<InteractionAcceptedState> {
        self.accepted
    }

    /// Returns the non-application cause when proved.
    #[must_use]
    pub const fn cause(self) -> Option<InteractionOutcomeCause> {
        self.cause
    }
}

/// Reason one reconciliation call returned no recovered or resolved value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionReconcileUnavailable {
    /// The retained effect remains nonterminal or unknown.
    Pending,
    /// Bounded reconciliation ended without proved effect knowledge.
    Unresolved,
    /// The retained reconciliation window expired visibly.
    Expired,
    /// Authority was denied before revealing request state.
    Denied,
    /// No request/reconcile pair matches.
    UnknownRequest,
    /// The request belongs to another Session generation.
    StaleSession,
    /// Session shutdown is resolving retained state.
    Closing,
    /// The Session is closed without recoverable state.
    Closed,
}

/// Observable status of one Outcome reconciliation call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionReconcileStatus {
    /// An Outcome that had never transferred is recovered.
    Recovered,
    /// Previously unknown effect knowledge is resolved.
    EffectResolved,
    /// No value transfers for the stated reason.
    Unavailable(InteractionReconcileUnavailable),
}

/// Result of one exact bounded Outcome reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionReconcileResult {
    status: InteractionReconcileStatus,
    recovered: Option<InteractionOutcome>,
    effect: Option<InteractionReconciledEffect>,
}

impl InteractionReconcileResult {
    /// Recovers an Outcome that had not previously transferred.
    #[must_use]
    pub const fn recovered(outcome: InteractionOutcome) -> Self {
        Self {
            status: InteractionReconcileStatus::Recovered,
            recovered: Some(outcome),
            effect: None,
        }
    }

    /// Resolves previously unknown local effect knowledge.
    #[must_use]
    pub const fn effect_resolved(effect: InteractionReconciledEffect) -> Self {
        Self {
            status: InteractionReconcileStatus::EffectResolved,
            recovered: None,
            effect: Some(effect),
        }
    }

    /// Constructs one non-value reconciliation status.
    #[must_use]
    pub const fn unavailable(cause: InteractionReconcileUnavailable) -> Self {
        Self {
            status: InteractionReconcileStatus::Unavailable(cause),
            recovered: None,
            effect: None,
        }
    }

    /// Returns the reconciliation status.
    #[must_use]
    pub const fn status(self) -> InteractionReconcileStatus {
        self.status
    }

    /// Returns a recovered first-delivery Outcome when present.
    #[must_use]
    pub const fn recovered_outcome(self) -> Option<InteractionOutcome> {
        self.recovered
    }

    /// Returns resolved effect knowledge when present.
    #[must_use]
    pub const fn effect(self) -> Option<InteractionReconciledEffect> {
        self.effect
    }
}

const fn is_supported(value: InteractionCapabilitySupport) -> bool {
    value as u8 == InteractionCapabilitySupport::Supported as u8
}
