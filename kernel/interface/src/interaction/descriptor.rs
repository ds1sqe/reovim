//! Interaction Session capabilities, operations, and attachment descriptors.

use core::fmt;

use crate::bound::CheckedCount;
use crate::identity::InstanceId;
use crate::operation::OperationDescriptor;
use crate::semantic::descriptor::RepresentationPolicyDescriptor;
use crate::semantic::stamp::{
    CoordinateSpaceStamp, ProjectionStamp, RepresentationPolicyStamp,
};

use super::bounds::{InteractionBoundViolation, InteractionSessionBounds};
use super::session::{
    InteractionResourceScope, InteractionSessionStamp,
    RepresentationAttachmentHandle, RepresentationUpdateStreamStamp,
};
use super::update::{
    RepresentationUpdateContract, RepresentationUpdateError,
    RepresentationUpdateLineage, RepresentationUpdatePayloadClaim,
    RepresentationUpdatePreparation, RepresentationUpdateStamp,
};

/// Whether one optional Session capability is selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCapabilitySupport {
    /// The operation is unavailable and returns typed unsupported.
    Unsupported,
    /// The operation is selected under the Session descriptor.
    Supported,
}

/// Selected optional operations for one Session generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionSessionCapabilities {
    cancellation: InteractionCapabilitySupport,
    reconciliation: InteractionCapabilitySupport,
    updates: InteractionCapabilitySupport,
    deltas: InteractionCapabilitySupport,
    resynchronization: InteractionCapabilitySupport,
}

impl InteractionSessionCapabilities {
    /// Constructs one capability set after cross-capability validation.
    ///
    /// # Errors
    ///
    /// Returns a typed incoherence when a child capability lacks Updates.
    pub const fn new(
        cancellation: InteractionCapabilitySupport,
        reconciliation: InteractionCapabilitySupport,
        updates: InteractionCapabilitySupport,
        deltas: InteractionCapabilitySupport,
        resynchronization: InteractionCapabilitySupport,
    ) -> Result<Self, InteractionCapabilityError> {
        if unsupported(updates) && supported(deltas) {
            return Err(InteractionCapabilityError::DeltaWithoutUpdates);
        }
        if unsupported(updates) && supported(resynchronization) {
            return Err(
                InteractionCapabilityError::ResynchronizationWithoutUpdates,
            );
        }
        Ok(Self {
            cancellation,
            reconciliation,
            updates,
            deltas,
            resynchronization,
        })
    }

    /// Returns whether cooperative cancellation is selected.
    #[must_use]
    pub const fn cancellation(self) -> InteractionCapabilitySupport {
        self.cancellation
    }

    /// Returns whether Outcome reconciliation is selected.
    #[must_use]
    pub const fn reconciliation(self) -> InteractionCapabilitySupport {
        self.reconciliation
    }

    /// Returns whether Representation Updates are selected.
    #[must_use]
    pub const fn updates(self) -> InteractionCapabilitySupport {
        self.updates
    }

    /// Returns whether predecessor-bound deltas are selected.
    #[must_use]
    pub const fn deltas(self) -> InteractionCapabilitySupport {
        self.deltas
    }

    /// Returns whether full resynchronization is selected.
    #[must_use]
    pub const fn resynchronization(self) -> InteractionCapabilitySupport {
        self.resynchronization
    }
}

/// Typed incoherence in selected Session capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCapabilityError {
    /// Delta delivery requires the Update stream capability.
    DeltaWithoutUpdates,
    /// Resynchronization requires the Update stream capability.
    ResynchronizationWithoutUpdates,
}

impl fmt::Display for InteractionCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DeltaWithoutUpdates => {
                "Interaction delta capability requires Update delivery"
            }
            Self::ResynchronizationWithoutUpdates => {
                "Interaction resynchronization requires Update delivery"
            }
        })
    }
}

impl core::error::Error for InteractionCapabilityError {}

/// Cross-Session fairness guarantee without selecting a scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionFairness {
    /// Pressure in one Session cannot consume another Session's reservation.
    IndependentSessionProgress,
}

/// Terminal Outcome retention selected by the first Session contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionOutcomeRetention {
    /// Retain until transfer, bounded expiry, reconciliation, or close.
    UntilDeliveredReconciledOrClosed,
}

/// Update retention selected by the first Session contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateRetention {
    /// Retain a bounded cursor window and emit Gap on loss.
    BoundedCursorWindow,
}

/// Exact execution classification of all public Session operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionSessionOperations {
    negotiate: OperationDescriptor,
    attach: OperationDescriptor,
    submit_intent: OperationDescriptor,
    request_cancellation: OperationDescriptor,
    take_outcome: OperationDescriptor,
    reconcile_outcome: OperationDescriptor,
    take_update: OperationDescriptor,
    acknowledge_update: OperationDescriptor,
    request_resynchronization: OperationDescriptor,
    close: OperationDescriptor,
}

impl InteractionSessionOperations {
    /// Constructs the exact first Session operation family.
    #[must_use]
    pub const fn required() -> Self {
        let inline = OperationDescriptor::local_inline();
        Self {
            negotiate: inline,
            attach: inline,
            submit_intent:
                OperationDescriptor::local_deferred_callback_admission(),
            request_cancellation: inline,
            take_outcome: inline,
            reconcile_outcome: inline,
            take_update: OperationDescriptor::local_deferred_stream_admission(),
            acknowledge_update: inline,
            request_resynchronization:
                OperationDescriptor::local_deferred_callback_admission(),
            close: OperationDescriptor::local_deferred(),
        }
    }

    /// Returns the Session negotiation classification.
    #[must_use]
    pub const fn negotiate(self) -> OperationDescriptor {
        self.negotiate
    }

    /// Returns the Representation attachment classification.
    #[must_use]
    pub const fn attach(self) -> OperationDescriptor {
        self.attach
    }

    /// Returns the semantic Intent submission classification.
    #[must_use]
    pub const fn submit_intent(self) -> OperationDescriptor {
        self.submit_intent
    }

    /// Returns the cancellation-request classification.
    #[must_use]
    pub const fn request_cancellation(self) -> OperationDescriptor {
        self.request_cancellation
    }

    /// Returns the terminal Outcome retrieval classification.
    #[must_use]
    pub const fn take_outcome(self) -> OperationDescriptor {
        self.take_outcome
    }

    /// Returns the Outcome reconciliation classification.
    #[must_use]
    pub const fn reconcile_outcome(self) -> OperationDescriptor {
        self.reconcile_outcome
    }

    /// Returns the Update stream classification.
    #[must_use]
    pub const fn take_update(self) -> OperationDescriptor {
        self.take_update
    }

    /// Returns the Update acknowledgement classification.
    #[must_use]
    pub const fn acknowledge_update(self) -> OperationDescriptor {
        self.acknowledge_update
    }

    /// Returns the resynchronization classification.
    #[must_use]
    pub const fn request_resynchronization(self) -> OperationDescriptor {
        self.request_resynchronization
    }

    /// Returns the bounded close classification.
    #[must_use]
    pub const fn close(self) -> OperationDescriptor {
        self.close
    }
}

/// Typed construction failure for a Session descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionDescriptorError {
    /// The bound owner differs from the Session authority instance.
    ChargeOwnerMismatch {
        /// Owner required by the Session stamp.
        expected: InstanceId,
        /// Owner supplied by the bound family.
        actual: InstanceId,
    },
    /// Update delivery lacks one reserved control slot.
    MissingControlReserve(InteractionBoundViolation),
}

impl fmt::Display for InteractionDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ChargeOwnerMismatch { .. } => {
                "Interaction bounds belong to another Session owner"
            }
            Self::MissingControlReserve(_) => {
                "Interaction Session cannot retain a control Update"
            }
        })
    }
}

impl core::error::Error for InteractionDescriptorError {}

/// Immutable contract governing one selected Session endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionSessionDescriptor {
    stamp: InteractionSessionStamp,
    scope: InteractionResourceScope,
    capabilities: InteractionSessionCapabilities,
    bounds: InteractionSessionBounds,
    update_contract: RepresentationUpdateContract,
    operations: InteractionSessionOperations,
    fairness: InteractionFairness,
    outcome_retention: InteractionOutcomeRetention,
    update_retention: RepresentationUpdateRetention,
}

impl InteractionSessionDescriptor {
    /// Constructs one coherent Session descriptor.
    ///
    /// # Errors
    ///
    /// Returns a charge-owner or control-reserve failure.
    pub const fn new(
        stamp: InteractionSessionStamp,
        scope: InteractionResourceScope,
        capabilities: InteractionSessionCapabilities,
        bounds: InteractionSessionBounds,
        update_contract: RepresentationUpdateContract,
        operations: InteractionSessionOperations,
    ) -> Result<Self, InteractionDescriptorError> {
        let expected = InstanceId::new(stamp.lifecycle().instance());
        let actual = bounds.charge_owner();
        if expected.value() != actual.value() {
            return Err(InteractionDescriptorError::ChargeOwnerMismatch {
                expected,
                actual,
            });
        }
        match bounds.updates().require_control_reserve() {
            Ok(()) => {}
            Err(error) => {
                return Err(InteractionDescriptorError::MissingControlReserve(
                    error,
                ));
            }
        }
        Ok(Self {
            stamp,
            scope,
            capabilities,
            bounds,
            update_contract,
            operations,
            fairness: InteractionFairness::IndependentSessionProgress,
            outcome_retention:
                InteractionOutcomeRetention::UntilDeliveredReconciledOrClosed,
            update_retention:
                RepresentationUpdateRetention::BoundedCursorWindow,
        })
    }

    /// Returns the complete Session authority stamp.
    #[must_use]
    pub const fn stamp(&self) -> InteractionSessionStamp {
        self.stamp
    }

    /// Returns the exact Resource and Buffer scope.
    #[must_use]
    pub const fn scope(&self) -> InteractionResourceScope {
        self.scope
    }

    /// Returns selected optional capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> InteractionSessionCapabilities {
        self.capabilities
    }

    /// Returns all Session-owned bounds.
    #[must_use]
    pub const fn bounds(&self) -> &InteractionSessionBounds {
        &self.bounds
    }

    /// Returns the selected qualified Update carrier contract.
    #[must_use]
    pub const fn update_contract(&self) -> &RepresentationUpdateContract {
        &self.update_contract
    }

    /// Prepares one payload-bearing Update under this exact Session.
    ///
    /// # Errors
    ///
    /// Returns the first attachment, capability, carrier, lineage, or bound
    /// failure before payload allocation.
    pub const fn prepare_update(
        &self,
        attachment: &RepresentationAttachmentDescriptor,
        stamp: RepresentationUpdateStamp,
        lineage: RepresentationUpdateLineage,
        claim: RepresentationUpdatePayloadClaim,
    ) -> Result<RepresentationUpdatePreparation, RepresentationUpdateError>
    {
        super::update::prepare_update(self, attachment, stamp, lineage, claim)
    }

    /// Returns all exact operation classifications.
    #[must_use]
    pub const fn operations(&self) -> InteractionSessionOperations {
        self.operations
    }

    /// Returns the slow-Client isolation guarantee.
    #[must_use]
    pub const fn fairness(&self) -> InteractionFairness {
        self.fairness
    }

    /// Returns the terminal Outcome retention policy.
    #[must_use]
    pub const fn outcome_retention(&self) -> InteractionOutcomeRetention {
        self.outcome_retention
    }

    /// Returns the Update retention policy.
    #[must_use]
    pub const fn update_retention(&self) -> RepresentationUpdateRetention {
        self.update_retention
    }
}

/// Typed incoherence in one Representation attachment descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationAttachmentError {
    /// The attachment belongs to another Session generation.
    SessionMismatch,
    /// The Update stream belongs to another attachment generation.
    StreamMismatch,
    /// The supplied Representation differs from its descriptor.
    RepresentationMismatch,
    /// The Projection belongs to another Representation generation.
    ProjectionMismatch,
    /// The coordinate space differs from its Representation descriptor.
    CoordinateMismatch,
    /// The Session does not support Update delivery.
    UpdatesUnsupported,
    /// Initial data credit violates the Session bound.
    Credit(InteractionBoundViolation),
}

impl fmt::Display for RepresentationAttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => {
                "Representation attachment targets another Session"
            }
            Self::StreamMismatch => {
                "Representation stream targets another attachment"
            }
            Self::RepresentationMismatch => {
                "Representation attachment uses another policy"
            }
            Self::ProjectionMismatch => {
                "Projection belongs to another Representation"
            }
            Self::CoordinateMismatch => {
                "attachment uses another coordinate space"
            }
            Self::UpdatesUnsupported => {
                "Session does not support Representation Updates"
            }
            Self::Credit(_) => "initial Update credit violates Session bounds",
        })
    }
}

impl core::error::Error for RepresentationAttachmentError {}

/// Caller-selected values for one Representation attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationAttachmentSelection {
    handle: RepresentationAttachmentHandle,
    scope: InteractionResourceScope,
    representation: RepresentationPolicyStamp,
    projection: ProjectionStamp,
    coordinate: CoordinateSpaceStamp,
    stream: RepresentationUpdateStreamStamp,
    initial_credit: u64,
}

impl RepresentationAttachmentSelection {
    /// Groups one attachment selection before descriptor validation.
    #[must_use]
    pub const fn new(
        handle: RepresentationAttachmentHandle,
        scope: InteractionResourceScope,
        representation: RepresentationPolicyStamp,
        projection: ProjectionStamp,
        coordinate: CoordinateSpaceStamp,
        stream: RepresentationUpdateStreamStamp,
        initial_credit: u64,
    ) -> Self {
        Self {
            handle,
            scope,
            representation,
            projection,
            coordinate,
            stream,
            initial_credit,
        }
    }

    /// Returns the selected attachment handle.
    #[must_use]
    pub const fn handle(self) -> RepresentationAttachmentHandle {
        self.handle
    }

    /// Returns the selected Resource and Buffer scope.
    #[must_use]
    pub const fn scope(self) -> InteractionResourceScope {
        self.scope
    }

    /// Returns the selected Representation policy generation.
    #[must_use]
    pub const fn representation(self) -> RepresentationPolicyStamp {
        self.representation
    }

    /// Returns the selected Projection predecessor state.
    #[must_use]
    pub const fn projection(self) -> ProjectionStamp {
        self.projection
    }

    /// Returns the selected coordinate-space incarnation.
    #[must_use]
    pub const fn coordinate(self) -> CoordinateSpaceStamp {
        self.coordinate
    }

    /// Returns the selected Update stream incarnation.
    #[must_use]
    pub const fn stream(self) -> RepresentationUpdateStreamStamp {
        self.stream
    }

    /// Returns the requested initial data credit.
    #[must_use]
    pub const fn initial_credit(self) -> u64 {
        self.initial_credit
    }
}

/// Exact Representation and Update-stream attachment contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationAttachmentDescriptor {
    scope: InteractionResourceScope,
    projection: ProjectionStamp,
    coordinate: CoordinateSpaceStamp,
    stream: RepresentationUpdateStreamStamp,
    initial_credit: CheckedCount,
}

impl RepresentationAttachmentDescriptor {
    /// Constructs one coherent attachment descriptor.
    ///
    /// # Errors
    ///
    /// Returns the first Session, semantic, stream, or credit mismatch.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        policy: &RepresentationPolicyDescriptor,
        selection: RepresentationAttachmentSelection,
    ) -> Result<Self, RepresentationAttachmentError> {
        let handle = selection.handle();
        let scope = selection.scope();
        let representation = selection.representation();
        let projection = selection.projection();
        let coordinate = selection.coordinate();
        let stream = selection.stream();
        if !handle.session().matches(session.stamp())
            || !scope.same_scope(session.scope())
        {
            return Err(RepresentationAttachmentError::SessionMismatch);
        }
        if !stream.attachment().same_generation(handle) {
            return Err(RepresentationAttachmentError::StreamMismatch);
        }
        if !representation.same_generation(policy.stamp()) {
            return Err(RepresentationAttachmentError::RepresentationMismatch);
        }
        if !projection.representation().same_generation(representation) {
            return Err(RepresentationAttachmentError::ProjectionMismatch);
        }
        if !coordinate.same_incarnation(policy.coordinate().stamp()) {
            return Err(RepresentationAttachmentError::CoordinateMismatch);
        }
        if unsupported(session.capabilities().updates()) {
            return Err(RepresentationAttachmentError::UpdatesUnsupported);
        }
        let initial_credit = match session
            .bounds()
            .updates()
            .prepare_credit(selection.initial_credit())
        {
            Ok(value) => value,
            Err(error) => {
                return Err(RepresentationAttachmentError::Credit(error));
            }
        };
        Ok(Self { scope, projection, coordinate, stream, initial_credit })
    }

    /// Returns the compact attachment handle.
    #[must_use]
    pub const fn handle(&self) -> RepresentationAttachmentHandle {
        self.stream.attachment()
    }

    /// Returns the exact Resource and Buffer scope.
    #[must_use]
    pub const fn scope(&self) -> InteractionResourceScope {
        self.scope
    }

    /// Returns the exact Representation policy generation.
    #[must_use]
    pub const fn representation(&self) -> RepresentationPolicyStamp {
        self.projection.representation()
    }

    /// Returns the attached Projection predecessor state.
    #[must_use]
    pub const fn projection(&self) -> ProjectionStamp {
        self.projection
    }

    /// Returns the semantic coordinate-space incarnation.
    #[must_use]
    pub const fn coordinate(&self) -> CoordinateSpaceStamp {
        self.coordinate
    }

    /// Returns the exact Update stream incarnation.
    #[must_use]
    pub const fn stream(&self) -> RepresentationUpdateStreamStamp {
        self.stream
    }

    /// Returns the initial granted data credit.
    #[must_use]
    pub const fn initial_credit(&self) -> CheckedCount {
        self.initial_credit
    }
}

const fn supported(value: InteractionCapabilitySupport) -> bool {
    value as u8 == InteractionCapabilitySupport::Supported as u8
}

const fn unsupported(value: InteractionCapabilitySupport) -> bool {
    !supported(value)
}
