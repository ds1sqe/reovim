//! Public-only request/snapshot consumer for the local executable.

use alloc::boxed::Box;
use core::fmt;

use reovim_kernel_interface::buffer::{
    BufferStateStamp,
    plan::{
        BufferCancellationIdentity, BufferCorrelationIdentity, BufferDeadline,
        BufferHistoryTransitionIdentity, BufferPlanStamp,
        BufferTransactionIdentity,
    },
};
use reovim_kernel_interface::cancellation::{CancellationState, DeadlineState};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionNegotiationRequest, InteractionNegotiationStatus,
    RepresentationAttachmentRequest,
    RepresentationAttachmentRequestBindingOutcome,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
    RepresentationAttachmentSelection,
};
use reovim_kernel_interface::interaction::identity::{
    InteractionCancellationIdentity, InteractionCorrelationIdentity,
    InteractionRequestIdentity,
};
use reovim_kernel_interface::interaction::intent::{
    InteractionIntent, InteractionIntentBindingOutcome,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    RootAttachRequest, RootClientHandle, RootOpenedBuffer,
};
use reovim_kernel_interface::interaction::session::{
    InteractionDeadline, InteractionRequestStamp, InteractionSemanticBinding,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateCarrier,
    RepresentationUpdateIdentityStamp, RepresentationUpdateLineage,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::semantic::descriptor;
use reovim_kernel_interface::semantic::identity::{
    SemanticIntentIdentity, SemanticRequestIdentity,
};
use reovim_kernel_interface::semantic::intent::SemanticIntentBindingOutcome;
use reovim_kernel_interface::semantic::stamp::{
    ProjectionStamp, SemanticCallerStamp, SemanticIntentStamp,
};

/// Local consumer failures retain authoritative state at the service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// Initial negotiation or attachment did not publish readiness.
    NotReady,
    /// Returned identity or public descriptor does not match this consumer.
    Binding,
    /// A predecessor, state observation or coordinate is no longer usable.
    Stale,
    /// A control update requires a new authoritative semantic snapshot.
    Invalidated,
    /// Intent structure or resource capacity does not fit the selected profile.
    Intent,
    /// The bounded reference's request identity cannot advance.
    Identity,
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "reference client: {self:?}")
    }
}
impl core::error::Error for ClientError {}

/// Independent client observation; has no access to Core or native state.
pub struct Client {
    /// Selected public Session descriptor.
    pub session: InteractionSessionDescriptor,
    /// Current observed Representation binding, not a Kernel state handle.
    pub attachment: RepresentationAttachmentDescriptor,
    policy: descriptor::RepresentationPolicyDescriptor,
    authorization: LifecycleIdentity,
    plan_owner: LifecycleIdentity,
    handle: Option<RootClientHandle>,
    state: Option<BufferStateStamp>,
    projection: ProjectionStamp,
    payload: Option<Box<[u8]>>,
    cursor: u64,
}

impl Client {
    /// Constructs an inert public consumer from assembly-selected descriptors.
    #[must_use]
    pub fn new(
        binding: &reovim_kernel_runtime::ClientBinding,
        policy: descriptor::RepresentationPolicyDescriptor,
        plan_owner: LifecycleIdentity,
    ) -> Self {
        Self {
            session: binding.session.clone(),
            attachment: binding.attachment.clone(),
            projection: binding.attachment.projection(),
            policy,
            authorization: binding.authorization,
            plan_owner,
            handle: None,
            state: None,
            payload: None,
            cursor: 0,
        }
    }

    /// Requests only the authorization already issued to this client.
    #[must_use]
    pub const fn attach_request(
        &self,
        root: LifecycleIdentity,
    ) -> RootAttachRequest {
        RootAttachRequest {
            root,
            client: self.session.stamp().client(),
            authorization: self.authorization,
        }
    }

    /// Installs only a coherent opened Buffer identity; this is not a snapshot.
    ///
    /// # Errors
    /// Rejects another client lifetime or Resource/Buffer scope.
    pub fn opened(
        &mut self,
        handle: RootClientHandle,
        opened: RootOpenedBuffer,
    ) -> Result<(), ClientError> {
        if handle.client != self.session.stamp().client()
            || !self.session.scope().contains(opened.state)
        {
            return Err(ClientError::Binding);
        }
        self.handle = Some(handle);
        self.state = Some(opened.state);
        Ok(())
    }

    /// Negotiates and consumes the actual initial snapshot from the endpoint.
    ///
    /// # Errors
    /// Rejects incompatible readiness, attachment or initial state.
    pub fn initialize(
        &mut self,
        port: &mut impl InteractionSessionPort,
    ) -> Result<(), ClientError> {
        let request = InteractionNegotiationRequest::new(
            &self.session,
            self.session.stamp(),
            self.session.scope(),
            self.session.capabilities(),
        )
        .map_err(|_| ClientError::Binding)?;
        if port.negotiate(request).status()
            != InteractionNegotiationStatus::Negotiated
        {
            return Err(ClientError::NotReady);
        }
        let RepresentationAttachmentRequestBindingOutcome::Bound(request) =
            RepresentationAttachmentRequest::bind(
                &self.session,
                self.attachment.clone(),
            )
        else {
            return Err(ClientError::Binding);
        };
        let outcome = port.attach(request);
        let ready = outcome.ready_state().ok_or(ClientError::NotReady)?;
        self.install_borrowed(ready.initial())?;
        self.payload =
            ready.initial().carrier().map(|carrier| carrier.bytes().into());
        let ack = RepresentationUpdateAcknowledgement::new(
            &self.session,
            &self.attachment,
            ready.initial().stamp().identity(),
            RepresentationUpdateApplication::Installed,
        )
        .map_err(|_| ClientError::Binding)?;
        if port.acknowledge_update(ack)
            != RepresentationUpdateAcknowledgementResult::Accepted
        {
            return Err(ClientError::Binding);
        }
        Ok(())
    }

    /// Returns the latest exact observation used by subsequent requests.
    #[must_use]
    pub const fn state(&self) -> Option<BufferStateStamp> {
        self.state
    }

    /// Returns current interpreted bytes, never authoritative native storage.
    #[must_use]
    pub fn payload(&self) -> Option<&[u8]> {
        self.payload.as_deref()
    }

    /// Returns the last consumed stream position for ordered retrieval.
    #[must_use]
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Returns this client's live root attachment once opened.
    #[must_use]
    pub const fn handle(&self) -> Option<RootClientHandle> {
        self.handle
    }

    /// Creates a semantic request from current observation and caller identity.
    ///
    /// # Errors
    /// Rejects missing correspondence, malformed intent or bounded ID overflow.
    pub fn intent(
        &self,
        request: u128,
        payload: &[u8],
    ) -> Result<InteractionIntent, ClientError> {
        if self.payload.is_none() {
            return Err(ClientError::NotReady);
        }
        let target = self.state.ok_or(ClientError::NotReady)?;
        let caller = SemanticCallerStamp::new(
            self.session.stamp().lifecycle(),
            self.session.stamp().client().role(),
        );
        let local =
            u64::try_from(request).map_err(|_| ClientError::Identity)?;
        let namespace = u64::try_from(self.session.stamp().session().value())
            .map_err(|_| ClientError::Identity)?;
        let plan_id = (u128::from(namespace) << 64) | u128::from(local);
        let plan = BufferPlanStamp::new(
            self.plan_owner,
            BufferTransactionIdentity::new(plan_id),
            BufferHistoryTransitionIdentity::new(plan_id),
            BufferCancellationIdentity::new(plan_id),
            BufferCorrelationIdentity::new(plan_id),
            BufferDeadline::None,
        );
        let semantic = SemanticIntentStamp::new(
            SemanticIntentIdentity::new(plan_id),
            caller,
            SemanticRequestIdentity::new(request),
            target,
            plan,
            self.projection,
            self.attachment.coordinate(),
        );
        let prepared = semantic
            .prepare_intent(
                &self.policy,
                self.policy.intent().selected_stamp(),
                1,
                0,
                u64::try_from(payload.len())
                    .map_err(|_| ClientError::Intent)?,
            )
            .map_err(|_| ClientError::Intent)?;
        let SemanticIntentBindingOutcome::Bound(semantic) =
            prepared.bind(&self.policy, payload.into())
        else {
            return Err(ClientError::Intent);
        };
        let binding = InteractionSemanticBinding::new(
            semantic.stamp().request(),
            semantic.stamp().intent(),
            plan.cancellation(),
            plan.correlation(),
            plan.deadline(),
            plan.transaction(),
            plan.history_transition(),
        );
        let stamp = InteractionRequestStamp::new(
            self.session.stamp().handle(),
            self.attachment.handle(),
            InteractionRequestIdentity::new(request),
            InteractionCancellationIdentity::new(request),
            InteractionCorrelationIdentity::new(request),
            InteractionDeadline::None,
            binding,
        )
        .map_err(|_| ClientError::Binding)?;
        let charge = self
            .session
            .bounds()
            .requests()
            .prepare_intent(semantic.shape().bytes().value())
            .map_err(|_| ClientError::Intent)?;
        let InteractionIntentBindingOutcome::Bound(intent) =
            InteractionIntent::bind(
                &self.session,
                &self.attachment,
                stamp,
                charge,
                semantic,
                CancellationState::Open,
                DeadlineState::Pending,
            )
        else {
            return Err(ClientError::Intent);
        };
        Ok(intent)
    }

    /// Installs a received full snapshot only at its exact predecessor.
    ///
    /// # Errors
    /// Rejects stale/mismatched updates, or invalidates correspondence.
    pub fn install(
        &mut self,
        update: RepresentationUpdate,
    ) -> Result<RepresentationUpdateIdentityStamp, ClientError> {
        self.install_borrowed(&update)?;
        let identity = update.stamp().identity();
        self.payload =
            update.into_carrier().map(RepresentationUpdateCarrier::into_bytes);
        Ok(identity)
    }

    fn install_borrowed(
        &mut self,
        update: &RepresentationUpdate,
    ) -> Result<(), ClientError> {
        update
            .validate(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        let state = update
            .stamp()
            .resolve_target(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        let RepresentationUpdateLineage::FullSnapshot { supersedes, result } =
            update.lineage()
        else {
            self.payload = None;
            return Err(ClientError::Invalidated);
        };
        if supersedes != self.projection.predecessor()
            || self.state.is_some_and(|current| {
                state.version().value() < current.version().value()
            })
        {
            return Err(ClientError::Stale);
        }
        let projection = ProjectionStamp::new(
            self.projection.projection(),
            self.projection.lifecycle(),
            self.projection.representation(),
            result,
        );
        let attachment = self.at_projection(&projection)?;
        self.attachment = attachment;
        self.projection = projection;
        self.state = Some(state);
        self.cursor = update.stamp().cursor().value();
        Ok(())
    }

    fn at_projection(
        &self,
        projection: &ProjectionStamp,
    ) -> Result<RepresentationAttachmentDescriptor, ClientError> {
        let selected = RepresentationAttachmentSelection::new(
            self.attachment.handle(),
            self.attachment.scope(),
            self.attachment.representation(),
            *projection,
            self.attachment.coordinate(),
            self.attachment.stream(),
            self.attachment.initial_credit().value(),
        );
        RepresentationAttachmentDescriptor::new(
            &self.session,
            &self.policy,
            selected,
        )
        .map_err(|_| ClientError::Binding)
    }
}
