//! Qualified Representation Updates and exact Projection lineage.

use core::fmt;

use crate::buffer::{BufferHistoryStamp, BufferStateStamp, BufferVersion};
use crate::carrier::{
    BorrowModel, CarrierSafetyDescriptor, CopyModel, DropModel,
    InvocationModel, LifetimeModel, OwnershipModel, SharingModel,
    ThreadTransfer, TypeMismatch, UnsafeBoundary, UnwindModel,
};
use crate::operation::ContractSelection;
use crate::resource::{
    ResourceRevision, ResourceSnapshotIdentity, ResourceSnapshotStamp,
};
use crate::semantic::identity::ProjectionGeneration;
use crate::semantic::stamp::{CoordinateSpaceStamp, ProjectionStamp};

use super::bounds::{
    InteractionBoundViolation, RepresentationUpdateBounds,
    RepresentationUpdateShape,
};
use super::delivery::update::RepresentationResynchronizationFailure;
use super::descriptor::{
    InteractionCapabilitySupport, InteractionSessionDescriptor,
    RepresentationAttachmentDescriptor,
};
use super::identity::{
    RepresentationResynchronizationIdentity, RepresentationUpdateCursor,
    RepresentationUpdateIdentity,
};
use super::session::RepresentationUpdateStreamHandle;

/// Type identity and selected schema version of an Update carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateCarrierStamp {
    type_identity: u128,
    schema_version: u64,
}

impl RepresentationUpdateCarrierStamp {
    /// Constructs one carrier stamp from authority-supplied scalars.
    #[must_use]
    pub const fn new(type_identity: u128, schema_version: u64) -> Self {
        Self { type_identity, schema_version }
    }

    /// Returns the Update carrier-family identity.
    #[must_use]
    pub const fn type_identity(self) -> u128 {
        self.type_identity
    }

    /// Returns the selected carrier schema version.
    #[must_use]
    pub const fn schema_version(self) -> u64 {
        self.schema_version
    }
}

/// Caller-declared carrier qualification and structural payload shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdatePayloadClaim {
    carrier: RepresentationUpdateCarrierStamp,
    items: u64,
    depth: u64,
    bytes: u64,
}

impl RepresentationUpdatePayloadClaim {
    /// Groups the carrier stamp and raw pre-allocation shape claim.
    #[must_use]
    pub const fn new(
        carrier: RepresentationUpdateCarrierStamp,
        items: u64,
        depth: u64,
        bytes: u64,
    ) -> Self {
        Self { carrier, items, depth, bytes }
    }

    /// Returns the selected carrier stamp.
    #[must_use]
    pub const fn carrier(self) -> RepresentationUpdateCarrierStamp {
        self.carrier
    }

    /// Returns the declared semantic item count.
    #[must_use]
    pub const fn items(self) -> u64 {
        self.items
    }

    /// Returns the declared maximum nesting depth.
    #[must_use]
    pub const fn depth(self) -> u64 {
        self.depth
    }

    /// Returns the declared payload byte count.
    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Selected schema and safety terms for payload-bearing Updates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateContract {
    schema: ContractSelection,
    safety: CarrierSafetyDescriptor,
}

impl RepresentationUpdateContract {
    /// Constructs the exact owned immutable Update carrier contract.
    ///
    /// # Errors
    ///
    /// Returns incoherent safety when declared terms differ from the carrier.
    pub const fn new(
        schema: ContractSelection,
        safety: CarrierSafetyDescriptor,
    ) -> Result<Self, RepresentationUpdateError> {
        if !safety_is_exact(schema, safety) {
            return Err(RepresentationUpdateError::IncoherentSafety);
        }
        Ok(Self { schema, safety })
    }

    /// Returns the selected carrier schema and compatible range.
    #[must_use]
    pub const fn schema(&self) -> ContractSelection {
        self.schema
    }

    /// Returns enforced carrier safety terms.
    #[must_use]
    pub const fn safety(&self) -> CarrierSafetyDescriptor {
        self.safety
    }

    /// Returns the selected type and schema for a new carrier.
    #[must_use]
    pub const fn selected_stamp(&self) -> RepresentationUpdateCarrierStamp {
        RepresentationUpdateCarrierStamp::new(
            self.schema.identity(),
            self.schema.selected(),
        )
    }

    /// Validates one claimed carrier stamp.
    ///
    /// # Errors
    ///
    /// Returns a typed carrier-family or selected-version mismatch.
    pub const fn validate_stamp(
        &self,
        stamp: RepresentationUpdateCarrierStamp,
    ) -> Result<(), RepresentationUpdateError> {
        if stamp.type_identity() != self.schema.identity() {
            return Err(RepresentationUpdateError::TypeMismatch);
        }
        if stamp.schema_version() != self.schema.selected() {
            return Err(RepresentationUpdateError::IncompatibleSchema);
        }
        Ok(())
    }

    const fn prepare_payload(
        &self,
        bounds: &RepresentationUpdateBounds,
        stamp: RepresentationUpdateStamp,
        lineage: RepresentationUpdateLineage,
        claim: RepresentationUpdatePayloadClaim,
    ) -> Result<RepresentationUpdatePreparation, RepresentationUpdateError>
    {
        if !lineage.has_payload() {
            return Err(RepresentationUpdateError::UnexpectedPayload);
        }
        match validate_lineage(stamp.state().predecessor(), lineage) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let carrier = claim.carrier();
        match self.validate_stamp(carrier) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let shape = match bounds.prepare_shape(
            claim.items(),
            claim.depth(),
            claim.bytes(),
        ) {
            Ok(value) => value,
            Err(error) => return Err(RepresentationUpdateError::Bound(error)),
        };
        Ok(RepresentationUpdatePreparation { stamp, lineage, carrier, shape })
    }
}

pub(crate) const fn prepare_update(
    session: &InteractionSessionDescriptor,
    attachment: &RepresentationAttachmentDescriptor,
    stamp: RepresentationUpdateStamp,
    lineage: RepresentationUpdateLineage,
    claim: RepresentationUpdatePayloadClaim,
) -> Result<RepresentationUpdatePreparation, RepresentationUpdateError> {
    match validate_update_stamp(&stamp, session, attachment) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    match validate_capability(lineage, session) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    session.update_contract().prepare_payload(
        session.bounds().updates(),
        stamp,
        lineage,
        claim,
    )
}

/// Cause of a visible Representation Update gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationGapCause {
    /// The required predecessor aged out of bounded retention.
    RetentionEvicted,
    /// Declared coalescing retired one or more intermediate Updates.
    Coalesced,
    /// A Protocol composition reported carrier loss.
    ProtocolLoss,
    /// Update retention exhausted before data delivery.
    Pressure,
    /// Resource observation continuity was lost.
    SourceDiscontinuity,
    /// A newer attachment or Projection generation replaced the old one.
    Replacement,
    /// The Client reported a missing predecessor.
    MissingPredecessor,
    /// The selected provider could not produce the required Projection.
    ProjectionUnavailable,
}

/// Exact Projection-generation meaning of one Update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateLineage {
    /// Complete state replacing one explicitly named prior generation.
    FullSnapshot {
        /// Projection generation replaced by this complete state.
        supersedes: ProjectionGeneration,
        /// Projection generation described by this state.
        result: ProjectionGeneration,
    },
    /// Incremental state applying only to one exact predecessor.
    Delta {
        /// Required current Projection generation.
        predecessor: ProjectionGeneration,
        /// Resulting Projection generation after atomic installation.
        result: ProjectionGeneration,
    },
    /// Notice that one exact Projection generation is no longer current.
    Invalidation {
        /// Invalidated Projection generation.
        invalidated: ProjectionGeneration,
    },
    /// Visible missing ancestry requiring full resynchronization.
    Gap {
        /// Projection generation the Client expected next.
        expected: ProjectionGeneration,
        /// Oldest or current known available frontier.
        available: ProjectionGeneration,
        /// Exact reason ancestry is unavailable.
        cause: RepresentationGapCause,
        /// Identity required for the resynchronization request.
        resynchronization: RepresentationResynchronizationIdentity,
    },
    /// Complete state produced by one exact resynchronization request.
    Resynchronization {
        /// Projection generation described by the complete state.
        result: ProjectionGeneration,
        /// Request identity that produced the complete state.
        resynchronization: RepresentationResynchronizationIdentity,
    },
    /// Terminal failure of admitted recovery; does not reverse any edit.
    ResynchronizationFailed {
        /// Exact admitted recovery request resolved by this control record.
        resynchronization: RepresentationResynchronizationIdentity,
        /// Why no new semantic snapshot was produced or installed.
        cause: RepresentationResynchronizationFailure,
    },
}

impl RepresentationUpdateLineage {
    /// Reports whether this Update must own a semantic payload carrier.
    #[must_use]
    pub const fn has_payload(self) -> bool {
        let kind = lineage_kind(self);
        kind == 0 || kind == 1 || kind == 4
    }

    /// Returns the resulting Projection generation when state is supplied.
    #[must_use]
    pub const fn result(self) -> Option<ProjectionGeneration> {
        match self {
            Self::FullSnapshot { result, .. }
            | Self::Delta { result, .. }
            | Self::Resynchronization { result, .. } => Some(result),
            Self::Invalidation { .. }
            | Self::Gap { .. }
            | Self::ResynchronizationFailed { .. } => None,
        }
    }
}

/// Typed incoherence in a Representation Update stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateStampError {
    /// The attachment belongs to another Session.
    SessionMismatch,
    /// The target lies outside the Session Resource scope.
    ScopeMismatch,
    /// The Projection names another Representation generation.
    RepresentationMismatch,
    /// The Projection differs from the attached Projection series.
    ProjectionMismatch,
    /// The coordinate space differs from the attached incarnation.
    CoordinateMismatch,
}

impl fmt::Display for RepresentationUpdateStampError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionMismatch => {
                "Update attachment belongs to another Session"
            }
            Self::ScopeMismatch => "Update target lies outside Session scope",
            Self::RepresentationMismatch => {
                "Update Projection belongs to another Representation"
            }
            Self::ProjectionMismatch => {
                "Update Projection differs from its attachment"
            }
            Self::CoordinateMismatch => {
                "Update coordinate space differs from its attachment"
            }
        })
    }
}

impl core::error::Error for RepresentationUpdateStampError {}

/// Compact stream, Update, and cursor identity retained on hot paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateIdentityStamp {
    stream: RepresentationUpdateStreamHandle,
    update: RepresentationUpdateIdentity,
    cursor: RepresentationUpdateCursor,
}

impl RepresentationUpdateIdentityStamp {
    const fn new(
        stream: RepresentationUpdateStreamHandle,
        update: RepresentationUpdateIdentity,
        cursor: RepresentationUpdateCursor,
    ) -> Self {
        Self { stream, update, cursor }
    }

    /// Returns the exact descriptor-relative stream incarnation.
    #[must_use]
    pub const fn stream(self) -> RepresentationUpdateStreamHandle {
        self.stream
    }

    /// Returns the unique Update identity.
    #[must_use]
    pub const fn update(self) -> RepresentationUpdateIdentity {
        self.update
    }

    /// Returns the ordered stream cursor.
    #[must_use]
    pub const fn cursor(self) -> RepresentationUpdateCursor {
        self.cursor
    }

    /// Reports equality of stream, Update identity, and cursor.
    #[must_use]
    pub const fn same_update(self, other: Self) -> bool {
        self.stream.same_generation(other.stream)
            && self.update.value() == other.update.value()
            && self.cursor.value() == other.cursor.value()
    }
}

/// Descriptor-relative dynamic Resource, Buffer, and Projection state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateStateStamp {
    revision: ResourceRevision,
    snapshot: ResourceSnapshotIdentity,
    version: BufferVersion,
    history: BufferHistoryStamp,
    predecessor: ProjectionGeneration,
}

impl RepresentationUpdateStateStamp {
    const fn from_expanded(
        target: &BufferStateStamp,
        projection: &ProjectionStamp,
    ) -> Self {
        let source = target.source();
        Self {
            revision: source.revision(),
            snapshot: source.snapshot(),
            version: target.version(),
            history: target.history(),
            predecessor: projection.predecessor(),
        }
    }

    /// Returns the Resource-native Revision.
    #[must_use]
    pub const fn revision(self) -> ResourceRevision {
        self.revision
    }

    /// Returns the immutable Resource Snapshot identity.
    #[must_use]
    pub const fn snapshot(self) -> ResourceSnapshotIdentity {
        self.snapshot
    }

    /// Returns the local Buffer version.
    #[must_use]
    pub const fn version(self) -> BufferVersion {
        self.version
    }

    /// Returns the exact Buffer History state.
    #[must_use]
    pub const fn history(self) -> BufferHistoryStamp {
        self.history
    }

    /// Returns the observed Projection predecessor generation.
    #[must_use]
    pub const fn predecessor(self) -> ProjectionGeneration {
        self.predecessor
    }

    const fn resolve_target(
        self,
        attachment: &RepresentationAttachmentDescriptor,
    ) -> BufferStateStamp {
        let scope = attachment.scope();
        BufferStateStamp::new(
            ResourceSnapshotStamp::new(
                scope.access(),
                self.revision,
                self.snapshot,
            ),
            scope.buffer(),
            self.version,
            self.history,
        )
    }

    const fn resolve_projection(
        self,
        attachment: &RepresentationAttachmentDescriptor,
    ) -> ProjectionStamp {
        let projection = attachment.projection();
        ProjectionStamp::new(
            projection.projection(),
            projection.lifecycle(),
            projection.representation(),
            self.predecessor,
        )
    }
}

/// Complete descriptor-relative state stamp of one Update.
pub struct RepresentationUpdateStamp {
    identity: RepresentationUpdateIdentityStamp,
    state: RepresentationUpdateStateStamp,
}

impl fmt::Debug for RepresentationUpdateStamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationUpdateStamp")
            .field("identity", &self.identity)
            .field("state", &self.state)
            .finish()
    }
}

impl RepresentationUpdateStamp {
    /// Constructs one coherent compact Update stamp from expanded state.
    ///
    /// # Errors
    ///
    /// Returns the first Session, scope, Projection, or coordinate mismatch.
    pub const fn new(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        update: RepresentationUpdateIdentity,
        cursor: RepresentationUpdateCursor,
        target: &BufferStateStamp,
        projection: &ProjectionStamp,
        coordinate: &CoordinateSpaceStamp,
    ) -> Result<Self, RepresentationUpdateStampError> {
        match validate_stamp_inputs(
            session, attachment, target, projection, coordinate,
        ) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        Ok(Self {
            identity: RepresentationUpdateIdentityStamp::new(
                attachment.stream().handle(),
                update,
                cursor,
            ),
            state: RepresentationUpdateStateStamp::from_expanded(
                target, projection,
            ),
        })
    }

    /// Returns compact identity used by delivery and acknowledgement.
    #[must_use]
    pub const fn identity(&self) -> RepresentationUpdateIdentityStamp {
        self.identity
    }

    /// Returns descriptor-relative dynamic state.
    #[must_use]
    pub const fn state(&self) -> RepresentationUpdateStateStamp {
        self.state
    }

    /// Returns the exact descriptor-relative stream incarnation.
    #[must_use]
    pub const fn stream(&self) -> RepresentationUpdateStreamHandle {
        self.identity.stream()
    }

    /// Returns the unique Update identity.
    #[must_use]
    pub const fn update(&self) -> RepresentationUpdateIdentity {
        self.identity.update()
    }

    /// Returns the ordered stream cursor.
    #[must_use]
    pub const fn cursor(&self) -> RepresentationUpdateCursor {
        self.identity.cursor()
    }

    /// Reconstructs the complete Buffer target against its descriptor.
    ///
    /// # Errors
    ///
    /// Returns Session or stream mismatch before resolving static qualifiers.
    pub const fn resolve_target(
        &self,
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
    ) -> Result<BufferStateStamp, RepresentationUpdateError> {
        match validate_update_stamp(self, session, attachment) {
            Ok(()) => Ok(self.state.resolve_target(attachment)),
            Err(error) => Err(error),
        }
    }

    /// Reconstructs the complete Projection against its descriptor.
    ///
    /// # Errors
    ///
    /// Returns Session or stream mismatch before resolving static qualifiers.
    pub const fn resolve_projection(
        &self,
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
    ) -> Result<ProjectionStamp, RepresentationUpdateError> {
        match validate_update_stamp(self, session, attachment) {
            Ok(()) => Ok(self.state.resolve_projection(attachment)),
            Err(error) => Err(error),
        }
    }

    /// Reports equality of identity and every dynamic state stamp.
    #[must_use]
    pub const fn same_update(&self, other: &Self) -> bool {
        self.identity.same_update(other.identity)
            && same_update_state(self.state, other.state)
    }
}

/// Typed carrier, lineage, descriptor, or bound failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepresentationUpdateError {
    /// Declared carrier safety differs from actual carrier behavior.
    IncoherentSafety,
    /// The carrier names another Update family.
    TypeMismatch,
    /// The carrier selects another schema version.
    IncompatibleSchema,
    /// A payload was supplied for a control-only Update.
    UnexpectedPayload,
    /// A payload-bearing Update lacks its qualified carrier.
    MissingPayload,
    /// Projection lineage is not monotonic or does not match its stamp.
    InvalidLineage,
    /// Update targets another Session generation.
    SessionMismatch,
    /// Update targets another stream incarnation.
    StreamMismatch,
    /// Update lies outside the Session Resource scope.
    ScopeMismatch,
    /// Session does not support Representation Update delivery.
    UpdatesUnsupported,
    /// Session does not support delta delivery.
    DeltaUnsupported,
    /// Session does not support resynchronization.
    ResynchronizationUnsupported,
    /// A structural or retention bound rejected the Update.
    Bound(InteractionBoundViolation),
    /// Allocated bytes cannot fit the portable byte count.
    LengthOverflow,
    /// Allocation length differs from the prepared byte count.
    LengthMismatch {
        /// Byte count validated before allocation.
        prepared: u64,
        /// Actual allocated byte count when representable.
        actual: u64,
    },
}

impl fmt::Display for RepresentationUpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::IncoherentSafety => {
                "Representation Update carrier safety is incoherent"
            }
            Self::TypeMismatch => "Representation Update carrier type differs",
            Self::IncompatibleSchema => {
                "Representation Update schema is incompatible"
            }
            Self::UnexpectedPayload => {
                "control-only Representation Update has a payload"
            }
            Self::MissingPayload => "Representation Update requires a payload",
            Self::InvalidLineage => "Representation Update lineage is invalid",
            Self::SessionMismatch => "Update targets another Session",
            Self::StreamMismatch => "Update targets another stream",
            Self::ScopeMismatch => "Update lies outside Session scope",
            Self::UpdatesUnsupported => {
                "Session does not support Representation Updates"
            }
            Self::DeltaUnsupported => "Session does not support deltas",
            Self::ResynchronizationUnsupported => {
                "Session does not support resynchronization"
            }
            Self::Bound(_) => "Representation Update exceeds its bounds",
            Self::LengthOverflow => {
                "Representation Update allocation length overflowed"
            }
            Self::LengthMismatch { .. } => {
                "Representation Update allocation differs from preparation"
            }
        })
    }
}

impl core::error::Error for RepresentationUpdateError {}

/// Pre-allocation proof for one payload-bearing Update.
pub struct RepresentationUpdatePreparation {
    stamp: RepresentationUpdateStamp,
    lineage: RepresentationUpdateLineage,
    carrier: RepresentationUpdateCarrierStamp,
    shape: RepresentationUpdateShape,
}

impl RepresentationUpdatePreparation {
    /// Returns the complete Update stamp.
    #[must_use]
    pub const fn stamp(&self) -> &RepresentationUpdateStamp {
        &self.stamp
    }

    /// Returns the exact Projection lineage.
    #[must_use]
    pub const fn lineage(&self) -> RepresentationUpdateLineage {
        self.lineage
    }

    /// Returns the selected carrier stamp.
    #[must_use]
    pub const fn carrier(&self) -> RepresentationUpdateCarrierStamp {
        self.carrier
    }

    /// Returns the checked payload shape.
    #[must_use]
    pub const fn shape(&self) -> RepresentationUpdateShape {
        self.shape
    }

    /// Binds the exact allocation or returns it unchanged on rejection.
    #[must_use]
    pub fn bind(
        self,
        bytes: ::alloc::boxed::Box<[u8]>,
    ) -> RepresentationUpdateBindingOutcome {
        let actual = match actual_length(&bytes) {
            Ok(value) => value,
            Err(cause) => {
                return RepresentationUpdateBindingOutcome::Rejected(
                    RepresentationUpdateRejection {
                        cause,
                        preparation: self,
                        bytes,
                    },
                );
            }
        };
        let prepared = self.shape.bytes().value();
        if actual != prepared {
            let cause =
                RepresentationUpdateError::LengthMismatch { prepared, actual };
            return RepresentationUpdateBindingOutcome::Rejected(
                RepresentationUpdateRejection {
                    cause,
                    preparation: self,
                    bytes,
                },
            );
        }
        RepresentationUpdateBindingOutcome::Bound(RepresentationUpdateCarrier {
            stamp: self.stamp,
            lineage: self.lineage,
            carrier: self.carrier,
            shape: self.shape,
            bytes,
        })
    }
}

impl fmt::Debug for RepresentationUpdatePreparation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationUpdatePreparation")
            .field("stamp", &self.stamp)
            .field("lineage", &self.lineage)
            .field("carrier", &self.carrier)
            .field("shape", &self.shape)
            .finish_non_exhaustive()
    }
}

/// Result of binding allocated Update payload bytes.
pub enum RepresentationUpdateBindingOutcome {
    /// The allocation exactly matches its prevalidated Update.
    Bound(RepresentationUpdateCarrier),
    /// Binding failed and retains preparation plus unchanged bytes.
    Rejected(RepresentationUpdateRejection),
}

impl fmt::Debug for RepresentationUpdateBindingOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bound(carrier) => {
                formatter.debug_tuple("Bound").field(carrier).finish()
            }
            Self::Rejected(rejection) => {
                formatter.debug_tuple("Rejected").field(rejection).finish()
            }
        }
    }
}

/// Rejected Update allocation with complete ownership preserved.
pub struct RepresentationUpdateRejection {
    cause: RepresentationUpdateError,
    preparation: RepresentationUpdatePreparation,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl RepresentationUpdateRejection {
    /// Returns the binding failure.
    #[must_use]
    pub const fn cause(&self) -> RepresentationUpdateError {
        self.cause
    }

    /// Returns the unchanged preparation.
    #[must_use]
    pub const fn preparation(&self) -> &RepresentationUpdatePreparation {
        &self.preparation
    }

    /// Borrows the unchanged allocation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns preparation and allocation ownership.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (RepresentationUpdatePreparation, ::alloc::boxed::Box<[u8]>) {
        (self.preparation, self.bytes)
    }
}

impl fmt::Debug for RepresentationUpdateRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationUpdateRejection")
            .field("cause", &self.cause)
            .field("preparation", &self.preparation)
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Qualified move-only payload of one Representation Update.
pub struct RepresentationUpdateCarrier {
    stamp: RepresentationUpdateStamp,
    lineage: RepresentationUpdateLineage,
    carrier: RepresentationUpdateCarrierStamp,
    shape: RepresentationUpdateShape,
    bytes: ::alloc::boxed::Box<[u8]>,
}

impl RepresentationUpdateCarrier {
    /// Returns the complete Update stamp.
    #[must_use]
    pub const fn stamp(&self) -> &RepresentationUpdateStamp {
        &self.stamp
    }

    /// Returns the exact Projection lineage.
    #[must_use]
    pub const fn lineage(&self) -> RepresentationUpdateLineage {
        self.lineage
    }

    /// Returns the selected carrier stamp.
    #[must_use]
    pub const fn carrier(&self) -> RepresentationUpdateCarrierStamp {
        self.carrier
    }

    /// Returns the checked payload shape.
    #[must_use]
    pub const fn shape(&self) -> RepresentationUpdateShape {
        self.shape
    }

    /// Borrows the opaque payload bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Releases the carrier and returns its payload bytes.
    #[must_use]
    pub fn into_bytes(self) -> ::alloc::boxed::Box<[u8]> {
        self.bytes
    }

    /// Promotes this qualified carrier into one payload-bearing Update.
    #[must_use]
    pub const fn into_update(self) -> RepresentationUpdate {
        RepresentationUpdate { state: RepresentationUpdateState::Payload(self) }
    }
}

impl fmt::Debug for RepresentationUpdateCarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationUpdateCarrier")
            .field("stamp", &self.stamp)
            .field("lineage", &self.lineage)
            .field("carrier", &self.carrier)
            .field("shape", &self.shape)
            .field("bytes", &"<redacted>")
            .finish()
    }
}

#[derive(Debug)]
enum RepresentationUpdateState {
    Payload(RepresentationUpdateCarrier),
    Control {
        stamp: RepresentationUpdateStamp,
        lineage: RepresentationUpdateLineage,
    },
}

/// One move-only semantic state or control Update.
pub struct RepresentationUpdate {
    state: RepresentationUpdateState,
}

impl RepresentationUpdate {
    /// Binds a payload-free terminal recovery failure at its selected cursor.
    /// Session close reports `Closing` after fencing pending snapshot work;
    /// it does not claim a separate cancellation capability.
    ///
    /// # Errors
    /// Rejects another Session/stream or unsupported recovery capability.
    pub const fn resynchronization_failed(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        stamp: RepresentationUpdateStamp,
        resynchronization: RepresentationResynchronizationIdentity,
        cause: RepresentationResynchronizationFailure,
    ) -> Result<Self, RepresentationUpdateError> {
        let lineage = RepresentationUpdateLineage::ResynchronizationFailed {
            resynchronization,
            cause,
        };
        match validate_update_stamp(&stamp, session, attachment) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_capability(lineage, session) {
            Ok(()) => Ok(Self {
                state: RepresentationUpdateState::Control { stamp, lineage },
            }),
            Err(error) => Err(error),
        }
    }

    /// Constructs one typed invalidation control Update.
    ///
    /// # Errors
    ///
    /// Returns invalid lineage when the generation differs from the stamp.
    pub const fn invalidation(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        stamp: RepresentationUpdateStamp,
        invalidated: ProjectionGeneration,
    ) -> Result<Self, RepresentationUpdateError> {
        let lineage = RepresentationUpdateLineage::Invalidation { invalidated };
        match validate_update_stamp(&stamp, session, attachment) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_capability(lineage, session) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_lineage(stamp.state().predecessor(), lineage) {
            Ok(()) => Ok(Self {
                state: RepresentationUpdateState::Control { stamp, lineage },
            }),
            Err(error) => Err(error),
        }
    }

    /// Constructs one typed Gap control Update.
    ///
    /// # Errors
    ///
    /// Returns invalid lineage when its frontier precedes the expectation.
    pub const fn gap(
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        stamp: RepresentationUpdateStamp,
        expected: ProjectionGeneration,
        available: ProjectionGeneration,
        cause: RepresentationGapCause,
        resynchronization: RepresentationResynchronizationIdentity,
    ) -> Result<Self, RepresentationUpdateError> {
        let lineage = RepresentationUpdateLineage::Gap {
            expected,
            available,
            cause,
            resynchronization,
        };
        match validate_update_stamp(&stamp, session, attachment) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_capability(lineage, session) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_lineage(stamp.state().predecessor(), lineage) {
            Ok(()) => Ok(Self {
                state: RepresentationUpdateState::Control { stamp, lineage },
            }),
            Err(error) => Err(error),
        }
    }

    /// Returns the complete Update stamp.
    #[must_use]
    pub const fn stamp(&self) -> &RepresentationUpdateStamp {
        match &self.state {
            RepresentationUpdateState::Payload(carrier) => carrier.stamp(),
            RepresentationUpdateState::Control { stamp, .. } => stamp,
        }
    }

    /// Returns the exact typed Projection lineage.
    #[must_use]
    pub const fn lineage(&self) -> RepresentationUpdateLineage {
        match &self.state {
            RepresentationUpdateState::Payload(carrier) => carrier.lineage(),
            RepresentationUpdateState::Control { lineage, .. } => *lineage,
        }
    }

    /// Borrows the opaque carrier for payload-bearing shapes.
    #[must_use]
    pub const fn carrier(&self) -> Option<&RepresentationUpdateCarrier> {
        match &self.state {
            RepresentationUpdateState::Payload(carrier) => Some(carrier),
            RepresentationUpdateState::Control { .. } => None,
        }
    }

    /// Releases this Update and returns its payload carrier when present.
    #[must_use]
    pub fn into_carrier(self) -> Option<RepresentationUpdateCarrier> {
        match self.state {
            RepresentationUpdateState::Payload(carrier) => Some(carrier),
            RepresentationUpdateState::Control { .. } => None,
        }
    }

    /// Revalidates this Update against exact Session and attachment state.
    ///
    /// # Errors
    ///
    /// Returns the first authority, semantic, capability, or bound mismatch.
    pub const fn validate(
        &self,
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
    ) -> Result<(), RepresentationUpdateError> {
        match validate_update_binding(self, session, attachment) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        let lineage = self.lineage();
        match validate_lineage(self.stamp().state().predecessor(), lineage) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match validate_capability(lineage, session) {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        match self.carrier() {
            Some(carrier) => validate_update_carrier(carrier, session),
            None if lineage.has_payload() => {
                Err(RepresentationUpdateError::MissingPayload)
            }
            None => Ok(()),
        }
    }
}

impl fmt::Debug for RepresentationUpdate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepresentationUpdate")
            .field("state", &self.state)
            .finish()
    }
}

const fn validate_update_binding(
    update: &RepresentationUpdate,
    session: &InteractionSessionDescriptor,
    attachment: &RepresentationAttachmentDescriptor,
) -> Result<(), RepresentationUpdateError> {
    validate_update_stamp(update.stamp(), session, attachment)
}

const fn validate_update_stamp(
    stamp: &RepresentationUpdateStamp,
    session: &InteractionSessionDescriptor,
    attachment: &RepresentationAttachmentDescriptor,
) -> Result<(), RepresentationUpdateError> {
    if !attachment.handle().session().matches(session.stamp()) {
        return Err(RepresentationUpdateError::SessionMismatch);
    }
    if !stamp.stream().same_generation(attachment.stream().handle()) {
        return Err(RepresentationUpdateError::StreamMismatch);
    }
    let target = stamp.state().resolve_target(attachment);
    if !session.scope().contains(target) {
        return Err(RepresentationUpdateError::ScopeMismatch);
    }
    Ok(())
}

const fn validate_stamp_inputs(
    session: &InteractionSessionDescriptor,
    attachment: &RepresentationAttachmentDescriptor,
    target: &BufferStateStamp,
    projection: &ProjectionStamp,
    coordinate: &CoordinateSpaceStamp,
) -> Result<(), RepresentationUpdateStampError> {
    if !attachment.handle().session().matches(session.stamp()) {
        return Err(RepresentationUpdateStampError::SessionMismatch);
    }
    if !session.scope().contains(*target) {
        return Err(RepresentationUpdateStampError::ScopeMismatch);
    }
    if !projection.representation().same_generation(attachment.representation())
    {
        return Err(RepresentationUpdateStampError::RepresentationMismatch);
    }
    if projection.projection().value()
        != attachment.projection().projection().value()
        || !projection
            .lifecycle()
            .same_generation(attachment.projection().lifecycle())
    {
        return Err(RepresentationUpdateStampError::ProjectionMismatch);
    }
    if !coordinate.same_incarnation(attachment.coordinate()) {
        return Err(RepresentationUpdateStampError::CoordinateMismatch);
    }
    Ok(())
}

const fn same_update_state(
    left: RepresentationUpdateStateStamp,
    right: RepresentationUpdateStateStamp,
) -> bool {
    same_revision(left.revision(), right.revision())
        && left.snapshot().value() == right.snapshot().value()
        && left.version().value() == right.version().value()
        && left.history().same_state(right.history())
        && left.predecessor().value() == right.predecessor().value()
}

const fn same_revision(
    left: ResourceRevision,
    right: ResourceRevision,
) -> bool {
    same_contract(left.schema(), right.schema())
        && left.token() == right.token()
}

const fn same_contract(
    left: ContractSelection,
    right: ContractSelection,
) -> bool {
    left.identity() == right.identity()
        && left.selected() == right.selected()
        && left.minimum() == right.minimum()
        && left.maximum() == right.maximum()
}

const fn validate_capability(
    lineage: RepresentationUpdateLineage,
    session: &InteractionSessionDescriptor,
) -> Result<(), RepresentationUpdateError> {
    let kind = lineage_kind(lineage);
    if !is_supported(session.capabilities().updates()) {
        return Err(RepresentationUpdateError::UpdatesUnsupported);
    }
    if kind == 1 && !is_supported(session.capabilities().deltas()) {
        return Err(RepresentationUpdateError::DeltaUnsupported);
    }
    if (kind == 4 || kind == 5)
        && !is_supported(session.capabilities().resynchronization())
    {
        return Err(RepresentationUpdateError::ResynchronizationUnsupported);
    }
    Ok(())
}

const fn validate_update_carrier(
    carrier: &RepresentationUpdateCarrier,
    session: &InteractionSessionDescriptor,
) -> Result<(), RepresentationUpdateError> {
    match session.update_contract().validate_stamp(carrier.carrier()) {
        Ok(()) => {}
        Err(error) => return Err(error),
    }
    let shape = carrier.shape();
    match session.bounds().updates().prepare_shape(
        shape.items().value(),
        shape.depth().value(),
        shape.bytes().value(),
    ) {
        Ok(_) => Ok(()),
        Err(error) => Err(RepresentationUpdateError::Bound(error)),
    }
}

const fn validate_lineage(
    predecessor: ProjectionGeneration,
    lineage: RepresentationUpdateLineage,
) -> Result<(), RepresentationUpdateError> {
    let observed = predecessor.value();
    let valid = match lineage {
        RepresentationUpdateLineage::FullSnapshot { supersedes, result } => {
            supersedes.value() == observed && result.value() >= observed
        }
        RepresentationUpdateLineage::Delta { predecessor, result } => {
            match predecessor.value().checked_add(1) {
                Some(next) => {
                    predecessor.value() == observed && next == result.value()
                }
                None => false,
            }
        }
        RepresentationUpdateLineage::Invalidation { invalidated } => {
            invalidated.value() == observed
        }
        RepresentationUpdateLineage::Gap { expected, available, .. } => {
            expected.value() == observed && available.value() >= observed
        }
        RepresentationUpdateLineage::Resynchronization { result, .. } => {
            result.value() >= observed
        }
        RepresentationUpdateLineage::ResynchronizationFailed { .. } => true,
    };
    if valid { Ok(()) } else { Err(RepresentationUpdateError::InvalidLineage) }
}

fn actual_length(bytes: &[u8]) -> Result<u64, RepresentationUpdateError> {
    u64::try_from(bytes.len())
        .map_err(|_| RepresentationUpdateError::LengthOverflow)
}

const fn is_supported(value: InteractionCapabilitySupport) -> bool {
    value as u8 == InteractionCapabilitySupport::Supported as u8
}

const fn lineage_kind(lineage: RepresentationUpdateLineage) -> u8 {
    match lineage {
        RepresentationUpdateLineage::FullSnapshot { .. } => 0,
        RepresentationUpdateLineage::Delta { .. } => 1,
        RepresentationUpdateLineage::Invalidation { .. } => 2,
        RepresentationUpdateLineage::Gap { .. } => 3,
        RepresentationUpdateLineage::Resynchronization { .. } => 4,
        RepresentationUpdateLineage::ResynchronizationFailed { .. } => 5,
    }
}

const fn safety_is_exact(
    schema: ContractSelection,
    safety: CarrierSafetyDescriptor,
) -> bool {
    let versions = safety.schema_versions();
    safety.type_id().value() == schema.identity()
        && versions.minimum().value() == schema.minimum()
        && versions.maximum().value() == schema.maximum()
        && memory_code(safety) == 0
        && concurrency_code(safety) == 0
        && invocation_code(safety) == 0
        && mismatch_code(safety.mismatch()) == 0
        && boundary_code(safety.unsafe_boundary()) == 0
}

const fn memory_code(safety: CarrierSafetyDescriptor) -> u8 {
    let memory = safety.memory();
    match (
        memory.ownership(),
        memory.borrowing(),
        memory.copying(),
        memory.dropping(),
        memory.lifetime(),
    ) {
        (
            OwnershipModel::Owned,
            BorrowModel::Shared,
            CopyModel::MoveOnly,
            DropModel::ReleasesOwnedState,
            LifetimeModel::Value,
        ) => 0,
        _ => 1,
    }
}

const fn concurrency_code(safety: CarrierSafetyDescriptor) -> u8 {
    let concurrency = safety.concurrency();
    match (concurrency.thread_transfer(), concurrency.sharing()) {
        (ThreadTransfer::Transferable, SharingModel::Immutable) => 0,
        _ => 1,
    }
}

const fn invocation_code(safety: CarrierSafetyDescriptor) -> u8 {
    let invocation = safety.invocation();
    match (invocation.invocation(), invocation.unwind()) {
        (InvocationModel::DataOnly, UnwindModel::NotInvocable) => 0,
        _ => 1,
    }
}

const fn mismatch_code(mismatch: TypeMismatch) -> u8 {
    match mismatch {
        TypeMismatch::Reject => 0,
    }
}

const fn boundary_code(boundary: UnsafeBoundary) -> u8 {
    match boundary {
        UnsafeBoundary::None => 0,
        UnsafeBoundary::Reviewed(_) => 1,
    }
}
