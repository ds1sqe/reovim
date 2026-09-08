use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationResynchronizationFailure,
    RepresentationResynchronizationReceipt,
    RepresentationResynchronizationRequest,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateCarrier,
    RepresentationUpdateIdentityStamp, RepresentationUpdateLineage,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::{Client, ClientError};

#[derive(Default)]
pub(super) struct Recovery {
    gap: Option<RepresentationUpdate>,
    receipt: Option<RepresentationResynchronizationReceipt>,
}

impl Client {
    /// Builds recovery only from the actual retained, qualified Gap record.
    ///
    /// # Errors
    /// Rejects missing Gap, unsupported recovery or an already-owned request.
    pub fn recovery_request(
        &self,
    ) -> Result<RepresentationResynchronizationRequest, ClientError> {
        if self.recovery.receipt.is_some() {
            return Err(ClientError::Pressure);
        }
        let gap = self.recovery.gap.as_ref().ok_or(ClientError::NotReady)?;
        RepresentationResynchronizationRequest::new(
            &self.session,
            &self.attachment,
            gap,
        )
        .map_err(|_| {
            ClientError::Recovery(
                RepresentationResynchronizationFailure::Unsupported,
            )
        })
    }

    /// Retains the exact admitted recovery identity without claiming success.
    ///
    /// # Errors
    /// Rejects a receipt unrelated to this client's actual Gap.
    pub fn recovery_admitted(
        &mut self,
        receipt: RepresentationResynchronizationReceipt,
    ) -> Result<(), ClientError> {
        if self.recovery_request()?.receipt() != receipt {
            return Err(ClientError::Binding);
        }
        self.clear_pairs();
        self.recovery.receipt = Some(receipt);
        Ok(())
    }

    /// Installs only the terminal snapshot of this exact admitted recovery.
    /// Selection stays separately qualified until explicit local revalidation.
    ///
    /// # Errors
    /// Rejects wrong receipt/schema/state or reports a typed terminal failure.
    pub fn install_recovery(
        &mut self,
        update: RepresentationUpdate,
    ) -> Result<RepresentationUpdateIdentityStamp, ClientError> {
        let receipt = self.recovery.receipt.ok_or(ClientError::NotReady)?;
        receipt.validate_result(&update).map_err(|_| ClientError::Binding)?;
        update
            .validate(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        let identity = update.stamp().identity();
        if let RepresentationUpdateLineage::ResynchronizationFailed {
            cause,
            ..
        } = update.lineage()
        {
            self.recovery = Recovery::default();
            self.cursor = identity.cursor().value();
            return Err(ClientError::Recovery(cause));
        }
        let RepresentationUpdateLineage::Resynchronization { result, .. } =
            update.lineage()
        else {
            return Err(ClientError::Binding);
        };
        let state = update
            .stamp()
            .resolve_target(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        if self.state.is_some_and(|prior| {
            state.version().value() < prior.version().value()
        }) || result.value() < self.projection.predecessor().value()
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
        self.payload =
            update.into_carrier().map(RepresentationUpdateCarrier::into_bytes);
        self.state = Some(state);
        self.projection = projection;
        self.attachment = attachment;
        self.cursor = identity.cursor().value();
        self.recovery = Recovery::default();
        self.clear_pairs();
        Ok(identity)
    }

    /// Reports lost correspondence independently of a kept local selection.
    #[must_use]
    pub const fn recovery_pending(&self) -> bool {
        self.recovery.receipt.is_some()
    }

    pub(crate) fn lose_correspondence(&mut self) {
        self.payload = None;
        self.clear_pairs();
    }

    pub(crate) fn abandon_update(
        &mut self,
        update: &RepresentationUpdate,
    ) -> Result<(), ClientError> {
        update
            .validate(&self.session, &self.attachment)
            .map_err(|_| ClientError::Binding)?;
        if update.stamp().cursor().value() <= self.cursor {
            return Err(ClientError::Stale);
        }
        self.lose_correspondence();
        self.cursor = update.stamp().cursor().value();
        Ok(())
    }

    pub(crate) fn record_gap(&mut self, gap: RepresentationUpdate) {
        self.retain_gap(gap);
    }

    pub(super) fn take_control(
        &mut self,
        update: &RepresentationUpdate,
    ) -> Result<bool, ClientError> {
        match update.lineage() {
            RepresentationUpdateLineage::FullSnapshot { .. } => Ok(false),
            RepresentationUpdateLineage::Gap { .. }
            | RepresentationUpdateLineage::Invalidation { .. } => {
                self.payload = None;
                self.clear_pairs();
                self.cursor = update.stamp().cursor().value();
                Ok(true)
            }
            _ => Err(ClientError::Binding),
        }
    }

    pub(super) fn retain_gap(&mut self, update: RepresentationUpdate) {
        self.recovery = Recovery::default();
        if matches!(update.lineage(), RepresentationUpdateLineage::Gap { .. }) {
            self.recovery.gap = Some(update);
        }
    }
}
