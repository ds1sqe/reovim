use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::identity::{
    RepresentationUpdateCursor, RepresentationUpdateIdentity,
};
use reovim_kernel_interface::interaction::notification;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateBindingOutcome,
    RepresentationUpdateLineage, RepresentationUpdatePayloadClaim,
    RepresentationUpdateStamp,
};
use reovim_kernel_interface::resource::port::ResourceAccessPort;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::identity::ProjectionGeneration;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::{
    RepresentationSnapshotPort, RepresentationSnapshotRequest,
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use crate::session::Phase;
use crate::{Runtime, RuntimeError, Session};

impl<A, M, D, R, S> Runtime<A, M, D, R, S>
where
    A: ResourceAccessPort,
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    pub(crate) fn initial_snapshot(
        &mut self,
        index: usize,
    ) -> Result<(), RuntimeError> {
        let update = self.make_snapshot(index)?;
        self.sessions[index].initial = Some(update);
        Ok(())
    }

    pub(crate) fn publish(
        &mut self,
        notice: &notification::BufferChangeNotification,
        context: &EditingContext,
    ) {
        for index in 0..self.sessions.len() {
            self.publish_one(index, notice, context);
        }
    }

    fn publish_one(
        &mut self,
        index: usize,
        notice: &notification::BufferChangeNotification,
        context: &EditingContext,
    ) {
        let session = &mut self.sessions[index];
        if !session.ready() {
            return;
        }
        session.current = Some(notice.accepted().result());
        session.publish_change(notice);
        if session.invalidated {
            return;
        }
        let update = self.make_snapshot(index);
        let session = &mut self.sessions[index];
        let failure = match update {
            Ok(update) => session.retain_snapshot(update).err(),
            Err(cause) => {
                session.invalidate();
                Some(cause)
            }
        };
        let mut context = *context;
        context.session = Some(session.binding.session.stamp().handle());
        if let Some(failure) = failure {
            self.snapshot_failure(failure, &context);
        }
        let session = &self.sessions[index];
        if session.initial.is_some()
            || session.control.is_some()
            || !session.updates.is_empty()
        {
            let _ = self.telemetry.emit(EditingStage::UpdateReady, &context);
        }
    }

    fn snapshot_failure(
        &self,
        failure: RuntimeError,
        context: &EditingContext,
    ) {
        let mut context = *context;
        context.cause = Some(match failure {
            RuntimeError::Capacity => InteractionOutcomeCause::Pressure,
            RuntimeError::Provider | RuntimeError::Configuration => {
                InteractionOutcomeCause::ProviderContractViolation
            }
        });
        let _ = self.telemetry.emit(EditingStage::UpdateFailed, &context);
    }

    fn make_snapshot(
        &mut self,
        index: usize,
    ) -> Result<RepresentationUpdate, RuntimeError> {
        let buffer = self.buffer.as_ref().ok_or(RuntimeError::Provider)?;
        let lease = buffer.observe();
        let state = lease.view();
        let session = &mut self.sessions[index];
        let predecessor = session.snapshot_predecessor();
        let request = RepresentationSnapshotRequest::new(
            &self.snapshot_selection,
            &state,
            predecessor,
            crate::engine::charge(self.snapshot_selection.call())
                .map_err(|_| RuntimeError::Capacity)?,
            CancellationState::Open,
            DeadlineState::Pending,
        )
        .map_err(|_| RuntimeError::Provider)?;
        let snapshot = request
            .produce_with(&self.snapshot)
            .map_err(|_| RuntimeError::Provider)?;
        snapshot
            .check_admission(
                self.snapshot.descriptor(),
                &buffer.stamp(),
                &session.snapshot_predecessor(),
                InstallationOutcome::Current,
            )
            .map_err(|_| RuntimeError::Provider)?;
        let payload = snapshot.into_payload();
        let selected =
            session.binding.session.update_contract().selected_stamp();
        if payload.schema.type_identity() != selected.type_identity()
            || payload.schema.schema_version() != selected.schema_version()
        {
            return Err(RuntimeError::Provider);
        }
        let next = predecessor
            .predecessor()
            .value()
            .checked_add(1)
            .ok_or(RuntimeError::Capacity)?;
        let cursor = session.snapshot_cursor().ok_or(RuntimeError::Capacity)?;
        let result = ProjectionGeneration::new(next);
        let projection = ProjectionStamp::new(
            predecessor.projection(),
            predecessor.lifecycle(),
            predecessor.representation(),
            result,
        );
        let stamp = stamp(session, cursor, &predecessor)?;
        let claim = RepresentationUpdatePayloadClaim::new(
            session.binding.session.update_contract().selected_stamp(),
            payload.items.value(),
            payload.depth.value(),
            u64::try_from(payload.bytes.len())
                .map_err(|_| RuntimeError::Capacity)?,
        );
        let lineage = RepresentationUpdateLineage::FullSnapshot {
            supersedes: predecessor.predecessor(),
            result,
        };
        let preparation = session
            .binding
            .session
            .prepare_update(&session.binding.attachment, stamp, lineage, claim)
            .map_err(|_| RuntimeError::Provider)?;
        let RepresentationUpdateBindingOutcome::Bound(carrier) =
            preparation.bind(payload.bytes)
        else {
            return Err(RuntimeError::Provider);
        };
        let update = carrier.into_update();
        session.projection = projection;
        session.cursor = cursor;
        Ok(update)
    }
}

impl Session {
    const fn snapshot_predecessor(&self) -> ProjectionStamp {
        match self.phase {
            Phase::Attached => self.projection,
            _ => self.transferred_projection,
        }
    }

    const fn snapshot_cursor(&self) -> Option<u64> {
        match self.phase {
            Phase::Attached => self.cursor.checked_add(1),
            _ => self.next_delivery_cursor,
        }
    }

    fn retain_snapshot(
        &mut self,
        update: RepresentationUpdate,
    ) -> Result<(), RuntimeError> {
        if self.phase != Phase::Attached {
            self.initial = Some(update);
            return Ok(());
        }
        let bytes: u64 = self
            .updates
            .iter()
            .flatten()
            .filter_map(|value| {
                value.carrier().map(|carrier| carrier.shape().bytes().value())
            })
            .sum();
        let reserved = self
            .subscription
            .as_ref()
            .filter(|_| !self.subscription_closed)
            .and_then(|value| value.reserved_bytes().ok())
            .map_or(0, reovim_kernel_interface::bound::CheckedByteCount::value);
        let bytes = bytes.saturating_add(reserved);
        let count = self
            .subscription
            .as_ref()
            .filter(|_| !self.subscription_closed)
            .map_or(0, |value| value.capacity.value());
        let next_bytes =
            update.carrier().map_or(0, |value| value.shape().bytes().value());
        if self
            .binding
            .session
            .bounds()
            .updates()
            .admit_retained(
                self.updates.len() as u64 + count,
                bytes,
                next_bytes,
            )
            .is_err()
        {
            self.invalidate();
            return Err(RuntimeError::Capacity);
        }
        self.updates.push(Some(update));
        Ok(())
    }

    fn invalidate(&mut self) {
        // Loss affects this projection, not the accepted edit or other clients.
        if self.phase != Phase::Attached {
            self.initial = None;
            self.invalidated = true;
            return;
        }
        let Some(cursor) = self.next_delivery_cursor else {
            self.invalidated = true;
            return;
        };
        let projection = self.transferred_projection;
        let Ok(stamp) = stamp(self, cursor, &projection) else {
            self.invalidated = true;
            return;
        };
        self.control = RepresentationUpdate::invalidation(
            &self.binding.session,
            &self.binding.attachment,
            stamp,
            projection.predecessor(),
        )
        .ok();
        self.updates.clear();
        self.invalidated = true;
    }
}

fn stamp(
    session: &Session,
    cursor: u64,
    projection: &ProjectionStamp,
) -> Result<RepresentationUpdateStamp, RuntimeError> {
    let state = session.current.ok_or(RuntimeError::Provider)?;
    RepresentationUpdateStamp::new(
        &session.binding.session,
        &session.binding.attachment,
        RepresentationUpdateIdentity::new(u128::from(cursor)),
        RepresentationUpdateCursor::new(cursor),
        &state,
        projection,
        &session.binding.attachment.coordinate(),
    )
    .map_err(|_| RuntimeError::Provider)
}
