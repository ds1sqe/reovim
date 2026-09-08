//! History submission shares Session authority and terminal reservations.

use core::fmt;

use crate::bound::{BoundValue, CheckedByteCount};
use crate::cancellation::CancellationCause;

use super::bounds::{
    InteractionBoundField, InteractionBoundViolation, InteractionIntentCharge,
};
use super::delivery::intent::{
    InteractionCancellationResult, InteractionIntentAdmissionStatus,
    InteractionOutcomeDeliveryStatus, InteractionOutcomeUnavailable,
    InteractionReconcileResult,
};
use super::descriptor::InteractionSessionDescriptor;
use super::history::{InteractionHistoryOutcome, InteractionHistoryRequest};
use super::identity::InteractionReconcileIdentity;
use super::outcome::InteractionOutcomeCause;
use super::port::InteractionSessionPort;

/// Immediate admission; rejection never reports an applied effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionHistoryAdmission {
    /// Exact immutable request, retained even when admission fails.
    pub request: InteractionHistoryRequest,
    /// `None` means Runtime owns the admitted work and terminal reservation.
    pub rejection: Option<InteractionOutcomeCause>,
}

impl InteractionHistoryAdmission {
    /// Reports transfer into bounded Runtime ownership, not acceptance.
    #[must_use]
    pub const fn admitted(request: InteractionHistoryRequest) -> Self {
        Self { request, rejection: None }
    }

    /// Returns the unchanged request with definite non-admission.
    #[must_use]
    pub const fn not_admitted(
        request: InteractionHistoryRequest,
        cause: InteractionOutcomeCause,
    ) -> Self {
        Self { request, rejection: Some(cause) }
    }

    /// Uses the same admission distinction as semantic edit submission.
    #[must_use]
    pub const fn status(&self) -> InteractionIntentAdmissionStatus {
        if self.rejection.is_some() {
            InteractionIntentAdmissionStatus::NotAdmitted
        } else {
            InteractionIntentAdmissionStatus::Admitted
        }
    }
}

/// Non-waiting terminal lookup for the real History request type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionHistoryOutcomeDelivery {
    outcome: Option<InteractionHistoryOutcome>,
    unavailable: Option<InteractionOutcomeUnavailable>,
}

impl InteractionHistoryOutcomeDelivery {
    /// Transfers a terminal result; its request must match the exact lookup.
    #[must_use]
    pub const fn delivered(outcome: InteractionHistoryOutcome) -> Self {
        Self { outcome: Some(outcome), unavailable: None }
    }

    /// Reports pending, expired, replaced or closed retained ownership.
    #[must_use]
    pub const fn unavailable(cause: InteractionOutcomeUnavailable) -> Self {
        Self { outcome: None, unavailable: Some(cause) }
    }

    /// Returns the existing common terminal-delivery status.
    #[must_use]
    pub const fn status(&self) -> InteractionOutcomeDeliveryStatus {
        match self.unavailable {
            Some(cause) => InteractionOutcomeDeliveryStatus::Unavailable(cause),
            None => InteractionOutcomeDeliveryStatus::Delivered,
        }
    }

    /// Returns the exact terminal History disposition when available.
    #[must_use]
    pub const fn outcome(&self) -> Option<InteractionHistoryOutcome> {
        self.outcome
    }
}

/// Reserves native request storage and a complete terminal outcome.
///
/// These are in-process storage charges, not serialized-protocol lengths.
/// Runtime must additionally use `admit_pending` for aggregate reservations.
///
/// # Errors
///
/// Returns a typed Session bound failure or unrepresentable storage size.
pub fn history_charge(
    session: &InteractionSessionDescriptor,
) -> Result<InteractionIntentCharge, InteractionHistoryChargeError> {
    let bounds = session.bounds().requests();
    let bytes =
        u64::try_from(core::mem::size_of::<InteractionHistoryOutcome>())
            .map_err(|_| InteractionHistoryChargeError::SizeOverflow)?;
    let needed = CheckedByteCount::new(bytes, crate::bound::ZeroPolicy::Reject)
        .map_err(|cause| {
            InteractionHistoryChargeError::Bound(
                InteractionBoundViolation::Bound {
                    field: InteractionBoundField::RetainedOutcomeBytes,
                    cause,
                },
            )
        })?;
    bounds
        .retained_outcome_bytes()
        .admit_per_object(BoundValue::Bytes(needed))
        .map_err(|cause| {
            InteractionHistoryChargeError::Bound(
                InteractionBoundViolation::Bound {
                    field: InteractionBoundField::RetainedOutcomeBytes,
                    cause,
                },
            )
        })?;
    let bytes =
        u64::try_from(core::mem::size_of::<InteractionHistoryRequest>())
            .map_err(|_| InteractionHistoryChargeError::SizeOverflow)?;
    bounds.prepare_intent(bytes).map_err(InteractionHistoryChargeError::Bound)
}

/// Failure to reserve a complete in-process History request/result pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionHistoryChargeError {
    /// Native storage size cannot fit the portable byte count.
    SizeOverflow,
    /// The existing Session request or terminal bound rejected its charge.
    Bound(InteractionBoundViolation),
}

impl fmt::Display for InteractionHistoryChargeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "History admission charge: {self:?}")
    }
}

impl core::error::Error for InteractionHistoryChargeError {}

/// Additional operations of the SAME mutable Session endpoint.
///
/// Implement together with `InteractionSessionPort`; never create a second
/// Session, Buffer, or History owner. Check authority before request lookup.
/// Edit and History share one Session-scoped request-ID namespace. An identical
/// retry reuses its retained result. Changed kind, binding or edit payload
/// under that ID rejects. Expired evidence requires explicit reconciliation.
///
/// Admission is non-waiting and reserves terminal capacity using the existing
/// request bounds before work begins. Cancellation uses the complete History
/// request, never a fabricated semantic Intent. Session close fences and drains
/// these requests too. Observer queues cannot consume terminal reservations.
pub trait InteractionHistoryPort: InteractionSessionPort {
    /// Admits one immutable traversal or returns it definitely not admitted.
    fn submit_history(
        &mut self,
        request: InteractionHistoryRequest,
    ) -> InteractionHistoryAdmission;

    /// Transfers only the outcome matching this complete request binding.
    fn take_history_outcome(
        &mut self,
        request: InteractionHistoryRequest,
    ) -> InteractionHistoryOutcomeDelivery;

    /// Requests cancellation without claiming execution has terminated.
    fn request_history_cancellation(
        &mut self,
        request: InteractionHistoryRequest,
        cause: CancellationCause,
    ) -> InteractionCancellationResult;

    /// Reconciles this History request after uncertainty or retention loss.
    fn reconcile_history_outcome(
        &mut self,
        request: InteractionHistoryRequest,
        reconcile: InteractionReconcileIdentity,
    ) -> InteractionReconcileResult<InteractionHistoryRequest>;
}
