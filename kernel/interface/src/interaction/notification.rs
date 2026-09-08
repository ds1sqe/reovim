//! Correlated accepted Buffer changes, independent of client follow policy.

use core::fmt;

use crate::bound::{BoundError, CheckedCount};
use crate::buffer::plan::BufferCorrelationIdentity;
use crate::buffer::{BufferStateStamp, BufferVersion};

use super::bounds::InteractionBoundViolation;
use super::history::InteractionHistoryOutcome;
use super::identity::InteractionRequestIdentity;
use super::intent::InteractionIntent;
use super::outcome::{
    InteractionAcceptedState, InteractionOutcome, InteractionOutcomeError,
};
use super::port::InteractionSessionPort;
use super::session::InteractionSessionHandle;

mod delivery;
mod subscription;

pub use delivery::{BufferChangeDelivery, BufferChangeDeliveryStatus};
pub use subscription::BufferChangeSubscription;

/// Typed subscription, correlation or checkpoint failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferChangeError {
    /// Authority was denied before revealing subscription or Buffer state.
    Denied,
    /// This Session does not expose semantic update observation.
    Unsupported,
    /// Session or subscription lifetime no longer matches the retained grant.
    StaleSubscription,
    /// A checkpoint or change belongs to another Resource/Buffer scope.
    ScopeMismatch,
    /// The supplied edit result names another complete request binding.
    RequestMismatch,
    /// Rejection or uncertainty cannot be reported as a committed change.
    NotAccepted,
    /// Prior/result stamps do not describe the accepted transition.
    Transition(InteractionOutcomeError),
    /// A requested frontier or delivered transition is discontinuous.
    CheckpointMismatch,
    /// Queue, byte or control reservation exceeded the selected bound.
    Bound(BoundError),
    /// The shared update/control accounting contract rejected a reservation.
    FlowBound(InteractionBoundViolation),
    /// A storage charge cannot be represented by the portable byte count.
    SizeOverflow,
    /// The owner is closing and cannot admit a new subscription.
    Closing,
    /// The owning Session is closed.
    Closed,
    /// Bounded close did not resolve all subscription ownership.
    IncompleteShutdown,
}

impl fmt::Display for BufferChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Buffer change notification: {self:?}")
    }
}

impl core::error::Error for BufferChangeError {}

/// One accepted transition; contains no Position, Selection or drawing state.
///
/// The originating Session is not the destination subscriber. Runtime exposes
/// this record only to separately authorized observers of the same Buffer.
/// Correlation identifiers are evidence, never cancellation or edit grants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferChangeNotification {
    /// Originating request's Session incarnation.
    origin: InteractionSessionHandle,
    /// Identity within that Session's shared edit/History request namespace.
    request: InteractionRequestIdentity,
    /// Root Buffer-plan cause shared with diagnostics and the terminal outcome.
    correlation: BufferCorrelationIdentity,
    /// Exact state against which this transition was accepted.
    before: BufferStateStamp,
    /// Actual resulting state, transaction and created/traversed History edge.
    accepted: InteractionAcceptedState,
}

impl BufferChangeNotification {
    /// Returns the originating Session lifetime, not the destination observer.
    #[must_use]
    pub const fn origin(&self) -> InteractionSessionHandle {
        self.origin
    }

    /// Returns the request identity within the originating Session.
    #[must_use]
    pub const fn request(&self) -> InteractionRequestIdentity {
        self.request
    }

    /// Returns the root Buffer-plan correlation shared with the outcome.
    #[must_use]
    pub const fn correlation(&self) -> BufferCorrelationIdentity {
        self.correlation
    }

    /// Returns the exact state before this accepted transition.
    #[must_use]
    pub const fn before(&self) -> BufferStateStamp {
        self.before
    }

    /// Returns the actual result and created/traversed History edge.
    #[must_use]
    pub const fn accepted(&self) -> InteractionAcceptedState {
        self.accepted
    }

    /// Derives a notification from an accepted result and retained actual edit.
    ///
    /// # Errors
    ///
    /// Rejects a mismatched request, non-acceptance or incoherent transition.
    pub fn from_edit(
        outcome: InteractionOutcome,
        intent: &InteractionIntent,
    ) -> Result<Self, BufferChangeError> {
        if !outcome.request().same_request(intent.stamp()) {
            return Err(BufferChangeError::RequestMismatch);
        }
        let accepted =
            outcome.accepted_state().ok_or(BufferChangeError::NotAccepted)?;
        let expected = InteractionOutcome::accepted(
            intent.stamp(),
            intent.semantic().stamp(),
            accepted.result(),
        )
        .map_err(BufferChangeError::Transition)?;
        if expected != outcome {
            return Err(BufferChangeError::RequestMismatch);
        }
        Ok(Self {
            origin: outcome.request().session(),
            request: outcome.request().request(),
            correlation: intent.semantic().stamp().plan().correlation(),
            before: *intent.semantic().stamp().target(),
            accepted,
        })
    }

    /// Derives a change from the real History request/result, without an edit.
    ///
    /// # Errors
    ///
    /// Rejects a non-accepted or incoherent History result.
    pub fn from_history(
        outcome: InteractionHistoryOutcome,
    ) -> Result<Self, BufferChangeError> {
        let accepted =
            outcome.accepted_state().ok_or(BufferChangeError::NotAccepted)?;
        let request = outcome.request();
        InteractionHistoryOutcome::accepted_history(
            request,
            accepted.history_edge(),
            accepted.result(),
        )
        .map_err(BufferChangeError::Transition)?;
        Ok(Self {
            origin: request.session(),
            request: request.request(),
            correlation: request.plan().correlation(),
            before: request.target(),
            accepted,
        })
    }

    pub(super) fn validate(&self) -> Result<(), BufferChangeError> {
        super::outcome::accepted_transition(
            &self.before,
            &self.accepted.result(),
            self.accepted.transaction(),
            self.accepted.history(),
            self.accepted.history_edge(),
            self.accepted.history_direction(),
        )
        .map_err(BufferChangeError::Transition)?;
        Ok(())
    }
}

/// Bounded observation on the SAME Session authority as editing operations.
///
/// Each subscription has independent retention and control credit. Retained
/// notification storage uses the existing Session update bounds; it may not
/// consume request-terminal reservations. Fan-out occurs after commit, never
/// by calling clients under Buffer authority. Slow clients receive explicit
/// gaps rather than blocking edits or another subscriber.
///
/// Taking a record transfers its queue credit, not approval to follow it.
/// Representation payload installation, local Position/Selection policy and
/// presentation acknowledgements remain separate. Session close revokes these
/// subscriptions too, and reported close failure preserves live ownership.
/// Check authorization first, then exact-match the entire supplied subscription
/// against the live retained value. Static descriptor validation alone does not
/// establish a live grant or allow a caller to change its scope/checkpoint.
pub trait BufferChangePort: InteractionSessionPort {
    /// Checks observation grants, reserves capacity and atomically checkpoints.
    ///
    /// Ready requires the client's initial Representation snapshot to match
    /// this checkpoint, or explicit resynchronization before use. The returned
    /// stamp is a notification frontier, not reconstructed native data.
    ///
    /// # Errors
    ///
    /// Returns denied, unsupported, pressure or lifecycle failure before Ready.
    fn subscribe_changes(
        &mut self,
        capacity: CheckedCount,
    ) -> Result<BufferChangeSubscription, BufferChangeError>;

    /// Checks the live subscription before non-waiting next-record delivery.
    ///
    /// `after` is the last received Buffer version, not a History node ordinal.
    /// Deliver next change, pending, gap or closed; never silently skip one.
    /// Clients additionally match the notification's complete `before` stamp
    /// against their last received authoritative state. Equal version numbers
    /// alone cannot establish source or History continuity.
    ///
    /// # Errors
    ///
    /// Returns denied, stale subscription, invalid checkpoint or scope failure.
    fn take_change(
        &mut self,
        subscription: &BufferChangeSubscription,
        after: BufferVersion,
    ) -> Result<BufferChangeDelivery, BufferChangeError>;

    /// Rotates subscription generation and returns a new atomic checkpoint.
    ///
    /// Old cursors cannot resume the replacement. A matching Representation
    /// snapshot is separately required before derived-state readiness.
    ///
    /// # Errors
    ///
    /// Returns denied, stale subscription, pressure or lifecycle failure.
    fn resynchronize_changes(
        &mut self,
        subscription: &BufferChangeSubscription,
    ) -> Result<BufferChangeSubscription, BufferChangeError>;

    /// Idempotently stops delivery and releases this subscription's ownership.
    ///
    /// # Errors
    ///
    /// Returns denied, stale subscription or explicit incomplete shutdown.
    fn unsubscribe_changes(
        &mut self,
        subscription: &BufferChangeSubscription,
    ) -> Result<(), BufferChangeError>;
}
