//! Move-only Interaction Intent ownership and outer validation.

use core::fmt;

use crate::cancellation::{CancellationState, DeadlineState};
use crate::semantic::intent::SemanticIntent;

use super::bounds::{InteractionBoundViolation, InteractionIntentCharge};
use super::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use super::session::InteractionRequestStamp;

/// Typed outer Interaction Intent validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionIntentError {
    /// The request targets another Session generation.
    SessionMismatch,
    /// The request targets another attachment generation.
    AttachmentMismatch,
    /// Semantic target lies outside the Session Resource scope.
    ScopeMismatch,
    /// Semantic caller authority differs from the Session authority.
    CallerAuthorityMismatch,
    /// Semantic caller role differs from the Client role.
    CallerRoleMismatch,
    /// Explicit semantic identity mapping differs from the retained Intent.
    SemanticBindingMismatch,
    /// Semantic Intent names another Representation generation.
    RepresentationMismatch,
    /// Semantic Intent names another Projection predecessor state.
    ProjectionMismatch,
    /// Semantic Intent names another coordinate-space incarnation.
    CoordinateMismatch,
    /// Session reservation owner, size, or limit is invalid.
    Bound(InteractionBoundViolation),
    /// Reserved Intent bytes differ from the retained payload bytes.
    IntentByteChargeMismatch {
        /// Bytes reserved by the outer Session.
        reserved: u64,
        /// Bytes owned by the retained Semantic Intent.
        actual: u64,
    },
    /// Cancellation was already requested before ownership transfer.
    Cancelled,
    /// The separately governed deadline was already reached.
    DeadlineReached,
}

impl fmt::Display for InteractionIntentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => {
                "Interaction Intent targets another Session"
            }
            Self::AttachmentMismatch => {
                "Interaction Intent targets another attachment"
            }
            Self::ScopeMismatch => {
                "Interaction Intent lies outside Session scope"
            }
            Self::CallerAuthorityMismatch => {
                "semantic caller authority differs from Session authority"
            }
            Self::CallerRoleMismatch => {
                "semantic caller role differs from Client role"
            }
            Self::SemanticBindingMismatch => {
                "Interaction semantic identity binding differs"
            }
            Self::RepresentationMismatch => {
                "Interaction Intent names another Representation"
            }
            Self::ProjectionMismatch => {
                "Interaction Intent names another Projection state"
            }
            Self::CoordinateMismatch => {
                "Interaction Intent names another coordinate space"
            }
            Self::Bound(_) => {
                "Interaction Intent reservation violates Session bounds"
            }
            Self::IntentByteChargeMismatch { .. } => {
                "Interaction Intent byte reservation differs from payload"
            }
            Self::Cancelled => {
                "Interaction Intent was cancelled before admission"
            }
            Self::DeadlineReached => {
                "Interaction Intent deadline was reached before admission"
            }
        })
    }
}

impl core::error::Error for InteractionIntentError {}

/// Validated move-only outer request retaining one Semantic Intent owner.
pub struct InteractionIntent {
    stamp: InteractionRequestStamp,
    charge: InteractionIntentCharge,
    semantic: SemanticIntent,
}

impl InteractionIntent {
    /// Binds one Semantic Intent to an exact Session request.
    ///
    /// Rejection retains the unchanged Semantic Intent and all outer inputs.
    #[must_use]
    pub const fn bind(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        stamp: InteractionRequestStamp,
        charge: InteractionIntentCharge,
        semantic: SemanticIntent,
        cancellation: CancellationState,
        deadline: DeadlineState,
    ) -> InteractionIntentBindingOutcome {
        let cause = match validate_outer(
            session,
            attachment,
            &stamp,
            charge,
            &semantic,
            cancellation,
            deadline,
        ) {
            Ok(()) => {
                return InteractionIntentBindingOutcome::Bound(Self {
                    stamp,
                    charge,
                    semantic,
                });
            }
            Err(cause) => cause,
        };
        InteractionIntentBindingOutcome::Rejected(InteractionIntentRejection {
            cause,
            stamp,
            charge,
            semantic,
        })
    }

    /// Returns the complete outer request stamp.
    #[must_use]
    pub const fn stamp(&self) -> InteractionRequestStamp {
        self.stamp
    }

    /// Returns the prepared Session reservation.
    #[must_use]
    pub const fn charge(&self) -> InteractionIntentCharge {
        self.charge
    }

    /// Borrows the Runtime-retained Semantic Intent.
    #[must_use]
    pub const fn semantic(&self) -> &SemanticIntent {
        &self.semantic
    }

    /// Releases the outer request and returns the Semantic Intent owner.
    #[must_use]
    pub fn into_semantic(self) -> SemanticIntent {
        self.semantic
    }
}

impl fmt::Debug for InteractionIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionIntent")
            .field("stamp", &self.stamp)
            .field("charge", &self.charge)
            .field("semantic", &self.semantic)
            .finish()
    }
}

/// Exact owning result of binding one outer Interaction Intent.
pub enum InteractionIntentBindingOutcome {
    /// Every outer relation passed and produced one retained Intent.
    Bound(InteractionIntent),
    /// Binding failed without losing the Semantic Intent or outer inputs.
    Rejected(InteractionIntentRejection),
}

impl fmt::Debug for InteractionIntentBindingOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bound(intent) => {
                formatter.debug_tuple("Bound").field(intent).finish()
            }
            Self::Rejected(rejection) => {
                formatter.debug_tuple("Rejected").field(rejection).finish()
            }
        }
    }
}

/// Rejected outer binding with complete Semantic Intent ownership.
pub struct InteractionIntentRejection {
    cause: InteractionIntentError,
    stamp: InteractionRequestStamp,
    charge: InteractionIntentCharge,
    semantic: SemanticIntent,
}

impl InteractionIntentRejection {
    /// Returns the outer binding failure.
    #[must_use]
    pub const fn cause(&self) -> InteractionIntentError {
        self.cause
    }

    /// Returns the unchanged outer request stamp.
    #[must_use]
    pub const fn stamp(&self) -> InteractionRequestStamp {
        self.stamp
    }

    /// Returns the unchanged Session reservation.
    #[must_use]
    pub const fn charge(&self) -> InteractionIntentCharge {
        self.charge
    }

    /// Borrows the unchanged Semantic Intent owner.
    #[must_use]
    pub const fn semantic(&self) -> &SemanticIntent {
        &self.semantic
    }

    /// Releases the rejection into its complete inputs.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (InteractionRequestStamp, InteractionIntentCharge, SemanticIntent)
    {
        (self.stamp, self.charge, self.semantic)
    }
}

impl fmt::Debug for InteractionIntentRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionIntentRejection")
            .field("cause", &self.cause)
            .field("stamp", &self.stamp)
            .field("charge", &self.charge)
            .field("semantic", &self.semantic)
            .finish()
    }
}

const fn validate_outer(
    session: &InteractionSessionDescriptor,
    attachment: &RepresentationAttachmentDescriptor,
    stamp: &InteractionRequestStamp,
    charge: InteractionIntentCharge,
    semantic: &SemanticIntent,
    cancellation: CancellationState,
    deadline: DeadlineState,
) -> Result<(), InteractionIntentError> {
    if !stamp.session().matches(session.stamp()) {
        return Err(InteractionIntentError::SessionMismatch);
    }
    if !stamp.attachment().same_generation(attachment.handle()) {
        return Err(InteractionIntentError::AttachmentMismatch);
    }
    let semantic_stamp = semantic.stamp();
    if !session.scope().contains(*semantic_stamp.target()) {
        return Err(InteractionIntentError::ScopeMismatch);
    }
    if !semantic_stamp
        .caller()
        .authority()
        .same_generation(session.stamp().lifecycle())
    {
        return Err(InteractionIntentError::CallerAuthorityMismatch);
    }
    if !same_contract(
        semantic_stamp.caller().role(),
        session.stamp().client().role(),
    ) {
        return Err(InteractionIntentError::CallerRoleMismatch);
    }
    if !stamp.semantic().matches_intent(semantic_stamp) {
        return Err(InteractionIntentError::SemanticBindingMismatch);
    }
    let semantic_projection = semantic_stamp.projection();
    if !semantic_projection
        .representation()
        .same_generation(attachment.representation())
    {
        return Err(InteractionIntentError::RepresentationMismatch);
    }
    if !semantic_projection.same_state(attachment.projection()) {
        return Err(InteractionIntentError::ProjectionMismatch);
    }
    if !semantic_stamp.coordinate().same_incarnation(attachment.coordinate()) {
        return Err(InteractionIntentError::CoordinateMismatch);
    }
    match session.bounds().requests().validate_charge(charge) {
        Ok(()) => {}
        Err(error) => return Err(InteractionIntentError::Bound(error)),
    }
    let reserved = charge.intent_bytes().value();
    let actual = semantic.shape().bytes().value();
    if reserved != actual {
        return Err(InteractionIntentError::IntentByteChargeMismatch {
            reserved,
            actual,
        });
    }
    if cancellation_code(cancellation) != 0 {
        return Err(InteractionIntentError::Cancelled);
    }
    if deadline as u8 != DeadlineState::Pending as u8 {
        return Err(InteractionIntentError::DeadlineReached);
    }
    Ok(())
}

const fn cancellation_code(state: CancellationState) -> u8 {
    match state {
        CancellationState::Open => 0,
        CancellationState::Requested(_) => 1,
    }
}

const fn same_contract(
    left: crate::operation::ContractSelection,
    right: crate::operation::ContractSelection,
) -> bool {
    left.identity() == right.identity()
        && left.selected() == right.selected()
        && left.minimum() == right.minimum()
        && left.maximum() == right.maximum()
}
