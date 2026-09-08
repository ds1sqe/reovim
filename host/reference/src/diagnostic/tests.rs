use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::diagnostic::{
    DiagnosticClosedCause, DiagnosticDropCause, DiagnosticOutput,
    DiagnosticOutputRecord, DiagnosticSubmissionOutcome,
    DiagnosticSubmissionStatus,
};
use reovim_kernel_interface::event::EventSequence;
use reovim_kernel_interface::event::editing::{
    EditingContext, EditingRecord, EditingStage,
};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, ShutdownOutcome,
};
use reovim_kernel_interface::operation::ContractSelection;

use super::DiagnosticQueue;

#[test]
fn pressure_returns_exact_record_and_close_keeps_drain_ownership() {
    let queue = queue(1, 4096);
    let first = record(&queue, 1);
    let second = record(&queue, 2);
    let expected = second.bytes().to_vec();
    assert_eq!(
        queue.submit(first).status(),
        DiagnosticSubmissionStatus::Accepted
    );
    let second =
        dropped(queue.submit(second), DiagnosticDropCause::RecordCapacity);
    assert_eq!(second.bytes(), expected);
    assert_eq!(queue.take().unwrap().unwrap().sequence(), sequence(1));
    assert_eq!(
        queue.submit(second).status(),
        DiagnosticSubmissionStatus::Accepted
    );
    assert_eq!(queue.close(), Ok(ShutdownOutcome::IncompleteShutdown));
    let third = record(&queue, 3);
    let rejected = queue.submit(third);
    assert!(matches!(
        rejected,
        DiagnosticSubmissionOutcome::Closed {
            cause: DiagnosticClosedCause::Closing,
            ..
        }
    ));
    assert_eq!(queue.take().unwrap().unwrap().bytes(), expected);
    assert_eq!(queue.close(), Ok(ShutdownOutcome::Closed));
    assert_eq!(queue.close(), Ok(ShutdownOutcome::Closed));
    assert!(matches!(
        queue.submit(record(&queue, 4)),
        DiagnosticSubmissionOutcome::Closed {
            cause: DiagnosticClosedCause::Closed,
            ..
        }
    ));
}

#[test]
fn failure_and_reentrant_pressure_do_not_consume_a_sequence() {
    let queue = queue(2, 4096);
    let first = record(&queue, 1);
    queue.set_available(false).unwrap();
    let first = dropped(
        queue.submit(first),
        DiagnosticDropCause::DestinationUnavailable,
    );
    queue.set_available(true).unwrap();
    let guard = queue.state.borrow_mut();
    let first = dropped(
        queue.submit(first),
        DiagnosticDropCause::SynchronizationPressure,
    );
    drop(guard);
    assert_eq!(
        queue.submit(first).status(),
        DiagnosticSubmissionStatus::Accepted
    );
    let delivered = queue.take().unwrap().unwrap();
    assert_eq!(delivered.sequence(), sequence(1));
    dropped(queue.submit(delivered), DiagnosticDropCause::InvalidRecord);
    assert_eq!(
        queue.submit(record(&queue, 2)).status(),
        DiagnosticSubmissionStatus::Accepted
    );
}

#[test]
fn byte_pressure_and_replaced_generations_preserve_records() {
    let template = queue(2, 4096);
    let first = record(&template, 1);
    let charge = first.size().value();
    let bounded = DiagnosticQueue::new(
        lifecycle(10),
        lifecycle(11),
        lifecycle(12),
        2,
        charge,
        charge,
    )
    .unwrap();
    assert_eq!(
        bounded.submit(first).status(),
        DiagnosticSubmissionStatus::Accepted
    );
    dropped(
        bounded.submit(record(&template, 2)),
        DiagnosticDropCause::ByteCapacity,
    );
    let other_output = DiagnosticQueue::new(
        lifecycle(10),
        lifecycle(111),
        lifecycle(12),
        2,
        2048,
        4096,
    )
    .unwrap();
    let changed = record(&other_output, 2);
    let bytes = changed.bytes().to_vec();
    let rejected = bounded.submit(changed);
    assert!(matches!(
        rejected,
        DiagnosticSubmissionOutcome::Closed {
            cause: DiagnosticClosedCause::Replaced,
            ..
        }
    ));
    assert_eq!(rejected.returned_record().unwrap().bytes(), bytes);
    let other_stream = DiagnosticQueue::new(
        lifecycle(10),
        lifecycle(11),
        lifecycle(112),
        2,
        2048,
        4096,
    )
    .unwrap();
    dropped(
        bounded.submit(record(&other_stream, 2)),
        DiagnosticDropCause::InvalidRecord,
    );
}

fn dropped(
    outcome: DiagnosticSubmissionOutcome,
    expected: DiagnosticDropCause,
) -> DiagnosticOutputRecord {
    let DiagnosticSubmissionOutcome::Dropped { cause, record } = outcome else {
        panic!("expected retained record with a typed drop cause")
    };
    assert_eq!(cause, expected);
    record
}

fn queue(capacity: u64, maximum: u64) -> DiagnosticQueue {
    DiagnosticQueue::new(
        lifecycle(10),
        lifecycle(11),
        lifecycle(12),
        capacity,
        2048,
        maximum,
    )
    .unwrap()
}

fn record(queue: &DiagnosticQueue, value: u64) -> DiagnosticOutputRecord {
    let clock = ClockDescriptor::new(
        ContractSelection::new(20, 1, 1, 1).unwrap(),
        lifecycle(21),
        lifecycle(22),
        1,
        100,
        23,
    )
    .unwrap();
    let record = EditingRecord::new(
        queue.descriptor().source_stream(),
        lifecycle(24),
        sequence(value),
        clock,
        clock.instant(1).unwrap(),
        EditingStage::Opened,
        EditingContext::default(),
    )
    .unwrap();
    DiagnosticOutputRecord::bind(
        queue.descriptor(),
        record.canonical(),
        record.canonical().bytes().into(),
    )
    .unwrap()
}

const fn lifecycle(identity: u128) -> LifecycleIdentity {
    LifecycleIdentity::new(identity, 1, Generation::new(1))
}

fn sequence(value: u64) -> EventSequence {
    EventSequence::new(value).unwrap()
}
