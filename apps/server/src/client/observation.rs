use super::{Client, ClientError};
use reovim_kernel_interface::interaction::descriptor::{
    RepresentationAttachmentDescriptor, RepresentationAttachmentSelection,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateCarrier,
    RepresentationUpdateIdentityStamp, RepresentationUpdateLineage,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

impl Client {
    /// Installs a received full snapshot only at its exact predecessor.
    ///
    /// # Errors
    /// Rejects stale/mismatched updates, or invalidates correspondence.
    pub fn install(
        &mut self,
        update: RepresentationUpdate,
    ) -> Result<RepresentationUpdateIdentityStamp, ClientError> {
        if self.change_pending() || self.update_pending() {
            return Err(ClientError::Pressure);
        }
        self.validate_incoming(&update)?;
        if self.take_control(&update)? {
            self.retain_gap(update);
            return Err(ClientError::Invalidated);
        }
        self.commit_update(update)
    }

    pub(super) fn commit_update(
        &mut self,
        update: RepresentationUpdate,
    ) -> Result<RepresentationUpdateIdentityStamp, ClientError> {
        self.install_borrowed(&update)?;
        let identity = update.stamp().identity();
        self.payload =
            update.into_carrier().map(RepresentationUpdateCarrier::into_bytes);
        Ok(identity)
    }

    pub(super) fn install_borrowed(
        &mut self,
        update: &RepresentationUpdate,
    ) -> Result<(), ClientError> {
        self.validate_incoming(update)?;
        let state = update
            .stamp()
            .resolve_target(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        let RepresentationUpdateLineage::FullSnapshot { supersedes, result } =
            update.lineage()
        else {
            return Err(ClientError::Binding);
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

    pub(super) fn validate_incoming(
        &self,
        update: &RepresentationUpdate,
    ) -> Result<(), ClientError> {
        update
            .validate(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        let next = self.cursor.checked_add(1).ok_or(ClientError::Identity)?;
        let cursor = update.stamp().cursor().value();
        let gap =
            matches!(update.lineage(), RepresentationUpdateLineage::Gap { .. });
        if cursor < next || (!gap && cursor != next) {
            return Err(ClientError::Stale);
        }
        let state = update
            .stamp()
            .resolve_target(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        if self.state.is_some_and(|prior| {
            state.version().value() < prior.version().value()
                || state.history().lifecycle() != prior.history().lifecycle()
        }) {
            return Err(ClientError::Stale);
        }
        match update.lineage() {
            RepresentationUpdateLineage::FullSnapshot {
                supersedes, ..
            } if supersedes == self.projection.predecessor() => Ok(()),
            RepresentationUpdateLineage::Gap { .. } => Ok(()),
            RepresentationUpdateLineage::Invalidation { invalidated }
                if invalidated == self.projection.predecessor() =>
            {
                Ok(())
            }
            _ => Err(ClientError::Stale),
        }
    }

    pub(super) fn at_projection(
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
