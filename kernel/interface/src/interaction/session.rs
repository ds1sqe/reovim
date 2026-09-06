//! Exact Session, attachment, stream, and request stamps.

use core::fmt;

use crate::buffer::plan::{
    BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
    BufferHistoryTransitionIdentity, BufferTransactionIdentity,
};
use crate::buffer::{BufferIncarnationStamp, BufferStateStamp};
use crate::lifecycle::LifecycleIdentity;
use crate::operation::ContractSelection;
use crate::resource::ResourceAccessStamp;
use crate::semantic::identity::{
    SemanticIntentIdentity, SemanticRequestIdentity,
};
use crate::semantic::stamp::SemanticIntentStamp;

use super::identity::{
    ClientIdentity, InteractionCancellationIdentity,
    InteractionCorrelationIdentity, InteractionDeadlineIdentity,
    InteractionRequestIdentity, InteractionSessionIdentity,
    RepresentationAttachmentIdentity, RepresentationUpdateStreamIdentity,
};

/// Exact Client identity, lifecycle, and selected role contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionClientStamp {
    client: ClientIdentity,
    lifecycle: LifecycleIdentity,
    role: ContractSelection,
}

impl InteractionClientStamp {
    /// Constructs one exact Client generation and role.
    #[must_use]
    pub const fn new(
        client: ClientIdentity,
        lifecycle: LifecycleIdentity,
        role: ContractSelection,
    ) -> Self {
        Self { client, lifecycle, role }
    }

    /// Returns the stable Client identity.
    #[must_use]
    pub const fn client(self) -> ClientIdentity {
        self.client
    }

    /// Returns the exact Client lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the selected Client role contract.
    #[must_use]
    pub const fn role(self) -> ContractSelection {
        self.role
    }

    /// Reports equality of Client, lifecycle, and selected role.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.client.value() == other.client.value()
            && self.lifecycle.same_generation(other.lifecycle)
            && same_contract(self.role, other.role)
    }
}

/// Complete Contract Session authority stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionSessionStamp {
    session: InteractionSessionIdentity,
    contract: ContractSelection,
    lifecycle: LifecycleIdentity,
    client: InteractionClientStamp,
}

impl InteractionSessionStamp {
    /// Constructs one selected Session generation.
    #[must_use]
    pub const fn new(
        session: InteractionSessionIdentity,
        contract: ContractSelection,
        lifecycle: LifecycleIdentity,
        client: InteractionClientStamp,
    ) -> Self {
        Self { session, contract, lifecycle, client }
    }

    /// Returns the stable Session identity.
    #[must_use]
    pub const fn session(self) -> InteractionSessionIdentity {
        self.session
    }

    /// Returns the selected Session contract.
    #[must_use]
    pub const fn contract(self) -> ContractSelection {
        self.contract
    }

    /// Returns the exact Session authority generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the exact Client and role binding.
    #[must_use]
    pub const fn client(self) -> InteractionClientStamp {
        self.client
    }

    /// Returns the compact handle used by Session operations.
    #[must_use]
    pub const fn handle(self) -> InteractionSessionHandle {
        InteractionSessionHandle::new(self.session, self.lifecycle)
    }

    /// Reports equality of the complete Session authority.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.session.value() == other.session.value()
            && same_contract(self.contract, other.contract)
            && self.lifecycle.same_generation(other.lifecycle)
            && self.client.same_generation(other.client)
    }
}

/// Compact exact handle for one Session generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionSessionHandle {
    session: InteractionSessionIdentity,
    lifecycle: LifecycleIdentity,
}

impl InteractionSessionHandle {
    /// Constructs a compact handle without adding authority.
    #[must_use]
    pub const fn new(
        session: InteractionSessionIdentity,
        lifecycle: LifecycleIdentity,
    ) -> Self {
        Self { session, lifecycle }
    }

    /// Returns the stable Session identity.
    #[must_use]
    pub const fn session(self) -> InteractionSessionIdentity {
        self.session
    }

    /// Returns the exact Session lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Reports equality with one complete Session stamp.
    #[must_use]
    pub const fn matches(self, session: InteractionSessionStamp) -> bool {
        self.session.value() == session.session().value()
            && self.lifecycle.same_generation(session.lifecycle())
    }

    /// Reports equality of the two exact Session generations.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.session.value() == other.session.value()
            && self.lifecycle.same_generation(other.lifecycle)
    }
}

/// Resource Access and Buffer scope granted to one Session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionResourceScope {
    access: ResourceAccessStamp,
    buffer: BufferIncarnationStamp,
}

impl InteractionResourceScope {
    /// Constructs one exact single-Resource Session scope.
    #[must_use]
    pub const fn new(
        access: ResourceAccessStamp,
        buffer: BufferIncarnationStamp,
    ) -> Self {
        Self { access, buffer }
    }

    /// Returns the exact Resource Access generation.
    #[must_use]
    pub const fn access(self) -> ResourceAccessStamp {
        self.access
    }

    /// Returns the exact Buffer incarnation and generation.
    #[must_use]
    pub const fn buffer(self) -> BufferIncarnationStamp {
        self.buffer
    }

    /// Reports whether one target lies inside this Session scope.
    #[must_use]
    pub const fn contains(self, target: BufferStateStamp) -> bool {
        self.access.same_generation(target.source().access())
            && self.buffer.same_generation(target.buffer())
    }

    /// Reports equality of Access and Buffer scope.
    #[must_use]
    pub const fn same_scope(self, other: Self) -> bool {
        self.access.same_generation(other.access)
            && self.buffer.same_generation(other.buffer)
    }
}

/// Compact handle for one attached Representation generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationAttachmentHandle {
    attachment: RepresentationAttachmentIdentity,
    lifecycle: LifecycleIdentity,
    session: InteractionSessionHandle,
}

impl RepresentationAttachmentHandle {
    /// Constructs a handle bound to one Session generation.
    #[must_use]
    pub const fn new(
        attachment: RepresentationAttachmentIdentity,
        lifecycle: LifecycleIdentity,
        session: InteractionSessionHandle,
    ) -> Self {
        Self { attachment, lifecycle, session }
    }

    /// Returns the stable attachment identity.
    #[must_use]
    pub const fn attachment(self) -> RepresentationAttachmentIdentity {
        self.attachment
    }

    /// Returns the exact attachment lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the owning Session generation.
    #[must_use]
    pub const fn session(self) -> InteractionSessionHandle {
        self.session
    }

    /// Reports equality of attachment, lifecycle, and Session generation.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.attachment.value() == other.attachment.value()
            && self.lifecycle.same_generation(other.lifecycle)
            && self.session.same_generation(other.session)
    }
}

/// Compact identity and lifecycle of one ordered Update stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateStreamHandle {
    stream: RepresentationUpdateStreamIdentity,
    lifecycle: LifecycleIdentity,
}

impl RepresentationUpdateStreamHandle {
    /// Constructs a descriptor-relative stream handle.
    #[must_use]
    pub const fn new(
        stream: RepresentationUpdateStreamIdentity,
        lifecycle: LifecycleIdentity,
    ) -> Self {
        Self { stream, lifecycle }
    }

    /// Returns the stable stream identity.
    #[must_use]
    pub const fn stream(self) -> RepresentationUpdateStreamIdentity {
        self.stream
    }

    /// Returns the exact stream lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Reports equality of stream identity and lifecycle generation.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.stream.value() == other.stream.value()
            && self.lifecycle.same_generation(other.lifecycle)
    }
}

/// Exact attachment-qualified incarnation of one ordered Update stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepresentationUpdateStreamStamp {
    stream: RepresentationUpdateStreamIdentity,
    lifecycle: LifecycleIdentity,
    attachment: RepresentationAttachmentHandle,
}

impl RepresentationUpdateStreamStamp {
    /// Constructs a stream bound to one attachment generation.
    #[must_use]
    pub const fn new(
        stream: RepresentationUpdateStreamIdentity,
        lifecycle: LifecycleIdentity,
        attachment: RepresentationAttachmentHandle,
    ) -> Self {
        Self { stream, lifecycle, attachment }
    }

    /// Returns the stable stream identity.
    #[must_use]
    pub const fn stream(self) -> RepresentationUpdateStreamIdentity {
        self.stream
    }

    /// Returns the exact stream lifecycle generation.
    #[must_use]
    pub const fn lifecycle(self) -> LifecycleIdentity {
        self.lifecycle
    }

    /// Returns the owning attachment generation.
    #[must_use]
    pub const fn attachment(self) -> RepresentationAttachmentHandle {
        self.attachment
    }

    /// Returns the compact descriptor-relative stream handle.
    #[must_use]
    pub const fn handle(self) -> RepresentationUpdateStreamHandle {
        RepresentationUpdateStreamHandle::new(self.stream, self.lifecycle)
    }

    /// Reports equality of stream, lifecycle, and attachment generation.
    #[must_use]
    pub const fn same_generation(self, other: Self) -> bool {
        self.stream.value() == other.stream.value()
            && self.lifecycle.same_generation(other.lifecycle)
            && self.attachment.same_generation(other.attachment)
    }
}

/// Whether one Interaction has a separately governed deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionDeadline {
    /// No time-based deadline applies.
    None,
    /// The named outside authority observes and enforces the deadline.
    Governed(InteractionDeadlineIdentity),
}

/// Explicit relation to identities already carried by Semantic Intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionSemanticBinding {
    request: SemanticRequestIdentity,
    intent: SemanticIntentIdentity,
    cancellation: BufferCancellationIdentity,
    correlation: BufferCorrelationIdentity,
    deadline: BufferDeadline,
    transaction: BufferTransactionIdentity,
    history: BufferHistoryTransitionIdentity,
}

impl InteractionSemanticBinding {
    /// Constructs an explicit cross-scope identity relation.
    #[must_use]
    pub const fn new(
        request: SemanticRequestIdentity,
        intent: SemanticIntentIdentity,
        cancellation: BufferCancellationIdentity,
        correlation: BufferCorrelationIdentity,
        deadline: BufferDeadline,
        transaction: BufferTransactionIdentity,
        history: BufferHistoryTransitionIdentity,
    ) -> Self {
        Self {
            request,
            intent,
            cancellation,
            correlation,
            deadline,
            transaction,
            history,
        }
    }

    /// Returns the separately scoped semantic request identity.
    #[must_use]
    pub const fn request(self) -> SemanticRequestIdentity {
        self.request
    }

    /// Returns the immutable Semantic Intent identity.
    #[must_use]
    pub const fn intent(self) -> SemanticIntentIdentity {
        self.intent
    }

    /// Returns the Buffer-plan cancellation identity.
    #[must_use]
    pub const fn cancellation(self) -> BufferCancellationIdentity {
        self.cancellation
    }

    /// Returns the Buffer-plan correlation identity.
    #[must_use]
    pub const fn correlation(self) -> BufferCorrelationIdentity {
        self.correlation
    }

    /// Returns the Buffer-plan deadline relation.
    #[must_use]
    pub const fn deadline(self) -> BufferDeadline {
        self.deadline
    }

    /// Returns the future Transaction identity.
    #[must_use]
    pub const fn transaction(self) -> BufferTransactionIdentity {
        self.transaction
    }

    /// Returns the future History transition identity.
    #[must_use]
    pub const fn history(self) -> BufferHistoryTransitionIdentity {
        self.history
    }

    /// Reports equality with the complete Semantic Intent stamp.
    #[must_use]
    pub const fn matches_intent(self, stamp: &SemanticIntentStamp) -> bool {
        self.request.value() == stamp.request().value()
            && self.intent.value() == stamp.intent().value()
            && self.cancellation.value() == stamp.plan().cancellation().value()
            && self.correlation.value() == stamp.plan().correlation().value()
            && same_buffer_deadline(self.deadline, stamp.plan().deadline())
            && self.transaction.value() == stamp.plan().transaction().value()
            && self.history.value() == stamp.plan().history_transition().value()
    }
}

/// Typed incoherence in one Interaction request stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionStampError {
    /// The attachment belongs to another Session generation.
    AttachmentSessionMismatch,
}

impl fmt::Display for InteractionStampError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "Interaction attachment belongs to another Session generation",
        )
    }
}

impl core::error::Error for InteractionStampError {}

/// Complete outer Interaction request and explicit semantic binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionRequestStamp {
    attachment: RepresentationAttachmentHandle,
    request: InteractionRequestIdentity,
    cancellation: InteractionCancellationIdentity,
    correlation: InteractionCorrelationIdentity,
    deadline: InteractionDeadline,
    semantic: InteractionSemanticBinding,
}

impl InteractionRequestStamp {
    /// Constructs one coherent outer request stamp.
    ///
    /// # Errors
    ///
    /// Returns an attachment-to-Session mismatch.
    pub const fn new(
        session: InteractionSessionHandle,
        attachment: RepresentationAttachmentHandle,
        request: InteractionRequestIdentity,
        cancellation: InteractionCancellationIdentity,
        correlation: InteractionCorrelationIdentity,
        deadline: InteractionDeadline,
        semantic: InteractionSemanticBinding,
    ) -> Result<Self, InteractionStampError> {
        if !attachment.session().same_generation(session) {
            return Err(InteractionStampError::AttachmentSessionMismatch);
        }
        Ok(Self {
            attachment,
            request,
            cancellation,
            correlation,
            deadline,
            semantic,
        })
    }

    /// Returns the exact Session generation.
    #[must_use]
    pub const fn session(self) -> InteractionSessionHandle {
        self.attachment.session()
    }

    /// Returns the exact attachment generation.
    #[must_use]
    pub const fn attachment(self) -> RepresentationAttachmentHandle {
        self.attachment
    }

    /// Returns the Interaction request identity.
    #[must_use]
    pub const fn request(self) -> InteractionRequestIdentity {
        self.request
    }

    /// Returns the Interaction cancellation identity.
    #[must_use]
    pub const fn cancellation(self) -> InteractionCancellationIdentity {
        self.cancellation
    }

    /// Returns the root causal identity.
    #[must_use]
    pub const fn correlation(self) -> InteractionCorrelationIdentity {
        self.correlation
    }

    /// Returns the separately governed Interaction deadline.
    #[must_use]
    pub const fn deadline(self) -> InteractionDeadline {
        self.deadline
    }

    /// Returns the explicit mapping to semantic and Buffer identities.
    #[must_use]
    pub const fn semantic(self) -> InteractionSemanticBinding {
        self.semantic
    }

    /// Reports equality of every outer request identity and generation.
    #[must_use]
    pub const fn same_request(self, other: Self) -> bool {
        self.attachment.same_generation(other.attachment)
            && self.request.value() == other.request.value()
            && self.cancellation.value() == other.cancellation.value()
            && self.correlation.value() == other.correlation.value()
            && same_interaction_deadline(self.deadline, other.deadline)
            && same_semantic_binding(self.semantic, other.semantic)
    }
}

const fn same_semantic_binding(
    left: InteractionSemanticBinding,
    right: InteractionSemanticBinding,
) -> bool {
    left.request().value() == right.request().value()
        && left.intent().value() == right.intent().value()
        && left.cancellation().value() == right.cancellation().value()
        && left.correlation().value() == right.correlation().value()
        && same_buffer_deadline(left.deadline(), right.deadline())
        && left.transaction().value() == right.transaction().value()
        && left.history().value() == right.history().value()
}

const fn same_interaction_deadline(
    left: InteractionDeadline,
    right: InteractionDeadline,
) -> bool {
    match (left, right) {
        (InteractionDeadline::None, InteractionDeadline::None) => true,
        (
            InteractionDeadline::Governed(left),
            InteractionDeadline::Governed(right),
        ) => left.value() == right.value(),
        _ => false,
    }
}

const fn same_buffer_deadline(
    left: BufferDeadline,
    right: BufferDeadline,
) -> bool {
    match (left, right) {
        (BufferDeadline::None, BufferDeadline::None) => true,
        (BufferDeadline::Governed(left), BufferDeadline::Governed(right)) => {
            left.value() == right.value()
        }
        _ => false,
    }
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
