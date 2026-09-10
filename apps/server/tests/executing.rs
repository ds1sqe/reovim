use std::rc::Rc;

use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause, InteractionOutcomeStatus,
};
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootOpenRequest, RootOpenTarget,
};
use reovim_kernel_interface::lifecycle::{LifecycleIdentity, LifecycleState};
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, life};

#[test]
fn repeated_heading_edits_execute_and_both_clients_receive_actual_snapshots() {
    let owner = life(1);
    let mut service = assembly::heading(
        owner,
        b"# a\n\xff\0\xfeopaque".to_vec(),
        telemetry(owner),
    )
    .unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    assert_ne!(service.clients[0].handle(), service.clients[1].handle());
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01a"[..]));
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01a"[..]));

    for (writer, request, replacement) in
        [(0, 10, b'b'), (1, 11, b'c'), (0, 12, b'd'), (1, 13, b'e')]
    {
        let prior = service.clients[writer].state().unwrap();
        let outcome = service.edit(writer, request, &[replacement]).unwrap();
        let state = accepted(&outcome, &prior);
        // Acceptance does not silently install either independent Client view.
        assert_eq!(service.clients[0].state(), Some(prior));
        assert_eq!(service.clients[1].state(), Some(prior));
        service.receive(1).unwrap();
        assert_eq!(service.clients[0].state(), Some(prior));
        assert_eq!(service.clients[1].state(), Some(state));
        service.receive(0).unwrap();
        assert_eq!(service.clients[0].state(), Some(state));
        assert_eq!(
            service.clients[0].payload(),
            Some(&[b'H', 1, replacement][..])
        );
        assert_eq!(
            service.clients[1].payload(),
            Some(&[b'H', 1, replacement][..])
        );
    }
    service.close().unwrap();
    assert_eq!(service.runtime.state(), LifecycleState::Closed);
    service.close().unwrap();
}

#[test]
fn keyed_toggles_use_the_same_service_without_byte_addressed_client_state() {
    let owner = life(2);
    // Source record order is deliberately opposite to the snapshot ordering.
    let mut service =
        assembly::keyed(owner, vec![93, 1, 17, 0], telemetry(owner)).unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    for (writer, request, key, alpha, omega) in [
        (0, 10, 17, 1, 1),
        (1, 11, 93, 1, 0),
        (0, 12, 17, 0, 0),
        (1, 13, 93, 0, 1),
    ] {
        let prior = service.clients[writer].state().unwrap();
        let state =
            accepted(&service.edit(writer, request, &[key]).unwrap(), &prior);
        service.receive(0).unwrap();
        service.receive(1).unwrap();
        assert_eq!(service.clients[0].state(), Some(state));
        assert_eq!(service.clients[1].state(), Some(state));
        assert_eq!(
            service.clients[0].payload(),
            Some(&[b'K', 17, alpha, 93, omega][..])
        );
        assert_eq!(
            service.clients[1].payload(),
            Some(&[b'K', 17, alpha, 93, omega][..])
        );
    }
    service.close().unwrap();
}

#[test]
fn invalid_heading_edit_preserves_observation_and_next_valid_precondition() {
    let owner = life(3);
    let mut service =
        assembly::heading(owner, b"## a\n\xff".to_vec(), telemetry(owner))
            .unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    let prior = service.clients[0].state().unwrap();
    let refused = service.edit(0, 10, &[0xff]).unwrap();
    assert_eq!(refused.status(), InteractionOutcomeStatus::NotAccepted);
    assert_eq!(refused.cause(), Some(InteractionOutcomeCause::PolicyFailure));
    assert_eq!(service.clients[0].state(), Some(prior));
    assert_eq!(service.clients[1].state(), Some(prior));
    assert_eq!(service.clients[0].payload(), Some(&b"H\x02a"[..]));
    assert_eq!(service.clients[1].payload(), Some(&b"H\x02a"[..]));
    let state = accepted(&service.edit(1, 11, b"z").unwrap(), &prior);
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), Some(state));
    assert_eq!(service.clients[1].state(), Some(state));
    assert_eq!(service.clients[0].payload(), Some(&b"H\x02z"[..]));
    assert_eq!(service.clients[1].payload(), Some(&b"H\x02z"[..]));
    service.close().unwrap();
}

#[test]
fn stale_observer_cannot_edit_until_it_consumes_the_published_snapshot() {
    let owner = life(4);
    let mut service =
        assembly::keyed(owner, vec![93, 1], telemetry(owner)).unwrap();
    service.open(0).unwrap();
    service.open(1).unwrap();
    let initial = service.clients[0].state().unwrap();
    let current = accepted(&service.edit(0, 10, &[93]).unwrap(), &initial);
    service.receive(0).unwrap();
    let refused = service.edit(1, 11, &[93]).unwrap();
    assert_eq!(refused.cause(), Some(InteractionOutcomeCause::StaleState));
    assert_eq!(service.clients[1].state(), Some(initial));
    assert_eq!(service.clients[1].payload(), Some(&b"K\x5d\x01"[..]));
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].state(), Some(current));
    assert_eq!(service.clients[1].payload(), Some(&b"K\x5d\0"[..]));
    let missing = service.edit(1, 12, &[17]).unwrap();
    assert_eq!(missing.cause(), Some(InteractionOutcomeCause::PolicyFailure));
    let restored = accepted(&service.edit(1, 13, &[93]).unwrap(), &current);
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), Some(restored));
    assert_eq!(service.clients[1].state(), Some(restored));
    assert_eq!(service.clients[0].payload(), Some(&b"K\x5d\x01"[..]));
    assert_eq!(service.clients[1].payload(), Some(&b"K\x5d\x01"[..]));
    service.close().unwrap();
}

#[test]
fn initial_attachment_remains_coherent_when_another_client_edits_first() {
    let owner = life(5);
    let mut service =
        assembly::heading(owner, b"# a\n\xff".to_vec(), telemetry(owner))
            .unwrap();
    service.open(0).unwrap();
    let original = service.clients[0].state().unwrap();
    let handle = service
        .runtime
        .attach_client(service.clients[1].attach_request(owner))
        .unwrap();
    let ticket = service
        .runtime
        .open(RootOpenRequest {
            client: handle,
            request: InteractionRequestIdentity::new(1),
            target: RootOpenTarget::Existing(
                service.runtime.resolve(handle, "buffer").unwrap(),
            ),
            work: profile::count(1),
        })
        .unwrap();
    service.progress().unwrap();
    let opened =
        service.runtime.take_open(ticket).unwrap().unwrap().outcome.unwrap();
    let current = accepted(&service.edit(0, 10, b"b").unwrap(), &original);

    let client = &mut service.clients[1];
    client.opened(handle, opened).unwrap();
    let endpoint = service.runtime.endpoint(handle, original.buffer()).unwrap();
    client.initialize(endpoint).unwrap();
    assert_eq!(client.state(), Some(current));
    assert_eq!(client.payload(), Some(&b"H\x01b"[..]));
    service.receive(0).unwrap();
    assert_eq!(service.clients[0].state(), Some(current));
    service.close().unwrap();
}

fn accepted(
    outcome: &InteractionOutcome,
    prior: &BufferStateStamp,
) -> BufferStateStamp {
    assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
    let accepted = outcome.accepted_state().expect("actual Core acceptance");
    let current = accepted.result();
    assert_eq!(current.source(), prior.source());
    assert_eq!(current.buffer(), prior.buffer());
    assert_ne!(current.version(), prior.version());
    assert_eq!(accepted.history_edge().parent(), prior.history().node());
    assert_eq!(accepted.history_edge().child(), current.history().node());
    current
}

fn telemetry(owner: LifecycleIdentity) -> Rc<Telemetry> {
    let clock = Rc::new(DeterministicClock::new(
        ClockDescriptor::new(
            profile::schema(0x8001),
            life(800),
            owner,
            1,
            u64::MAX,
            0x8002,
        )
        .unwrap(),
    ));
    Rc::new(Telemetry::new(owner, life(801), clock, None, 64).unwrap())
}
