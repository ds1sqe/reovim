use reovim_kernel_interface::diagnostic::{
    DiagnosticClosedCause, DiagnosticDropCause, DiagnosticOutput,
    DiagnosticOutputDescriptor, DiagnosticOutputRecord,
    DiagnosticSubmissionOutcome,
};
use reovim_kernel_interface::lifecycle::ShutdownOutcome;

use super::{DiagnosticQueue, Phase, QueueError, State};

impl DiagnosticOutput for DiagnosticQueue {
    fn descriptor(&self) -> &DiagnosticOutputDescriptor {
        &self.descriptor
    }

    fn submit(
        &self,
        record: DiagnosticOutputRecord,
    ) -> DiagnosticSubmissionOutcome {
        let Ok(mut state) = self.state.try_borrow_mut() else {
            return DiagnosticSubmissionOutcome::dropped(
                DiagnosticDropCause::SynchronizationPressure,
                record,
            );
        };
        if let Some(cause) = lifecycle_failure(self, &state, &record) {
            return DiagnosticSubmissionOutcome::closed(cause, record);
        }
        let charged = match admission(self, &state, &record) {
            Ok(charged) => charged,
            Err(cause) => {
                return DiagnosticSubmissionOutcome::dropped(cause, record);
            }
        };
        state.last = Some(record.sequence());
        state.bytes = charged;
        // Admission proved spare capacity reserved by the constructor.
        state.records.push_back(record);
        DiagnosticSubmissionOutcome::accepted()
    }
}

impl DiagnosticQueue {
    /// Transfers the oldest complete record to the outside I/O consumer.
    /// Draining remains available after Closing or destination failure.
    ///
    /// # Errors
    /// Returns local synchronization pressure or an accounting invariant error.
    pub fn take(&self) -> Result<Option<DiagnosticOutputRecord>, QueueError> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| QueueError::SynchronizationPressure)?;
        let Some(record) = state.records.front() else {
            return Ok(None);
        };
        let bytes = state
            .bytes
            .checked_sub(record.size().value())
            .ok_or(QueueError::Accounting)?;
        state.bytes = bytes;
        Ok(state.records.pop_front())
    }

    /// Fences admission without discarding accepted records or doing I/O.
    /// Incomplete ownership remains drainable through `take`; retry after
    /// draining reaches Closed. This does not claim external sink delivery.
    ///
    /// # Errors
    /// Returns local synchronization pressure without waiting or dropping work.
    pub fn close(&self) -> Result<ShutdownOutcome, QueueError> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| QueueError::SynchronizationPressure)?;
        if !state.records.is_empty() {
            state.phase = Phase::Closing;
            return Ok(ShutdownOutcome::IncompleteShutdown);
        }
        state.phase = Phase::Closed;
        Ok(ShutdownOutcome::Closed)
    }

    /// Changes destination availability for deterministic failure exercises.
    /// Accepted records remain owned; availability never reopens a generation.
    ///
    /// # Errors
    /// Returns synchronization pressure or a Closing/Closed generation.
    pub fn set_available(&self, available: bool) -> Result<(), QueueError> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| QueueError::SynchronizationPressure)?;
        if state.phase != Phase::Ready {
            return Err(QueueError::Closed);
        }
        state.available = available;
        Ok(())
    }
}

fn lifecycle_failure(
    queue: &DiagnosticQueue,
    state: &State,
    record: &DiagnosticOutputRecord,
) -> Option<DiagnosticClosedCause> {
    if record.output() != queue.descriptor.lifecycle() {
        return Some(DiagnosticClosedCause::Replaced);
    }
    match state.phase {
        Phase::Ready => None,
        Phase::Closing => Some(DiagnosticClosedCause::Closing),
        Phase::Closed => Some(DiagnosticClosedCause::Closed),
    }
}

fn admission(
    queue: &DiagnosticQueue,
    state: &State,
    record: &DiagnosticOutputRecord,
) -> Result<u64, DiagnosticDropCause> {
    if record.stream() != queue.descriptor.source_stream()
        || state.last.is_some_and(|last| !last.is_before(record.sequence()))
    {
        return Err(DiagnosticDropCause::InvalidRecord);
    }
    if !state.available {
        return Err(DiagnosticDropCause::DestinationUnavailable);
    }
    if record.size().value() > queue.maximum_record {
        return Err(DiagnosticDropCause::BoundExceeded);
    }
    if state.records.len() >= queue.capacity {
        return Err(DiagnosticDropCause::RecordCapacity);
    }
    let charged = state
        .bytes
        .checked_add(record.size().value())
        .ok_or(DiagnosticDropCause::ChargeOverflow)?;
    if charged > queue.maximum_bytes {
        return Err(DiagnosticDropCause::ByteCapacity);
    }
    Ok(charged)
}
