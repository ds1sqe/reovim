use reovim_kernel_interface::interaction::identity::{
    RepresentationResynchronizationIdentity, RepresentationUpdateCursor,
};
use reovim_kernel_interface::interaction::update::{
    RepresentationGapCause, RepresentationUpdate, RepresentationUpdateLineage,
    RepresentationUpdateStamp,
};
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::RecoveryGap;
use crate::{RuntimeError, Session};

impl Session {
    pub(crate) fn queue_gap(
        &mut self,
        cause: RepresentationGapCause,
    ) -> Result<(), RuntimeError> {
        let (cursor, projection) = self.gap_frontier()?;
        let token =
            RepresentationResynchronizationIdentity::new(self.next_recovery);
        let next =
            self.next_recovery.checked_add(1).ok_or(RuntimeError::Capacity)?;
        let stamp = crate::snapshot::stamp(self, cursor, &projection)?;
        let identity = stamp.identity();
        let state = self.current.ok_or(RuntimeError::Provider)?;
        let expected = projection.predecessor();
        let available = ProjectionGeneration::new(
            expected.value().max(self.projection.predecessor().value()),
        );
        let update = RepresentationUpdate::gap(
            &self.binding.session,
            &self.binding.attachment,
            stamp,
            expected,
            available,
            cause,
            token,
        )
        .map_err(|_| RuntimeError::Provider)?;
        self.updates.clear();
        self.note_ready(&update);
        self.control = Some(update);
        self.gap = Some(RecoveryGap {
            identity,
            state,
            projection,
            token,
            expected,
            available,
            cause,
            delivered: false,
            acknowledged: false,
        });
        self.next_recovery = next;
        self.invalidated = true;
        Ok(())
    }

    fn gap_frontier(&self) -> Result<(u64, ProjectionStamp), RuntimeError> {
        let Some(terminal) =
            self.recovery.as_ref().and_then(|work| work.completion.as_ref())
        else {
            return Ok((
                self.next_delivery_cursor.ok_or(RuntimeError::Capacity)?,
                self.transferred_projection,
            ));
        };
        let cursor = terminal
            .stamp()
            .cursor()
            .checked_next()
            .ok_or(RuntimeError::Capacity)?;
        let prior = terminal
            .stamp()
            .resolve_projection(&self.binding.session, &self.binding.attachment)
            .map_err(|_| RuntimeError::Provider)?;
        let projection = ProjectionStamp::new(
            prior.projection(),
            prior.lifecycle(),
            prior.representation(),
            terminal.lineage().result().unwrap_or_else(|| prior.predecessor()),
        );
        Ok((cursor.value(), projection))
    }

    pub(crate) fn replay_gap(&mut self, expected: RepresentationUpdateCursor) {
        if self.control.is_some() || self.recovery.is_some() {
            return;
        }
        let Some(gap) = &self.gap else {
            return;
        };
        if expected.value() > gap.identity.cursor().value() {
            return;
        }
        let Ok(stamp) = RepresentationUpdateStamp::new(
            &self.binding.session,
            &self.binding.attachment,
            gap.identity.update(),
            gap.identity.cursor(),
            &gap.state,
            &gap.projection,
            &self.binding.attachment.coordinate(),
        ) else {
            return;
        };
        self.control = RepresentationUpdate::gap(
            &self.binding.session,
            &self.binding.attachment,
            stamp,
            gap.expected,
            gap.available,
            gap.cause,
            gap.token,
        )
        .ok();
    }

    pub(crate) fn mark_gap_transferred(
        &mut self,
        update: &RepresentationUpdate,
    ) {
        let RepresentationUpdateLineage::Gap { resynchronization, .. } =
            update.lineage()
        else {
            return;
        };
        let Some(gap) = &mut self.gap else {
            return;
        };
        if gap.token == resynchronization
            && gap.identity == update.stamp().identity()
        {
            gap.delivered = true;
        }
    }
}
