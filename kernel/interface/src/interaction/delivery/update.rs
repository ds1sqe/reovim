//! Update retrieval, acknowledgement, and resynchronization values.

use core::fmt;

use crate::cancellation::FenceCause;

use super::super::bounds::RepresentationResynchronizationCharge;
use super::super::descriptor::{
    InteractionCapabilitySupport, InteractionSessionDescriptor,
    RepresentationAttachmentDescriptor,
};
use super::super::identity::{
    RepresentationResynchronizationIdentity, RepresentationUpdateCursor,
    RepresentationUpdateIdentity,
};
use super::super::session::RepresentationUpdateStreamHandle;
use super::super::update::{
    RepresentationUpdate, RepresentationUpdateError,
    RepresentationUpdateIdentityStamp, RepresentationUpdateLineage,
};

/// Typed mismatch in one non-waiting Update lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateRequestError {
    /// The attachment belongs to another Session generation.
    SessionMismatch,
    /// The requested stream belongs to another attachment.
    StreamMismatch,
    /// Session does not support Update delivery.
    Unsupported,
}

impl fmt::Display for RepresentationUpdateRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => "Update lookup targets another Session",
            Self::StreamMismatch => "Update lookup targets another stream",
            Self::Unsupported => {
                "Session does not support Representation Updates"
            }
        })
    }
}

impl core::error::Error for RepresentationUpdateRequestError {}

/// Caller-owned non-waiting request for the next ordered Update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateRequest {
    stream: RepresentationUpdateStreamHandle,
    expected: RepresentationUpdateCursor,
}

impl RepresentationUpdateRequest {
    /// Constructs one exact stream and cursor lookup.
    ///
    /// # Errors
    ///
    /// Returns Session, stream, or unsupported-capability failure.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        expected: RepresentationUpdateCursor,
    ) -> Result<Self, RepresentationUpdateRequestError> {
        if !attachment.handle().session().matches(session.stamp()) {
            return Err(RepresentationUpdateRequestError::SessionMismatch);
        }
        if !attachment
            .stream()
            .attachment()
            .same_generation(attachment.handle())
        {
            return Err(RepresentationUpdateRequestError::StreamMismatch);
        }
        if !is_supported(session.capabilities().updates()) {
            return Err(RepresentationUpdateRequestError::Unsupported);
        }
        Ok(Self { stream: attachment.stream().handle(), expected })
    }

    /// Returns the exact Update stream incarnation.
    #[must_use]
    pub const fn stream(self) -> RepresentationUpdateStreamHandle {
        self.stream
    }

    /// Returns the next cursor expected by the Client.
    #[must_use]
    pub const fn expected(self) -> RepresentationUpdateCursor {
        self.expected
    }
}

/// Typed failure binding one retained Update to one exact stream request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateDeliveryError {
    /// The provider supplied no retained Update candidate.
    MissingCandidate,
    /// The Update belongs to another stream incarnation.
    StreamMismatch,
    /// The Update does not occupy the requested cursor.
    CursorMismatch,
}

impl fmt::Display for RepresentationUpdateDeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingCandidate => "provider supplied no Update candidate",
            Self::StreamMismatch => {
                "delivered Update belongs to another stream"
            }
            Self::CursorMismatch => "delivered Update has another cursor",
        })
    }
}

impl core::error::Error for RepresentationUpdateDeliveryError {}

/// Reason one non-waiting stream read transfers no Update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateUnavailable {
    /// No Update is currently ready at the expected cursor.
    Pending,
    /// All granted data credit is awaiting acknowledgement.
    NoCredit,
    /// The supplied cursor is older than the retained next cursor.
    CursorBehind,
    /// The supplied cursor is ahead of the produced frontier.
    CursorAhead,
    /// No active stream matches the supplied stamp.
    UnknownStream,
    /// The exact Update at this cursor already transferred.
    AlreadyDelivered,
    /// A newer stream or attachment generation replaced this one.
    Replaced,
    /// Session shutdown is resolving retained delivery.
    Closing,
    /// The Session is closed with no matching retained Update.
    Closed,
    /// A retained provider value failed the public delivery binding.
    ProviderContractViolation(RepresentationUpdateDeliveryError),
}

/// Status of one non-waiting Update stream read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateDeliveryStatus {
    /// One exact ordered Update transfers and consumes one credit.
    Delivered,
    /// No Update transfers for the stated reason.
    Unavailable(RepresentationUpdateUnavailable),
}

/// One owned Update or typed non-waiting unavailability.
pub struct RepresentationUpdateDelivery {
    update: Option<RepresentationUpdate>,
    unavailable: Option<RepresentationUpdateUnavailable>,
}

impl RepresentationUpdateDelivery {
    /// Takes and binds one exact candidate only after validation succeeds.
    ///
    /// # Errors
    ///
    /// Returns a small typed failure while leaving the owner slot unchanged.
    pub fn try_bind(
        request: &RepresentationUpdateRequest,
        candidate: &mut Option<RepresentationUpdate>,
    ) -> Result<Self, RepresentationUpdateDeliveryError> {
        let Some(update) = candidate.as_ref() else {
            return Err(RepresentationUpdateDeliveryError::MissingCandidate);
        };
        let stamp = update.stamp();
        if !stamp.stream().same_generation(request.stream()) {
            return Err(RepresentationUpdateDeliveryError::StreamMismatch);
        }
        if stamp.cursor().value() != request.expected().value() {
            return Err(RepresentationUpdateDeliveryError::CursorMismatch);
        }
        let Some(update) = candidate.take() else {
            return Err(RepresentationUpdateDeliveryError::MissingCandidate);
        };
        Ok(Self { update: Some(update), unavailable: None })
    }

    /// Reports why no Update transfers.
    #[must_use]
    pub const fn unavailable(cause: RepresentationUpdateUnavailable) -> Self {
        Self { update: None, unavailable: Some(cause) }
    }

    /// Returns delivered or unavailable status.
    #[must_use]
    pub const fn status(&self) -> RepresentationUpdateDeliveryStatus {
        match self.unavailable {
            Some(cause) => {
                RepresentationUpdateDeliveryStatus::Unavailable(cause)
            }
            None => RepresentationUpdateDeliveryStatus::Delivered,
        }
    }

    /// Borrows the delivered Update when present.
    #[must_use]
    pub const fn update(&self) -> Option<&RepresentationUpdate> {
        self.update.as_ref()
    }

    /// Returns delivered Update ownership when present.
    #[must_use]
    pub fn into_update(self) -> Option<RepresentationUpdate> {
        self.update
    }
}

impl fmt::Debug for RepresentationUpdateDelivery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationUpdateDelivery")
            .field("status", &self.status())
            .field("update", &self.update)
            .field("unavailable", &self.unavailable)
            .finish()
    }
}

/// Client observation reported when one Update returns its credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateApplication {
    /// Update installed atomically on its exact predecessor.
    Installed,
    /// The same Update or result generation was already installed.
    Duplicate,
    /// Update is older than current installed state.
    Older,
    /// Delta predecessor is not available; resynchronization is required.
    MissingPredecessor,
    /// Client installation was rejected by an exact generation fence.
    Fenced(FenceCause),
}

/// Typed structural mismatch in one Update acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateAcknowledgementError {
    /// Update belongs to another Session generation.
    SessionMismatch,
    /// Update belongs to another attachment or stream.
    StreamMismatch,
}

impl fmt::Display for RepresentationUpdateAcknowledgementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => {
                "Update acknowledgement targets another Session"
            }
            Self::StreamMismatch => {
                "Update acknowledgement targets another stream"
            }
        })
    }
}

impl core::error::Error for RepresentationUpdateAcknowledgementError {}

/// Caller-owned acknowledgement returning one consumed stream credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateAcknowledgement {
    stamp: RepresentationUpdateIdentityStamp,
    application: RepresentationUpdateApplication,
}

impl RepresentationUpdateAcknowledgement {
    /// Constructs an acknowledgement for one exact delivered Update.
    ///
    /// # Errors
    ///
    /// Returns a Session, attachment, or stream mismatch.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        stamp: RepresentationUpdateIdentityStamp,
        application: RepresentationUpdateApplication,
    ) -> Result<Self, RepresentationUpdateAcknowledgementError> {
        if !attachment.handle().session().matches(session.stamp()) {
            return Err(
                RepresentationUpdateAcknowledgementError::SessionMismatch,
            );
        }
        if !stamp.stream().same_generation(attachment.stream().handle()) {
            return Err(
                RepresentationUpdateAcknowledgementError::StreamMismatch,
            );
        }
        Ok(Self { stamp, application })
    }

    /// Returns the exact delivered Update stamp.
    #[must_use]
    pub const fn stamp(self) -> RepresentationUpdateIdentityStamp {
        self.stamp
    }

    /// Returns the Client-observed installation result.
    #[must_use]
    pub const fn application(self) -> RepresentationUpdateApplication {
        self.application
    }
}

/// Result of acknowledging one Update and returning its credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateAcknowledgementResult {
    /// Acknowledgement was recorded and one data credit returned.
    Accepted,
    /// This exact acknowledgement was already recorded.
    AlreadyRecorded,
    /// No delivered Update matches the supplied identity.
    UnknownUpdate,
    /// The Update identity exists at another cursor.
    WrongCursor,
    /// The stream generation is stale or replaced.
    StaleStream,
    /// Session shutdown is resolving Update ownership.
    Closing,
    /// The Session is closed.
    Closed,
}

/// Typed structural failure for one resynchronization request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationResynchronizationRequestError {
    /// Gap Update fails current Session or attachment validation.
    Update(RepresentationUpdateError),
    /// The supplied Update is not a Gap.
    NotGap,
    /// Session does not support resynchronization.
    Unsupported,
    /// Concurrent resynchronization capacity is exhausted.
    Bound(super::super::bounds::InteractionBoundViolation),
}

impl fmt::Display for RepresentationResynchronizationRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Update(_) => "resynchronization Gap Update is invalid",
            Self::NotGap => "resynchronization requires one exact Gap",
            Self::Unsupported => "Session does not support resynchronization",
            Self::Bound(_) => "resynchronization exceeds the Session bound",
        })
    }
}

impl core::error::Error for RepresentationResynchronizationRequestError {}

/// Typed mismatch between admitted work and its terminal retained Update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationResynchronizationResultError {
    /// The terminal Update belongs to another stream incarnation.
    StreamMismatch,
    /// The Gap cursor cannot advance to a terminal result position.
    CursorOverflow,
    /// The terminal Update is not the exact next stream item.
    CursorMismatch,
    /// The Update is not a resynchronization result.
    NotResynchronization,
    /// The result resolves another resynchronization identity.
    IdentityMismatch,
    /// A resynchronization result lacks its qualified payload.
    MissingPayload,
    /// The result payload exceeds the prepared byte reservation.
    ResultBytesExceeded {
        /// Actual qualified result bytes.
        actual: u64,
        /// Bytes reserved by the admitted request.
        reserved: u64,
    },
}

impl fmt::Display for RepresentationResynchronizationResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StreamMismatch => {
                "resynchronization result belongs to another stream"
            }
            Self::CursorOverflow => {
                "resynchronization Gap cursor cannot advance"
            }
            Self::CursorMismatch => {
                "resynchronization result has another cursor"
            }
            Self::NotResynchronization => {
                "terminal Update is not a resynchronization result"
            }
            Self::IdentityMismatch => {
                "terminal Update resolves another resynchronization"
            }
            Self::MissingPayload => {
                "resynchronization result lacks its payload"
            }
            Self::ResultBytesExceeded { .. } => {
                "resynchronization result exceeds its byte reservation"
            }
        })
    }
}

impl core::error::Error for RepresentationResynchronizationResultError {}

/// Copyable evidence identifying one exact resynchronization submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationResynchronizationReceipt {
    stream: RepresentationUpdateStreamHandle,
    gap: RepresentationUpdateIdentity,
    cursor: RepresentationUpdateCursor,
    resynchronization: RepresentationResynchronizationIdentity,
    charge: RepresentationResynchronizationCharge,
}

impl RepresentationResynchronizationReceipt {
    /// Returns the exact descriptor-relative stream incarnation.
    #[must_use]
    pub const fn stream(self) -> RepresentationUpdateStreamHandle {
        self.stream
    }

    /// Returns the Gap Update identity.
    #[must_use]
    pub const fn gap(self) -> RepresentationUpdateIdentity {
        self.gap
    }

    /// Returns the Gap stream cursor.
    #[must_use]
    pub const fn cursor(self) -> RepresentationUpdateCursor {
        self.cursor
    }

    /// Returns the exact resynchronization identity.
    #[must_use]
    pub const fn resynchronization(
        self,
    ) -> RepresentationResynchronizationIdentity {
        self.resynchronization
    }

    /// Returns the exact prepared count, work, and byte charge.
    #[must_use]
    pub const fn charge(self) -> RepresentationResynchronizationCharge {
        self.charge
    }
}

/// Move-only exact request for full Representation resynchronization.
pub struct RepresentationResynchronizationRequest {
    receipt: RepresentationResynchronizationReceipt,
}

impl fmt::Debug for RepresentationResynchronizationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationResynchronizationRequest")
            .field("receipt", &self.receipt)
            .finish()
    }
}

impl RepresentationResynchronizationRequest {
    /// Constructs a resynchronization request from one exact Gap Update.
    ///
    /// # Errors
    ///
    /// Returns Update, capability, lineage, or bound failure.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        gap: &RepresentationUpdate,
    ) -> Result<Self, RepresentationResynchronizationRequestError> {
        if !is_supported(session.capabilities().resynchronization()) {
            return Err(
                RepresentationResynchronizationRequestError::Unsupported,
            );
        }
        match gap.validate(session, attachment) {
            Ok(()) => {}
            Err(error) => {
                return Err(
                    RepresentationResynchronizationRequestError::Update(error),
                );
            }
        }
        let RepresentationUpdateLineage::Gap { resynchronization, .. } =
            gap.lineage()
        else {
            return Err(RepresentationResynchronizationRequestError::NotGap);
        };
        let charge =
            match session.bounds().updates().prepare_resynchronization() {
                Ok(value) => value,
                Err(error) => {
                    return Err(
                        RepresentationResynchronizationRequestError::Bound(
                            error,
                        ),
                    );
                }
            };
        let stamp = gap.stamp();
        Ok(Self {
            receipt: RepresentationResynchronizationReceipt {
                stream: stamp.stream(),
                gap: stamp.update(),
                cursor: stamp.cursor(),
                resynchronization,
                charge,
            },
        })
    }

    /// Returns non-owning exact admission evidence.
    #[must_use]
    pub const fn receipt(&self) -> RepresentationResynchronizationReceipt {
        self.receipt
    }

    /// Returns the exact descriptor-relative stream incarnation.
    #[must_use]
    pub const fn stream(&self) -> RepresentationUpdateStreamHandle {
        self.receipt.stream()
    }

    /// Returns the Gap Update identity.
    #[must_use]
    pub const fn gap(&self) -> RepresentationUpdateIdentity {
        self.receipt.gap()
    }

    /// Returns the Gap stream cursor.
    #[must_use]
    pub const fn cursor(&self) -> RepresentationUpdateCursor {
        self.receipt.cursor()
    }

    /// Returns the exact resynchronization identity.
    #[must_use]
    pub const fn resynchronization(
        &self,
    ) -> RepresentationResynchronizationIdentity {
        self.receipt.resynchronization()
    }

    /// Returns the prepared count, work, and byte charge.
    #[must_use]
    pub const fn charge(&self) -> RepresentationResynchronizationCharge {
        self.receipt.charge()
    }

    /// Validates the exact terminal Update before releasing this owner.
    ///
    /// # Errors
    ///
    /// Returns the first stream, cursor, identity, payload, or byte mismatch.
    pub const fn validate_result(
        &self,
        update: &RepresentationUpdate,
    ) -> Result<(), RepresentationResynchronizationResultError> {
        let stamp = update.stamp();
        if !self.stream().same_generation(stamp.stream()) {
            return Err(
                RepresentationResynchronizationResultError::StreamMismatch,
            );
        }
        let Some(cursor) = self.cursor().checked_next() else {
            return Err(
                RepresentationResynchronizationResultError::CursorOverflow,
            );
        };
        if cursor.value() != stamp.cursor().value() {
            return Err(
                RepresentationResynchronizationResultError::CursorMismatch,
            );
        }
        let RepresentationUpdateLineage::Resynchronization {
            resynchronization,
            ..
        } = update.lineage()
        else {
            return Err(
                RepresentationResynchronizationResultError::
                    NotResynchronization,
            );
        };
        if self.resynchronization().value() != resynchronization.value() {
            return Err(
                RepresentationResynchronizationResultError::IdentityMismatch,
            );
        }
        let Some(carrier) = update.carrier() else {
            return Err(
                RepresentationResynchronizationResultError::MissingPayload,
            );
        };
        let actual = carrier.shape().bytes().value();
        let reserved = self.charge().bytes().value();
        if actual > reserved {
            return Err(
                RepresentationResynchronizationResultError::
                    ResultBytesExceeded { actual, reserved },
            );
        }
        Ok(())
    }
}

/// Cause of definite immediate resynchronization non-admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationResynchronizationFailure {
    /// Authority was denied before retained Gap lookup.
    Denied,
    /// Full resynchronization is unsupported.
    Unsupported,
    /// Request or carrier selections are incompatible.
    Incompatible,
    /// No active Gap matches the supplied identity.
    UnknownGap,
    /// The same resynchronization request is already admitted.
    DuplicateRequest,
    /// Session, attachment, or stream state is stale.
    Stale,
    /// Bounded work or result capacity is exhausted.
    Pressure,
    /// A newer generation replaced the request target.
    Replaced,
    /// Session shutdown has begun.
    Closing,
    /// The Session is closed.
    Closed,
    /// Another bounded Runtime failure prevented admission.
    RuntimeFailure,
}

/// Immediate ownership result of one resynchronization submission.
pub enum RepresentationResynchronizationAdmission {
    /// Runtime owns the request; the caller receives non-owning evidence.
    Admitted(RepresentationResynchronizationReceipt),
    /// Runtime retained no work or request ownership.
    NotAdmitted {
        /// Exact non-admission cause.
        cause: RepresentationResynchronizationFailure,
        /// Complete returned request.
        request: RepresentationResynchronizationRequest,
    },
}

impl RepresentationResynchronizationAdmission {
    /// Returns admitted evidence while the provider retains request ownership.
    #[must_use]
    pub const fn admitted(
        request: &RepresentationResynchronizationRequest,
    ) -> Self {
        Self::Admitted(request.receipt())
    }
}

impl fmt::Debug for RepresentationResynchronizationAdmission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admitted(receipt) => {
                formatter.debug_tuple("Admitted").field(receipt).finish()
            }
            Self::NotAdmitted { cause, request } => formatter
                .debug_struct("NotAdmitted")
                .field("cause", cause)
                .field("request", request)
                .finish(),
        }
    }
}

const fn is_supported(value: InteractionCapabilitySupport) -> bool {
    value as u8 == InteractionCapabilitySupport::Supported as u8
}
