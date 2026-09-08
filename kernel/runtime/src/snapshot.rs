use reovim_kernel_interface::cancellation::{
    CancellationState, DeadlineState, InstallationOutcome,
};
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::interaction::delivery::update as delivery;
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::interaction::identity::{
    RepresentationUpdateCursor, RepresentationUpdateIdentity,
};
use reovim_kernel_interface::interaction::notification;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::update::{
    RepresentationGapCause, RepresentationUpdate,
    RepresentationUpdateBindingOutcome, RepresentationUpdateLineage,
    RepresentationUpdatePayloadClaim, RepresentationUpdateStamp,
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
        let update = self.make_snapshot(index, None)?;
        self.sessions[index].note_ready(&update);
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
        session.update_context = *context;
        session.publish_change(notice);
        if session.invalidated {
            return;
        }
        let update = self.make_snapshot(index, None);
        let session = &mut self.sessions[index];
        let failure = match update {
            Ok(update) => session.retain_snapshot(update).err(),
            Err(cause) => {
                session.invalidate(cause);
                Some(cause)
            }
        };
        let mut context = *context;
        context.observer = Some(session.binding.session.stamp().handle());
        if let Some(failure) = failure {
            self.snapshot_failure(failure, &context);
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

    pub(crate) fn make_snapshot(
        &mut self,
        index: usize,
        recovery: Option<delivery::RepresentationResynchronizationReceipt>,
    ) -> Result<RepresentationUpdate, RuntimeError> {
        let buffer = self.buffer.as_ref().ok_or(RuntimeError::Provider)?;
        let lease = buffer.observe();
        let state = lease.view();
        let session = &mut self.sessions[index];
        let predecessor = if recovery.is_some() {
            session.transferred_projection
        } else {
            session.snapshot_predecessor()
        };
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
                &predecessor,
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
            .max(session.projection.predecessor().value())
            .checked_add(1)
            .ok_or(RuntimeError::Capacity)?;
        let cursor = recovery
            .map_or_else(
                || session.snapshot_cursor(),
                |receipt| {
                    receipt
                        .cursor()
                        .checked_next()
                        .map(RepresentationUpdateCursor::value)
                },
            )
            .ok_or(RuntimeError::Capacity)?;
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
        let lineage = recovery.map_or_else(
            || RepresentationUpdateLineage::FullSnapshot {
                supersedes: predecessor.predecessor(),
                result,
            },
            |receipt| RepresentationUpdateLineage::Resynchronization {
                result,
                resynchronization: receipt.resynchronization(),
            },
        );
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
    pub(crate) fn note_ready(&self, update: &RepresentationUpdate) {
        let _ = self.telemetry.capture_update(
            &self.binding.session,
            &self.binding.attachment,
            update,
        );
        let mut context = self.update_context;
        context.observer = Some(self.binding.session.stamp().handle());
        context.update = Some(update.stamp().identity());
        context.state = update
            .stamp()
            .resolve_target(&self.binding.session, &self.binding.attachment)
            .ok();
        let _ = self.telemetry.emit(EditingStage::UpdateReady, &context);
    }

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
            self.note_ready(&update);
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
            self.invalidate(RuntimeError::Capacity);
            return Err(RuntimeError::Capacity);
        }
        self.note_ready(&update);
        self.updates.push(Some(update));
        Ok(())
    }

    fn invalidate(&mut self, failure: RuntimeError) {
        // Loss affects this projection, not the accepted edit or other clients.
        if self.phase != Phase::Attached {
            self.initial = None;
            self.invalidated = true;
            return;
        }
        if self.binding.session.capabilities().resynchronization()
            == descriptor::InteractionCapabilitySupport::Supported
        {
            let cause = match failure {
                RuntimeError::Capacity => RepresentationGapCause::Pressure,
                _ => RepresentationGapCause::ProjectionUnavailable,
            };
            let _ = self.queue_gap(cause);
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
        if let Some(update) = &self.control {
            self.note_ready(update);
        }
        self.updates.clear();
        self.invalidated = true;
    }
}

pub fn stamp(
    session: &mut Session,
    cursor: u64,
    projection: &ProjectionStamp,
) -> Result<RepresentationUpdateStamp, RuntimeError> {
    let state = session.current.ok_or(RuntimeError::Provider)?;
    let next =
        session.next_update.checked_add(1).ok_or(RuntimeError::Capacity)?;
    let update = RepresentationUpdateStamp::new(
        &session.binding.session,
        &session.binding.attachment,
        RepresentationUpdateIdentity::new(session.next_update),
        RepresentationUpdateCursor::new(cursor),
        &state,
        projection,
        &session.binding.attachment.coordinate(),
    )
    .map_err(|_| RuntimeError::Provider)?;
    session.next_update = next;
    Ok(update)
}
