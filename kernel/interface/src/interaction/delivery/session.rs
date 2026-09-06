//! Session negotiation, Representation attachment, and close values.

use core::fmt;

use crate::bound::{BoundDescriptor, BoundError, BoundUnit, CheckedCount};
use crate::cancellation::ExecutionOutcome;

use super::super::bounds::{
    InteractionBoundField, InteractionBoundViolation, InteractionSessionBounds,
    RepresentationResynchronizationTotals,
};
use super::super::descriptor::{
    InteractionSessionCapabilities, InteractionSessionDescriptor,
    RepresentationAttachmentDescriptor,
};
use super::super::identity::RepresentationResynchronizationIdentity;
use super::super::session::{
    InteractionResourceScope, InteractionSessionHandle, InteractionSessionStamp,
};
use super::super::update::{
    RepresentationUpdate, RepresentationUpdateError,
    RepresentationUpdateLineage,
};

/// Typed structural mismatch in one Session negotiation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionNegotiationRequestError {
    /// The request targets another Session or Client generation.
    SessionMismatch,
    /// The request names another Resource and Buffer scope.
    ScopeMismatch,
    /// The request selects another capability set.
    CapabilityMismatch,
}

impl fmt::Display for InteractionNegotiationRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => {
                "negotiation targets another Interaction Session"
            }
            Self::ScopeMismatch => "negotiation targets another Resource scope",
            Self::CapabilityMismatch => {
                "negotiation selects another capability set"
            }
        })
    }
}

impl core::error::Error for InteractionNegotiationRequestError {}

/// Caller-owned exact Session negotiation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionNegotiationRequest {
    session: InteractionSessionStamp,
    scope: InteractionResourceScope,
    capabilities: InteractionSessionCapabilities,
}

impl InteractionNegotiationRequest {
    /// Constructs a request matching one selected endpoint descriptor.
    ///
    /// # Errors
    ///
    /// Returns the first Session, scope, or capability mismatch.
    pub const fn new(
        descriptor: &InteractionSessionDescriptor,
        session: InteractionSessionStamp,
        scope: InteractionResourceScope,
        capabilities: InteractionSessionCapabilities,
    ) -> Result<Self, InteractionNegotiationRequestError> {
        if !session.same_generation(descriptor.stamp()) {
            return Err(InteractionNegotiationRequestError::SessionMismatch);
        }
        if !scope.same_scope(descriptor.scope()) {
            return Err(InteractionNegotiationRequestError::ScopeMismatch);
        }
        if !same_capabilities(capabilities, descriptor.capabilities()) {
            return Err(InteractionNegotiationRequestError::CapabilityMismatch);
        }
        Ok(Self { session, scope, capabilities })
    }

    /// Returns the complete requested Session stamp.
    #[must_use]
    pub const fn session(self) -> InteractionSessionStamp {
        self.session
    }

    /// Returns the requested Resource and Buffer scope.
    #[must_use]
    pub const fn scope(self) -> InteractionResourceScope {
        self.scope
    }

    /// Returns the exact requested capability set.
    #[must_use]
    pub const fn capabilities(self) -> InteractionSessionCapabilities {
        self.capabilities
    }
}

/// Cause of immediate Session negotiation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionNegotiationFailure {
    /// Authority was denied before Resource lookup.
    Denied,
    /// Contract or capability versions are incompatible.
    Incompatible,
    /// Request structure failed validation.
    InvalidRequest,
    /// Required dependencies have not published readiness.
    NotReady,
    /// Bounded Session capacity is exhausted.
    Pressure,
    /// A newer endpoint generation replaced the requested one.
    Replaced,
    /// Endpoint shutdown has begun.
    Closing,
    /// The endpoint is closed.
    Closed,
    /// Another bounded Runtime failure prevented negotiation.
    RuntimeFailure,
}

/// Observable status of one bounded Session negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionNegotiationStatus {
    /// The exact Session generation is negotiated provisionally.
    Negotiated,
    /// Negotiation retained no Session authority.
    Rejected(InteractionNegotiationFailure),
}

/// Observable result of one bounded Session negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionNegotiationOutcome {
    session: Option<InteractionSessionHandle>,
    failure: Option<InteractionNegotiationFailure>,
}

impl InteractionNegotiationOutcome {
    /// Reports successful provisional negotiation.
    #[must_use]
    pub const fn negotiated(session: InteractionSessionHandle) -> Self {
        Self { session: Some(session), failure: None }
    }

    /// Reports definite negotiation rejection.
    #[must_use]
    pub const fn rejected(failure: InteractionNegotiationFailure) -> Self {
        Self { session: None, failure: Some(failure) }
    }

    /// Returns negotiated or rejected status.
    #[must_use]
    pub const fn status(self) -> InteractionNegotiationStatus {
        match self.failure {
            Some(failure) => InteractionNegotiationStatus::Rejected(failure),
            None => InteractionNegotiationStatus::Negotiated,
        }
    }

    /// Returns the negotiated Session generation when successful.
    #[must_use]
    pub const fn session(self) -> Option<InteractionSessionHandle> {
        self.session
    }
}

/// Typed mismatch in one Representation attachment request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationAttachmentRequestError {
    /// The descriptor belongs to another Session generation.
    SessionMismatch,
}

impl fmt::Display for RepresentationAttachmentRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("attachment request targets another Session")
    }
}

impl core::error::Error for RepresentationAttachmentRequestError {}

/// Move-only request to attach one Representation stream.
pub struct RepresentationAttachmentRequest {
    descriptor: RepresentationAttachmentDescriptor,
}

impl fmt::Debug for RepresentationAttachmentRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentRequest")
            .field("descriptor", &self.descriptor)
            .finish()
    }
}

impl RepresentationAttachmentRequest {
    /// Constructs a request for one exact Session and attachment.
    ///
    /// The owning result returns the descriptor unchanged on mismatch.
    #[must_use]
    pub const fn bind(
        session: &InteractionSessionDescriptor,
        descriptor: RepresentationAttachmentDescriptor,
    ) -> RepresentationAttachmentRequestBindingOutcome {
        if descriptor.handle().session().matches(session.stamp()) {
            return RepresentationAttachmentRequestBindingOutcome::Bound(
                Self { descriptor },
            );
        }
        RepresentationAttachmentRequestBindingOutcome::Rejected(
            RepresentationAttachmentRequestRejection {
                cause: RepresentationAttachmentRequestError::SessionMismatch,
                descriptor,
            },
        )
    }

    /// Returns the exact Session generation.
    #[must_use]
    pub const fn session(&self) -> InteractionSessionHandle {
        self.descriptor.handle().session()
    }

    /// Borrows the complete attachment descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &RepresentationAttachmentDescriptor {
        &self.descriptor
    }

    /// Releases the request into its complete descriptor.
    #[must_use]
    pub const fn into_descriptor(self) -> RepresentationAttachmentDescriptor {
        self.descriptor
    }
}

/// Owning result of binding one Representation attachment request.
pub enum RepresentationAttachmentRequestBindingOutcome {
    /// The descriptor belongs to the selected Session.
    Bound(RepresentationAttachmentRequest),
    /// Binding failed and retains the unchanged descriptor.
    Rejected(RepresentationAttachmentRequestRejection),
}

impl fmt::Debug for RepresentationAttachmentRequestBindingOutcome {
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

/// Rejected attachment request retaining its complete descriptor.
pub struct RepresentationAttachmentRequestRejection {
    cause: RepresentationAttachmentRequestError,
    descriptor: RepresentationAttachmentDescriptor,
}

impl RepresentationAttachmentRequestRejection {
    /// Returns the structural request mismatch.
    #[must_use]
    pub const fn cause(&self) -> RepresentationAttachmentRequestError {
        self.cause
    }

    /// Borrows the unchanged descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &RepresentationAttachmentDescriptor {
        &self.descriptor
    }

    /// Returns descriptor ownership.
    #[must_use]
    pub const fn into_descriptor(self) -> RepresentationAttachmentDescriptor {
        self.descriptor
    }
}

impl fmt::Debug for RepresentationAttachmentRequestRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentRequestRejection")
            .field("cause", &self.cause)
            .field("descriptor", &self.descriptor)
            .finish()
    }
}

/// Cause of definite Representation non-attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationAttachmentFailure {
    /// Session authority denied the attachment.
    Denied,
    /// The Representation or Update capability is unsupported.
    Unsupported,
    /// Contract, carrier, or policy generations are incompatible.
    Incompatible,
    /// The Session has not published readiness.
    NotReady,
    /// The attachment identity is already active.
    DuplicateAttachment,
    /// Session or policy state is stale.
    Stale,
    /// Retention or initial-state capacity is exhausted.
    Pressure,
    /// A newer generation replaced this attachment.
    Replaced,
    /// Session shutdown has begun.
    Closing,
    /// The Session is closed.
    Closed,
    /// Another bounded Runtime failure prevented attachment.
    RuntimeFailure,
}

/// Ready attachment plus its complete initial semantic snapshot.
pub struct RepresentationAttachmentReady {
    descriptor: RepresentationAttachmentDescriptor,
    initial: RepresentationUpdate,
}

impl RepresentationAttachmentReady {
    /// Validates and binds one complete initial snapshot.
    ///
    /// The owning result returns both inputs on validation failure.
    #[must_use]
    pub const fn bind(
        session: &InteractionSessionDescriptor,
        descriptor: RepresentationAttachmentDescriptor,
        initial: RepresentationUpdate,
    ) -> RepresentationAttachmentReadyBindingOutcome {
        let cause = match initial.validate(session, &descriptor) {
            Ok(()) => match initial.lineage() {
                RepresentationUpdateLineage::FullSnapshot { .. } => {
                    return RepresentationAttachmentReadyBindingOutcome::Bound(
                        Self { descriptor, initial },
                    );
                }
                _ => RepresentationAttachmentReadyError::NotFullSnapshot,
            },
            Err(error) => RepresentationAttachmentReadyError::Update(error),
        };
        RepresentationAttachmentReadyBindingOutcome::Rejected(
            RepresentationAttachmentReadyRejection {
                cause,
                descriptor,
                initial,
            },
        )
    }

    /// Borrows the exact attachment descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &RepresentationAttachmentDescriptor {
        &self.descriptor
    }

    /// Borrows the full initial semantic snapshot.
    #[must_use]
    pub const fn initial(&self) -> &RepresentationUpdate {
        &self.initial
    }

    /// Releases the ready value into descriptor and initial Update.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (RepresentationAttachmentDescriptor, RepresentationUpdate) {
        (self.descriptor, self.initial)
    }
}

impl fmt::Debug for RepresentationAttachmentReady {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentReady")
            .field("descriptor", &self.descriptor)
            .field("initial", &self.initial)
            .finish()
    }
}

/// Owning result of binding one complete initial attachment state.
pub enum RepresentationAttachmentReadyBindingOutcome {
    /// Initial full state and descriptor are coherent.
    Bound(RepresentationAttachmentReady),
    /// Binding failed while retaining both complete inputs.
    Rejected(RepresentationAttachmentReadyRejection),
}

impl fmt::Debug for RepresentationAttachmentReadyBindingOutcome {
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

/// Typed failure to build a ready initial attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationAttachmentReadyError {
    /// Initial Update fails descriptor or carrier validation.
    Update(RepresentationUpdateError),
    /// Initial Ready state must be a complete full snapshot.
    NotFullSnapshot,
}

impl fmt::Display for RepresentationAttachmentReadyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Update(_) => "initial attachment Update is invalid",
            Self::NotFullSnapshot => {
                "initial attachment Update is not a full snapshot"
            }
        })
    }
}

impl core::error::Error for RepresentationAttachmentReadyError {}

/// Rejected Ready construction retaining descriptor and initial Update.
pub struct RepresentationAttachmentReadyRejection {
    cause: RepresentationAttachmentReadyError,
    descriptor: RepresentationAttachmentDescriptor,
    initial: RepresentationUpdate,
}

impl RepresentationAttachmentReadyRejection {
    /// Returns the validation failure.
    #[must_use]
    pub const fn cause(&self) -> RepresentationAttachmentReadyError {
        self.cause
    }

    /// Borrows the unchanged attachment descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &RepresentationAttachmentDescriptor {
        &self.descriptor
    }

    /// Borrows the unchanged initial Update.
    #[must_use]
    pub const fn initial(&self) -> &RepresentationUpdate {
        &self.initial
    }

    /// Releases both unchanged inputs.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (RepresentationAttachmentDescriptor, RepresentationUpdate) {
        (self.descriptor, self.initial)
    }
}

impl fmt::Debug for RepresentationAttachmentReadyRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentReadyRejection")
            .field("cause", &self.cause)
            .field("descriptor", &self.descriptor)
            .field("initial", &self.initial)
            .finish()
    }
}

/// Provisional attachment waiting for a full resynchronization Update.
pub struct RepresentationAttachmentResynchronizing {
    descriptor: RepresentationAttachmentDescriptor,
    resynchronization: RepresentationResynchronizationIdentity,
}

impl fmt::Debug for RepresentationAttachmentResynchronizing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentResynchronizing")
            .field("descriptor", &self.descriptor)
            .field("resynchronization", &self.resynchronization)
            .finish()
    }
}

impl RepresentationAttachmentResynchronizing {
    /// Constructs a provisional resynchronizing attachment.
    #[must_use]
    pub const fn new(
        descriptor: RepresentationAttachmentDescriptor,
        resynchronization: RepresentationResynchronizationIdentity,
    ) -> Self {
        Self { descriptor, resynchronization }
    }

    /// Borrows the exact attachment descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &RepresentationAttachmentDescriptor {
        &self.descriptor
    }

    /// Returns the required resynchronization identity.
    #[must_use]
    pub const fn resynchronization(
        &self,
    ) -> RepresentationResynchronizationIdentity {
        self.resynchronization
    }

    /// Returns descriptor ownership.
    #[must_use]
    pub const fn into_descriptor(self) -> RepresentationAttachmentDescriptor {
        self.descriptor
    }
}

/// Definite non-attachment retaining the complete request.
pub struct RepresentationAttachmentRejection {
    cause: RepresentationAttachmentFailure,
    request: RepresentationAttachmentRequest,
}

impl fmt::Debug for RepresentationAttachmentRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentRejection")
            .field("cause", &self.cause)
            .field("request", &self.request)
            .finish()
    }
}

impl RepresentationAttachmentRejection {
    /// Constructs a definite rejection without retaining attachment state.
    #[must_use]
    pub const fn new(
        cause: RepresentationAttachmentFailure,
        request: RepresentationAttachmentRequest,
    ) -> Self {
        Self { cause, request }
    }

    /// Returns the non-attachment cause.
    #[must_use]
    pub const fn cause(&self) -> RepresentationAttachmentFailure {
        self.cause
    }

    /// Borrows the returned request.
    #[must_use]
    pub const fn request(&self) -> &RepresentationAttachmentRequest {
        &self.request
    }

    /// Returns complete request ownership.
    #[must_use]
    pub const fn into_request(self) -> RepresentationAttachmentRequest {
        self.request
    }
}

/// Terminal status of one bounded attachment call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationAttachmentStatus {
    /// Initial full state is ready for Client installation.
    Ready,
    /// Attachment remains provisional pending full resynchronization.
    Resynchronizing,
    /// No attachment state or request ownership was retained.
    Rejected(RepresentationAttachmentFailure),
}

/// Terminal result of one bounded attachment call.
pub struct RepresentationAttachmentOutcome {
    ready: Option<RepresentationAttachmentReady>,
    resynchronizing: Option<RepresentationAttachmentResynchronizing>,
    rejected: Option<RepresentationAttachmentRejection>,
}

impl RepresentationAttachmentOutcome {
    /// Returns one ready initial-state transfer.
    #[must_use]
    pub const fn ready(value: RepresentationAttachmentReady) -> Self {
        Self { ready: Some(value), resynchronizing: None, rejected: None }
    }

    /// Returns one provisional resynchronizing attachment.
    #[must_use]
    pub const fn resynchronizing(
        value: RepresentationAttachmentResynchronizing,
    ) -> Self {
        Self { ready: None, resynchronizing: Some(value), rejected: None }
    }

    /// Returns one definite non-attachment.
    #[must_use]
    pub const fn rejected(value: RepresentationAttachmentRejection) -> Self {
        Self { ready: None, resynchronizing: None, rejected: Some(value) }
    }

    /// Returns ready, resynchronizing, or rejected status.
    #[must_use]
    pub const fn status(&self) -> RepresentationAttachmentStatus {
        match &self.rejected {
            Some(value) => {
                RepresentationAttachmentStatus::Rejected(value.cause())
            }
            None if self.ready.is_some() => {
                RepresentationAttachmentStatus::Ready
            }
            None => RepresentationAttachmentStatus::Resynchronizing,
        }
    }

    /// Borrows ready initial state when present.
    #[must_use]
    pub const fn ready_state(&self) -> Option<&RepresentationAttachmentReady> {
        self.ready.as_ref()
    }

    /// Borrows provisional resynchronization state when present.
    #[must_use]
    pub const fn resynchronizing_state(
        &self,
    ) -> Option<&RepresentationAttachmentResynchronizing> {
        self.resynchronizing.as_ref()
    }

    /// Borrows definite non-attachment when present.
    #[must_use]
    pub const fn rejection(
        &self,
    ) -> Option<&RepresentationAttachmentRejection> {
        self.rejected.as_ref()
    }
}

impl fmt::Debug for RepresentationAttachmentOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationAttachmentOutcome")
            .field("status", &self.status())
            .field("ready", &self.ready)
            .field("resynchronizing", &self.resynchronizing)
            .field("rejected", &self.rejected)
            .finish()
    }
}

/// Caller-owned bounded close request for one Session generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionCloseRequest {
    session: InteractionSessionHandle,
    work: CheckedCount,
}

impl InteractionCloseRequest {
    /// Constructs one close request under the Session work bound.
    ///
    /// # Errors
    ///
    /// Returns zero or bounded close-work exhaustion.
    pub const fn new(
        descriptor: &InteractionSessionDescriptor,
        work: u64,
    ) -> Result<Self, super::super::bounds::InteractionBoundViolation> {
        let work = match descriptor.bounds().prepare_close_work(work) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(Self { session: descriptor.stamp().handle(), work })
    }

    /// Returns the exact Session generation to close.
    #[must_use]
    pub const fn session(self) -> InteractionSessionHandle {
        self.session
    }

    /// Returns the reserved abstract close work.
    #[must_use]
    pub const fn work(self) -> CheckedCount {
        self.work
    }
}

/// Typed incoherence in an incomplete Session close report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCloseError {
    /// No nonterminal execution or retained obligation remains.
    NoRetainedOwnership,
    /// One reported retained count exceeds its Session bound.
    Bound(InteractionBoundViolation),
}

impl fmt::Display for InteractionCloseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NoRetainedOwnership => {
                "incomplete Interaction close retains no owned obligation"
            }
            Self::Bound(_) => {
                "incomplete Interaction close exceeds a Session bound"
            }
        })
    }
}

impl core::error::Error for InteractionCloseError {}

/// Delivery and reconciliation ownership retained during incomplete close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionCloseRetainedState {
    pending_intents: CheckedCount,
    retained_outcomes: CheckedCount,
    retained_updates: CheckedCount,
    unacknowledged_updates: CheckedCount,
    reconciliations: CheckedCount,
    resynchronization: Option<RepresentationResynchronizationTotals>,
}

impl InteractionCloseRetainedState {
    /// Groups exact retained values before Session-bound validation.
    #[must_use]
    pub const fn new(
        pending_intents: CheckedCount,
        retained_outcomes: CheckedCount,
        retained_updates: CheckedCount,
        unacknowledged_updates: CheckedCount,
        reconciliations: CheckedCount,
        resynchronization: Option<RepresentationResynchronizationTotals>,
    ) -> Self {
        Self {
            pending_intents,
            retained_outcomes,
            retained_updates,
            unacknowledged_updates,
            reconciliations,
            resynchronization,
        }
    }
}

/// Bounded ownership retained after a Session failed to close fully.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionIncompleteClose {
    execution: ExecutionOutcome,
    retained: InteractionCloseRetainedState,
}

impl InteractionIncompleteClose {
    /// Constructs a report with at least one retained obligation.
    ///
    /// # Errors
    ///
    /// Returns incoherence when execution and all counts are terminal/zero.
    pub const fn new(
        bounds: &InteractionSessionBounds,
        execution: ExecutionOutcome,
        retained: InteractionCloseRetainedState,
    ) -> Result<Self, InteractionCloseError> {
        match validate_close_counts(bounds, retained) {
            Ok(()) => {}
            Err(error) => return Err(InteractionCloseError::Bound(error)),
        }
        if execution.is_terminal() && retained_is_empty(retained) {
            return Err(InteractionCloseError::NoRetainedOwnership);
        }
        Ok(Self { execution, retained })
    }

    /// Returns the independent execution disposition.
    #[must_use]
    pub const fn execution(self) -> ExecutionOutcome {
        self.execution
    }

    /// Returns pending admitted Intent ownership.
    #[must_use]
    pub const fn pending_intents(self) -> CheckedCount {
        self.retained.pending_intents
    }

    /// Returns terminal Outcomes retained for delivery/reconciliation.
    #[must_use]
    pub const fn retained_outcomes(self) -> CheckedCount {
        self.retained.retained_outcomes
    }

    /// Returns retained Update items.
    #[must_use]
    pub const fn retained_updates(self) -> CheckedCount {
        self.retained.retained_updates
    }

    /// Returns delivered Updates whose credit is not acknowledged.
    #[must_use]
    pub const fn unacknowledged_updates(self) -> CheckedCount {
        self.retained.unacknowledged_updates
    }

    /// Returns unresolved reconciliation obligations.
    #[must_use]
    pub const fn reconciliations(self) -> CheckedCount {
        self.retained.reconciliations
    }

    /// Returns admitted resynchronization ownership when retained.
    #[must_use]
    pub const fn resynchronization(
        self,
    ) -> Option<RepresentationResynchronizationTotals> {
        self.retained.resynchronization
    }
}

const fn validate_close_counts(
    bounds: &InteractionSessionBounds,
    retained: InteractionCloseRetainedState,
) -> Result<(), InteractionBoundViolation> {
    let requests = bounds.requests();
    let updates = bounds.updates();
    let values = [
        (
            InteractionBoundField::PendingIntents,
            requests.pending_intents(),
            retained.pending_intents,
        ),
        (
            InteractionBoundField::RetainedOutcomes,
            requests.retained_outcomes(),
            retained.retained_outcomes,
        ),
        (
            InteractionBoundField::RetainedUpdates,
            updates.retained_updates(),
            retained.retained_updates,
        ),
        (
            InteractionBoundField::UpdateCredits,
            updates.credits(),
            retained.unacknowledged_updates,
        ),
        (
            InteractionBoundField::RetainedOutcomes,
            requests.retained_outcomes(),
            retained.reconciliations,
        ),
    ];
    let mut index = 0;
    while index < values.len() {
        let (field, descriptor, value) = values[index];
        match validate_close_count(field, descriptor, value) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        index += 1;
    }
    match retained.resynchronization {
        Some(totals) => validate_close_resynchronization(bounds, totals),
        None => Ok(()),
    }
}

const fn validate_close_resynchronization(
    bounds: &InteractionSessionBounds,
    totals: RepresentationResynchronizationTotals,
) -> Result<(), InteractionBoundViolation> {
    let updates = bounds.updates();
    if totals.charge_owner().value() != bounds.charge_owner().value() {
        return Err(InteractionBoundViolation::ChargeOwnerMismatch {
            expected: bounds.charge_owner(),
            actual: totals.charge_owner(),
        });
    }
    match validate_close_count(
        InteractionBoundField::Resynchronizations,
        updates.resynchronizations(),
        totals.requests(),
    ) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_close_count(
        InteractionBoundField::ResynchronizationWork,
        updates.resynchronization_work(),
        totals.work(),
    ) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    validate_close_bytes(
        InteractionBoundField::ResynchronizationBytes,
        updates.resynchronization_bytes(),
        totals.bytes().value(),
    )
}

const fn validate_close_count(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    value: CheckedCount,
) -> Result<(), InteractionBoundViolation> {
    let maximum = descriptor.aggregate().value();
    let requested = value.value();
    if requested > maximum {
        return Err(InteractionBoundViolation::new(
            field,
            BoundError::Exhausted {
                unit: BoundUnit::Count,
                requested,
                maximum,
            },
        ));
    }
    Ok(())
}

const fn validate_close_bytes(
    field: InteractionBoundField,
    descriptor: BoundDescriptor,
    requested: u64,
) -> Result<(), InteractionBoundViolation> {
    let maximum = descriptor.aggregate().value();
    if requested > maximum {
        return Err(InteractionBoundViolation::new(
            field,
            BoundError::Exhausted {
                unit: BoundUnit::Bytes,
                requested,
                maximum,
            },
        ));
    }
    Ok(())
}

/// Idempotent terminal result of one Session close request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCloseOutcome {
    /// Execution and every retained delivery/release obligation are terminal.
    Closed,
    /// Bounded close ended while ownership remains explicit.
    Incomplete(InteractionIncompleteClose),
    /// The close request targets another Session generation.
    StaleSession,
}

const fn retained_is_empty(retained: InteractionCloseRetainedState) -> bool {
    if retained.resynchronization.is_some() {
        return false;
    }
    all_zero([
        retained.pending_intents,
        retained.retained_outcomes,
        retained.retained_updates,
        retained.unacknowledged_updates,
        retained.reconciliations,
    ])
}

const fn all_zero(values: [CheckedCount; 5]) -> bool {
    let mut index = 0;
    while index < values.len() {
        if values[index].value() != 0 {
            return false;
        }
        index += 1;
    }
    true
}

const fn same_capabilities(
    left: InteractionSessionCapabilities,
    right: InteractionSessionCapabilities,
) -> bool {
    support_code(left.cancellation()) == support_code(right.cancellation())
        && support_code(left.reconciliation())
            == support_code(right.reconciliation())
        && support_code(left.updates()) == support_code(right.updates())
        && support_code(left.deltas()) == support_code(right.deltas())
        && support_code(left.resynchronization())
            == support_code(right.resynchronization())
}

const fn support_code(
    value: super::super::descriptor::InteractionCapabilitySupport,
) -> u8 {
    match value {
        super::super::descriptor::InteractionCapabilitySupport::Unsupported => {
            0
        }
        super::super::descriptor::InteractionCapabilitySupport::Supported => 1,
    }
}
