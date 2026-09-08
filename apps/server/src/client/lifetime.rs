use super::{Client, ClientError};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionNegotiationRequest, InteractionNegotiationStatus,
    RepresentationAttachmentRequest,
    RepresentationAttachmentRequestBindingOutcome,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    RootAttachRequest, RootClientHandle, RootOpenedBuffer,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;

impl Client {
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

    /// Negotiates the Session without requiring a semantic view attachment.
    ///
    /// # Errors
    /// Rejects incompatible Session scope, capabilities or readiness.
    pub fn negotiate(
        &self,
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
        self.negotiate(port)?;
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
}
