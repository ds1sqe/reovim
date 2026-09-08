use alloc::boxed::Box;

use reovim_kernel_interface::bound::CheckedByteCount;
use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::event::EventSequence;
use reovim_kernel_interface::event::editing::{EditingContext, EditingStage};
use reovim_kernel_interface::event::trace::{
    SemanticCapture, SemanticCaptureKey, SemanticCaptureStamp, TraceError,
    TraceLimits, TraceNode, TraceQueryBudget, TraceReadPort, TraceReport,
    TraceStart,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use reovim_kernel_interface::interaction::session::InteractionSessionHandle;
use reovim_kernel_interface::interaction::update::{
    RepresentationUpdate, RepresentationUpdateIdentityStamp,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::{Emission, Telemetry, TraceHealth, trace::TraceStore};

impl Telemetry {
    /// Selects bounded semantic capture before the service becomes Ready.
    /// The separate read grant must not be handed to ordinary observers.
    ///
    /// # Errors
    /// Rejects unusable reservations or replacing an already selected store.
    pub fn with_tracing(
        mut self,
        grant: LifecycleIdentity,
        limits: TraceLimits,
    ) -> Result<Self, TraceError> {
        if self.trace.is_some() {
            return Err(TraceError::Invalid);
        }
        self.trace =
            Some(Box::new(TraceStore::new(self.stream, grant, limits)?));
        Ok(self)
    }

    /// Captures the actual qualified semantic output outside editing authority.
    /// No native state is borrowed or copied, and failures do not reject edits.
    ///
    /// # Errors
    /// Returns disabled, incompatible, duplicate or bounded capture failure.
    pub fn capture_update(
        &self,
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        update: &RepresentationUpdate,
    ) -> Result<(), TraceError> {
        let result = self.capture(session, attachment, update);
        if result.is_err()
            && self.enabled.get()
            && let Some(trace) = &self.trace
        {
            trace.loss();
        }
        result
    }

    fn capture(
        &self,
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        update: &RepresentationUpdate,
    ) -> Result<(), TraceError> {
        if !self.enabled.get() || self.closed.get() {
            return Err(TraceError::NotCaptured);
        }
        let trace = self.trace.as_ref().ok_or(TraceError::NotCaptured)?;
        update
            .validate(session, attachment)
            .map_err(|_| TraceError::Invalid)?;
        let Some(carrier) = update.carrier() else {
            return Ok(());
        };
        let result = update.lineage().result().ok_or(TraceError::Invalid)?;
        let stamp = update.stamp();
        let state = stamp
            .resolve_target(session, attachment)
            .map_err(|_| TraceError::Invalid)?;
        let prior = stamp
            .resolve_projection(session, attachment)
            .map_err(|_| TraceError::Invalid)?;
        let projection = ProjectionStamp::new(
            prior.projection(),
            prior.lifecycle(),
            prior.representation(),
            result,
        );
        trace.snapshot(
            &SemanticCaptureStamp {
                update: stamp.identity(),
                state,
                projection,
                coordinate: attachment.coordinate(),
                schema: carrier.carrier(),
            },
            carrier.bytes(),
        )
    }

    /// Records an actual observer receipt, linked to its retained producer.
    /// This observation never consumes or acknowledges the update itself.
    ///
    /// # Errors
    /// Rejects a fabricated observer/update qualification or missing evidence.
    pub fn received_update(
        &self,
        session: &InteractionSessionDescriptor,
        attachment: &RepresentationAttachmentDescriptor,
        update: &RepresentationUpdate,
    ) -> Result<Emission, TraceError> {
        update
            .validate(session, attachment)
            .map_err(|_| TraceError::Invalid)?;
        let state = update
            .stamp()
            .resolve_target(session, attachment)
            .map_err(|_| TraceError::Invalid)?;
        self.client_event(
            session.stamp().handle(),
            update.stamp().identity(),
            &state,
            EditingStage::Received,
            EditingStage::UpdateReady,
        )
    }

    /// Records successful semantic installation, not drawing/presentation.
    ///
    /// # Errors
    /// Rejects absent receipt/join evidence, another observer, or mismatched
    /// installed state. An initial snapshot does not need a commit join.
    pub fn installed_update(
        &self,
        observer: InteractionSessionHandle,
        update: RepresentationUpdateIdentityStamp,
        state: &BufferStateStamp,
    ) -> Result<Emission, TraceError> {
        self.client_event(
            observer,
            update,
            state,
            EditingStage::Installed,
            EditingStage::Received,
        )
    }

    fn client_event(
        &self,
        observer: InteractionSessionHandle,
        update: RepresentationUpdateIdentityStamp,
        state: &BufferStateStamp,
        event: EditingStage,
        predecessor: EditingStage,
    ) -> Result<Emission, TraceError> {
        if !self.enabled.get() {
            return Ok(Emission::Disabled);
        }
        let trace = self.trace.as_ref().ok_or(TraceError::NotCaptured)?;
        let node = predecessor_node(trace, &update, predecessor)?;
        if node.context.observer != Some(observer)
            || node.context.state != Some(*state)
        {
            return Err(TraceError::Invalid);
        }
        let mut context: EditingContext = node.context;
        context.parent = Some(node.location.sequence);
        context.additional_parent = if event == EditingStage::Installed {
            // Explicit join of the receipt and its producing change, not time.
            installed_parent(trace, &update)?
        } else {
            None
        };
        Ok(self.emit(event, &context))
    }

    /// Reports separate graph/capture loss when detailed tracing was selected.
    #[must_use]
    pub fn trace_health(&self) -> Option<TraceHealth> {
        self.trace.as_deref().map(TraceStore::health)
    }

    /// Reports admitted side-store metadata and currently retained payload.
    #[must_use]
    pub fn trace_resident_bytes(&self) -> Option<usize> {
        self.trace.as_ref().and_then(|trace| trace.resident_bytes().ok())
    }
}

fn predecessor_node(
    trace: &TraceStore,
    update: &RepresentationUpdateIdentityStamp,
    stage: EditingStage,
) -> Result<TraceNode, TraceError> {
    let Some(node) = trace.update(update, stage) else {
        // Count the actual observation lost with its required evidence.
        trace.loss();
        return Err(TraceError::NotCaptured);
    };
    Ok(node)
}

fn installed_parent(
    trace: &TraceStore,
    update: &RepresentationUpdateIdentityStamp,
) -> Result<Option<EventSequence>, TraceError> {
    let ready = predecessor_node(trace, update, EditingStage::UpdateReady)?;
    if ready.context.session.is_some() && ready.context.parent.is_none() {
        trace.loss();
        return Err(TraceError::NotCaptured);
    }
    // An initial snapshot has no edit origin and needs no commit join.
    Ok(ready.context.parent)
}

impl TraceReadPort for Telemetry {
    fn backtrace(
        &self,
        grant: LifecycleIdentity,
        start: TraceStart,
        budget: TraceQueryBudget,
    ) -> Result<TraceReport, TraceError> {
        self.trace
            .as_ref()
            .ok_or(TraceError::NotCaptured)?
            .backtrace(grant, start, budget)
    }

    fn captured(
        &self,
        grant: LifecycleIdentity,
        key: &SemanticCaptureKey,
        bytes: CheckedByteCount,
    ) -> Result<SemanticCapture, TraceError> {
        self.trace
            .as_ref()
            .ok_or(TraceError::NotCaptured)?
            .captured(grant, key, bytes)
    }
}
