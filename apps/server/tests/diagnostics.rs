use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use reovim_host_reference::{DeterministicClock, DiagnosticQueue};
use reovim_kernel_interface::buffer::plan::BufferCorrelationIdentity;
use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferHistoryStamp, BufferIdentity,
    BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::clock::{
    ClockDescriptor, ClockError, MonotonicClock, MonotonicInstant,
};
use reovim_kernel_interface::diagnostic::{
    DiagnosticOutput, DiagnosticOutputDescriptor, DiagnosticOutputRecord,
    DiagnosticSubmissionOutcome,
};
use reovim_kernel_interface::event::editing::{
    EDITING_CANONICAL_MAX_BYTES, EDITING_FAMILY, EditingContext, EditingRecord,
    EditingRecordError, EditingStage,
};
use reovim_kernel_interface::event::{EVENT_RING_MAX_BYTES, EventSequence};
use reovim_kernel_interface::interaction::identity::{
    InteractionRequestIdentity, InteractionSessionIdentity,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::session::InteractionSessionHandle;
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, ShutdownOutcome,
};
use reovim_kernel_interface::operation::ContractSelection;
use reovim_kernel_interface::resource::{
    ResourceAccessStamp, ResourceIdentity, ResourceRevision,
    ResourceSnapshotIdentity, ResourceSnapshotStamp,
};
use reovim_kernel_runtime::diagnostics::{
    Emission, Telemetry, TelemetryError, TelemetryHealth,
};

#[test]
fn canonical_record_contains_exact_typed_evidence_not_native_payload() {
    let context = context();
    let clock = clock_descriptor(life(1), life(3));
    let record = EditingRecord::new(
        life(2),
        life(1),
        sequence(2),
        clock,
        clock.instant(42).unwrap(),
        EditingStage::Rejected,
        context,
    )
    .unwrap();
    let expected = concat!(
        "kernel.editing/v1 stream=2:1:1 seq=2 producer=1:1:1 clock=3:1:1 ",
        "ns=42 severity=info stage=rejected session=5@6:1:1 request=11 ",
        "correlation=12 parent=1 state=7@8:1:1/b/9:1:1:1/a/c@d:1:1/14/",
        "f@10:1:1 cause=StaleState\n",
    );
    assert_eq!(record.context(), &context);
    assert_eq!(record.stage(), EditingStage::Rejected);
    assert_eq!(record.sequence(), sequence(2));
    assert_eq!(record.canonical().bytes(), expected.as_bytes());
    assert_eq!(record.canonical().stream(), life(2));
    assert_eq!(record.canonical().family(), EDITING_FAMILY);
    assert_eq!(record.canonical().schema_version(), 1);
    assert!(
        record.canonical().size().value() <= EDITING_CANONICAL_MAX_BYTES as u64
    );
}

#[test]
fn invalid_causal_parent_and_wrong_clock_generation_are_rejected() {
    let clock = clock_descriptor(life(1), life(3));
    let context = EditingContext { parent: Some(sequence(2)), ..context() };
    let rejected = EditingRecord::new(
        life(2),
        life(1),
        sequence(2),
        clock,
        clock.instant(1).unwrap(),
        EditingStage::Committed,
        context,
    );
    assert!(matches!(rejected, Err(EditingRecordError::InvalidParent)));
    let replacement = clock_descriptor(
        life(1),
        LifecycleIdentity::new(3, 1, Generation::new(2)),
    );
    let rejected = EditingRecord::new(
        life(2),
        life(1),
        sequence(2),
        clock,
        replacement.instant(1).unwrap(),
        EditingStage::Committed,
        EditingContext::default(),
    );
    assert!(matches!(
        rejected,
        Err(EditingRecordError::Clock(ClockError::StaleGeneration))
    ));
}

#[test]
fn ring_eviction_exposes_sequence_gaps_without_truncating_records() {
    let telemetry = telemetry(2, None);
    let first = emit(&telemetry, EditingStage::Opened);
    let second = emit(&telemetry, EditingStage::Admitted);
    let third = emit(&telemetry, EditingStage::Committed);
    let retained = telemetry.next_record(0).unwrap();
    assert!(first.is_before(retained.sequence()));
    assert_eq!(retained.sequence(), second);
    assert_eq!(retained.stage(), EditingStage::Admitted);
    assert_eq!(
        telemetry.next_record(second.value()).unwrap().sequence(),
        third
    );
    assert!(telemetry.next_record(third.value()).is_none());
    assert_eq!(
        telemetry.health(),
        TelemetryHealth { evicted: 1, ..TelemetryHealth::default() }
    );
    // This is the actual charged memory ceiling, not a repository-size proxy.
    assert!(
        u64::try_from(telemetry.resident_bytes()).unwrap()
            <= EVENT_RING_MAX_BYTES
    );
    let clock =
        || Rc::new(DeterministicClock::new(clock_descriptor(life(1), life(3))));
    assert!(matches!(
        Telemetry::new(life(1), life(2), clock(), None, 0),
        Err(TelemetryError::Capacity)
    ));
    assert!(matches!(
        Telemetry::new(life(1), life(2), clock(), None, usize::MAX),
        Err(TelemetryError::Capacity)
    ));
}

#[test]
fn disabled_observation_and_closed_admission_remain_distinct() {
    let clock =
        Rc::new(DeterministicClock::new(clock_descriptor(life(1), life(3))));
    let telemetry =
        Telemetry::new(life(1), life(2), clock.clone(), None, 2).unwrap();
    telemetry.set_enabled(false);
    clock.close();
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Disabled
    );
    assert_eq!(telemetry.health(), TelemetryHealth::default());
    assert!(telemetry.next_record(0).is_none());
    telemetry.set_enabled(true);
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Unavailable(TelemetryError::Clock(ClockError::Closed))
    );
    telemetry.set_enabled(false);
    telemetry.close();
    telemetry.close();
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Unavailable(TelemetryError::Closed)
    );
    assert_eq!(telemetry.health().rejected, 2);
}

#[test]
fn bad_clock_and_grants_do_not_consume_stream_sequences() {
    let selected = clock_descriptor(life(1), life(3));
    let replacement = clock_descriptor(
        life(1),
        LifecycleIdentity::new(3, 1, Generation::new(2)),
    );
    let clock = Rc::new(FaultClock {
        selected: Cell::new(selected),
        next: RefCell::new(Ok(replacement.instant(1).unwrap())),
        replace_on_observation: Cell::new(None),
    });
    let telemetry =
        Telemetry::new(life(1), life(2), clock.clone(), None, 2).unwrap();
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Unavailable(TelemetryError::Record(
            EditingRecordError::Clock(ClockError::StaleGeneration)
        ))
    );
    *clock.next.borrow_mut() = Err(ClockError::NotReady);
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Unavailable(TelemetryError::Clock(ClockError::NotReady))
    );
    *clock.next.borrow_mut() = Ok(selected.instant(2).unwrap());
    assert_eq!(emit(&telemetry, EditingStage::Opened), EventSequence::first());
    assert_eq!(telemetry.health().rejected, 2);
    assert!(matches!(
        Telemetry::new(life(99), life(2), clock.clone(), None, 2),
        Err(TelemetryError::Grant)
    ));
    let wrong_stream = Rc::new(
        DiagnosticQueue::new(life(1), life(4), life(99), 2, 2048, 4096)
            .unwrap(),
    );
    assert!(matches!(
        Telemetry::new(life(1), life(2), clock, Some(wrong_stream), 2),
        Err(TelemetryError::Grant)
    ));
}

#[test]
fn same_owner_clock_replacement_requires_a_fresh_grant() {
    let selected = clock_descriptor(life(1), life(3));
    let replacement = clock_descriptor(
        life(1),
        LifecycleIdentity::new(3, 1, Generation::new(2)),
    );
    let clock = Rc::new(FaultClock {
        selected: Cell::new(selected),
        next: RefCell::new(Ok(selected.instant(1).unwrap())),
        replace_on_observation: Cell::new(None),
    });
    let telemetry =
        Telemetry::new(life(1), life(2), clock.clone(), None, 2).unwrap();
    clock.selected.set(replacement);
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Unavailable(TelemetryError::Clock(
            ClockError::DescriptorMismatch
        ))
    );
    clock.selected.set(selected);
    clock.replace_on_observation.set(Some(replacement));
    assert_eq!(
        telemetry.emit(EditingStage::Opened, &EditingContext::default()),
        Emission::Unavailable(TelemetryError::Clock(
            ClockError::DescriptorMismatch
        ))
    );
    clock.selected.set(selected);
    assert_eq!(emit(&telemetry, EditingStage::Opened), EventSequence::first());
    assert_eq!(telemetry.health().rejected, 2);
}

#[test]
fn sink_pressure_and_failure_do_not_change_admitted_diagnostics() {
    let sink = queue(1);
    let telemetry = telemetry(8, Some(sink.clone()));
    let first = emit(&telemetry, EditingStage::Opened);
    let second = emit(&telemetry, EditingStage::Admitted);
    assert!(sink.take().unwrap().is_none()); // Emission never invokes a sink.
    telemetry.flush();
    let delivered = sink.take().unwrap().unwrap();
    assert_eq!(delivered.sequence(), first);
    assert_eq!(
        delivered.bytes(),
        telemetry.next_record(0).unwrap().canonical().bytes()
    );
    assert_eq!(
        telemetry.next_record(first.value()).unwrap().sequence(),
        second
    );
    assert_eq!(telemetry.health().output_lost, 1);
    sink.set_available(false).unwrap();
    let third = emit(&telemetry, EditingStage::Prepared);
    telemetry.flush();
    assert_eq!(telemetry.health().output_lost, 2);
    sink.set_available(true).unwrap();
    let fourth = emit(&telemetry, EditingStage::Committed);
    telemetry.flush();
    assert_eq!(sink.take().unwrap().unwrap().sequence(), fourth);
    assert_eq!(
        telemetry.next_record(second.value()).unwrap().sequence(),
        third
    );
    assert_eq!(telemetry.health().rejected, 0);
    telemetry.flush();
    assert!(sink.take().unwrap().is_none());
    assert_eq!(telemetry.health().output_lost, 2);
}

#[test]
fn evicted_output_loss_is_reported_once_and_shutdown_remains_drainable() {
    let sink = queue(2);
    let telemetry = telemetry(1, Some(sink.clone()));
    emit(&telemetry, EditingStage::Opened);
    let retained = emit(&telemetry, EditingStage::Closing);
    telemetry.close();
    telemetry.flush();
    assert_eq!(telemetry.health().output_lost, 1);
    assert_eq!(sink.close(), Ok(ShutdownOutcome::IncompleteShutdown));
    assert_eq!(sink.take().unwrap().unwrap().sequence(), retained);
    assert_eq!(sink.close(), Ok(ShutdownOutcome::Closed));
    assert_eq!(telemetry.next_record(0).unwrap().sequence(), retained);
    telemetry.flush();
    assert_eq!(telemetry.health().output_lost, 1);
}

#[test]
fn reentrant_sink_cannot_extend_the_captured_flush_frontier() {
    let (telemetry, sink) = callback_fixture();
    let weak = Rc::downgrade(&telemetry);
    *sink.callback.borrow_mut() = Some(Box::new(move || {
        emit(&weak.upgrade().unwrap(), EditingStage::Admitted);
    }));
    let first = emit(&telemetry, EditingStage::Opened);
    telemetry.flush();
    assert_eq!(sink.queue.take().unwrap().unwrap().sequence(), first);
    assert!(sink.queue.take().unwrap().is_none());
    let next = telemetry.next_record(first.value()).unwrap().sequence();
    telemetry.flush();
    assert_eq!(sink.queue.take().unwrap().unwrap().sequence(), next);
    assert_eq!(telemetry.health(), TelemetryHealth::default());
}

#[test]
fn recursive_flush_never_resubmits_an_in_progress_record() {
    let (telemetry, sink) = callback_fixture();
    let weak = Rc::downgrade(&telemetry);
    *sink.callback.borrow_mut() = Some(Box::new(move || {
        weak.upgrade().unwrap().flush();
    }));
    let first = emit(&telemetry, EditingStage::Opened);
    telemetry.flush();
    assert_eq!(sink.queue.take().unwrap().unwrap().sequence(), first);
    assert!(sink.queue.take().unwrap().is_none());
    assert_eq!(telemetry.health(), TelemetryHealth::default());
}

struct FaultClock {
    selected: Cell<ClockDescriptor>,
    next: RefCell<Result<MonotonicInstant, ClockError>>,
    replace_on_observation: Cell<Option<ClockDescriptor>>,
}

impl MonotonicClock for FaultClock {
    fn descriptor(&self) -> ClockDescriptor {
        self.selected.get()
    }
    fn observe(&self) -> Result<MonotonicInstant, ClockError> {
        if let Some(replacement) = self.replace_on_observation.take() {
            self.selected.set(replacement);
        }
        *self.next.borrow()
    }
}

struct CallbackOutput {
    queue: DiagnosticQueue,
    callback: RefCell<Option<Box<dyn FnOnce()>>>,
}

impl DiagnosticOutput for CallbackOutput {
    fn descriptor(&self) -> &DiagnosticOutputDescriptor {
        self.queue.descriptor()
    }
    fn submit(
        &self,
        record: DiagnosticOutputRecord,
    ) -> DiagnosticSubmissionOutcome {
        let callback = self.callback.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
        self.queue.submit(record)
    }
}

fn callback_fixture() -> (Rc<Telemetry>, Rc<CallbackOutput>) {
    let sink = Rc::new(CallbackOutput {
        queue: DiagnosticQueue::new(life(1), life(4), life(2), 4, 2048, 8192)
            .unwrap(),
        callback: RefCell::new(None),
    });
    (Rc::new(telemetry(4, Some(sink.clone()))), sink)
}

fn telemetry(
    capacity: usize,
    sink: Option<Rc<dyn DiagnosticOutput>>,
) -> Telemetry {
    let clock =
        Rc::new(DeterministicClock::new(clock_descriptor(life(1), life(3))));
    Telemetry::new(life(1), life(2), clock, sink, capacity).unwrap()
}

fn queue(capacity: u64) -> Rc<DiagnosticQueue> {
    Rc::new(
        DiagnosticQueue::new(life(1), life(4), life(2), capacity, 2048, 8192)
            .unwrap(),
    )
}

fn emit(telemetry: &Telemetry, stage: EditingStage) -> EventSequence {
    telemetry.emit(stage, &EditingContext::default()).sequence().unwrap()
}

fn clock_descriptor(
    owner: LifecycleIdentity,
    clock: LifecycleIdentity,
) -> ClockDescriptor {
    ClockDescriptor::new(
        ContractSelection::new(1, 1, 1, 1).unwrap(),
        clock,
        owner,
        1,
        u64::MAX,
        2,
    )
    .unwrap()
}

fn context() -> EditingContext {
    let source = ResourceSnapshotStamp::new(
        ResourceAccessStamp::new(ResourceIdentity::new(7), life(8)),
        ResourceRevision::new(ContractSelection::new(9, 1, 1, 1).unwrap(), 10),
        ResourceSnapshotIdentity::new(11),
    );
    EditingContext {
        session: Some(InteractionSessionHandle::new(
            InteractionSessionIdentity::new(5),
            life(6),
        )),
        request: Some(InteractionRequestIdentity::new(17)),
        correlation: Some(BufferCorrelationIdentity::new(18)),
        state: Some(BufferStateStamp::new(
            source,
            BufferIncarnationStamp::new(BufferIdentity::new(12), life(13)),
            BufferVersion::new(14),
            BufferHistoryStamp::new(
                life(16),
                BufferHistoryNodeIdentity::new(15),
            ),
        )),
        parent: Some(sequence(1)),
        cause: Some(InteractionOutcomeCause::StaleState),
    }
}

const fn life(id: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(id, 1, Generation::new(1))
}

fn sequence(value: u64) -> EventSequence {
    EventSequence::new(value).unwrap()
}
