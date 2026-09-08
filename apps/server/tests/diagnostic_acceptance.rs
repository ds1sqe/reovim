use std::rc::Rc;

use reovim_host_reference::{DeterministicClock, DiagnosticQueue};
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeStatus;
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, HeadingService, life};

#[test]
fn real_edits_survive_diagnostic_ring_pressure_and_failed_output() {
    let (telemetry, sink) = instrumentation();
    let mut service = assembly::heading(
        life(50),
        b"# a\n\xff".to_vec(),
        Rc::clone(&telemetry),
    )
    .unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    telemetry.flush(); // Occupy the sole output slot with a real boot record.
    let initial = service.clients[0].state().unwrap();
    let baseline = telemetry.health();
    accepted_edit(&mut service, 10, b'b');
    let changed = service.clients[0].state().unwrap();
    assert_eq!(changed.version().value(), initial.version().value() + 1);
    telemetry.flush();
    assert!(telemetry.health().output_lost > baseline.output_lost);
    assert!(telemetry.health().evicted > baseline.evicted);
    assert_eq!(service.clients[0].state(), Some(changed));
    assert!(sink.take().unwrap().is_some());
    assert!(sink.take().unwrap().is_none());

    sink.set_available(false).unwrap();
    let baseline = telemetry.health().output_lost;
    accepted_edit(&mut service, 11, b'c');
    let later = service.clients[0].state().unwrap();
    assert_eq!(later.version().value(), changed.version().value() + 1);
    telemetry.flush();
    assert!(telemetry.health().output_lost > baseline);
    assert_eq!(service.clients[0].state(), Some(later));
    assert!(sink.take().unwrap().is_none());

    sink.set_available(true).unwrap();
    accepted_edit(&mut service, 12, b'd');
    let final_state = service.clients[0].state().unwrap();
    assert_eq!(final_state.version().value(), later.version().value() + 1);
    telemetry.flush();
    assert!(sink.take().unwrap().is_some());
    service.close().unwrap();
    telemetry.close();
    telemetry.flush();
    while sink.take().unwrap().is_some() {}
    assert_eq!(sink.close(), Ok(ShutdownOutcome::Closed));
}

fn accepted_edit(service: &mut HeadingService, request: u128, label: u8) {
    let before = service.clients[0].state().unwrap();
    let outcome = service.edit(0, request, &[label]).unwrap();
    assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
    let accepted = outcome.accepted_state().unwrap();
    assert_eq!(accepted.history_edge().parent(), before.history().node());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), Some(accepted.result()));
    assert_eq!(service.clients[1].state(), Some(accepted.result()));
    assert_eq!(service.clients[0].payload(), Some(&[b'H', 1, label][..]));
    assert_eq!(service.clients[1].payload(), service.clients[0].payload());
}

fn instrumentation() -> (Rc<Telemetry>, Rc<DiagnosticQueue>) {
    let owner = life(50);
    let stream = life(51);
    let clock = ClockDescriptor::new(
        profile::schema(80),
        life(52),
        owner,
        1,
        u64::MAX,
        81,
    )
    .unwrap();
    let sink = Rc::new(
        DiagnosticQueue::new(owner, life(53), stream, 1, 2048, 2048).unwrap(),
    );
    let telemetry = Rc::new(
        Telemetry::new(
            owner,
            stream,
            Rc::new(DeterministicClock::new(clock)),
            Some(sink.clone()),
            8,
        )
        .unwrap(),
    );
    (telemetry, sink)
}
