use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionOutcomeDeliveryStatus, InteractionOutcomeRequest,
    InteractionOutcomeUnavailable,
};
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootFailure, RootOpenRequest, RootOpenTarget,
};
use reovim_kernel_interface::lifecycle::{
    LifecycleState, ReadinessOutcome, ShutdownOutcome,
};
use reovim_resource_local::profile;
use reovim_server::assembly::life;

use super::support::{budget, constructed, opened};

#[test]
fn root_readiness_deadline_and_authorization_precede_endpoint_publication() {
    let mut service = constructed();
    let request = service.clients[0].attach_request(service.runtime.owner());
    assert_eq!(
        service.runtime.attach_client(request),
        Err(RootFailure::NotReady)
    );
    let mut expired = budget(&service);
    expired.deadline = DeadlineState::Reached;
    assert_eq!(service.runtime.start(expired), Err(RootFailure::Deadline));
    assert_eq!(service.runtime.state(), LifecycleState::Constructed);
    assert_eq!(
        service.runtime.start(budget(&service)),
        Ok(Some(ReadinessOutcome::Ready))
    );
    let mut forged = request;
    forged.authorization = life(999);
    assert_eq!(service.runtime.attach_client(forged), Err(RootFailure::Denied));
    let handle = service.runtime.attach_client(request).unwrap();
    let buffer = service.clients[0].session.scope().buffer();
    assert!(matches!(
        service.runtime.endpoint(handle, buffer),
        Err(RootFailure::Denied)
    ));
    service.close().unwrap();
}

#[test]
fn successful_open_replay_preserves_original_result_and_rejects_changed_target()
{
    let mut service = opened();
    let initial = service.clients[0].state().unwrap();
    let handle = service.clients[0].handle().unwrap();
    let request = RootOpenRequest {
        client: handle,
        request: InteractionRequestIdentity::new(1),
        target: service.open_target,
        work: service.runtime.limits().work,
    };
    service.edit(1, 10, b"b").unwrap();
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    let current = service.clients[0].state().unwrap();
    let replay = service.runtime.open(request).unwrap();
    let completion = service.runtime.take_open(replay).unwrap().unwrap();
    assert_eq!(completion.ticket, replay);
    assert_eq!(completion.outcome.unwrap().state, initial);
    assert_eq!(
        service.runtime.open(RootOpenRequest {
            target: RootOpenTarget::Existing(initial.buffer()),
            ..request
        }),
        Err(RootFailure::ConflictingRequest)
    );
    let next = service.edit(0, 11, b"c").unwrap().accepted_state().unwrap();
    assert_eq!(next.history_edge().parent(), current.history().node());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01c"[..]));
    service.close().unwrap();
}

#[test]
fn detach_pending_open_keeps_the_exact_failure_drain_without_publication() {
    let mut service = constructed();
    service.runtime.start(budget(&service)).unwrap();
    let handle = service
        .runtime
        .attach_client(
            service.clients[0].attach_request(service.runtime.owner()),
        )
        .unwrap();
    let request = RootOpenRequest {
        client: handle,
        request: InteractionRequestIdentity::new(1),
        target: service.open_target,
        work: profile::count(1),
    };
    let ticket = service.runtime.open(request).unwrap();
    assert_eq!(service.runtime.take_open(ticket), Ok(None));
    assert_eq!(
        service.runtime.detach_client(handle, budget(&service)),
        Ok(ShutdownOutcome::IncompleteShutdown)
    );
    service.progress().unwrap();
    let completion = service.runtime.take_open(ticket).unwrap().unwrap();
    assert_eq!(completion.ticket, ticket);
    assert_eq!(completion.outcome.unwrap_err().cause, RootFailure::Cancelled);
    let buffer = service.clients[0].session.scope().buffer();
    assert!(service.runtime.endpoint(handle, buffer).is_err());
    assert_eq!(
        service.runtime.detach_client(handle, budget(&service)),
        Ok(ShutdownOutcome::Closed)
    );
    service.close().unwrap();
}

#[test]
fn detach_retains_accepted_undelivered_outcome_and_leaves_peer_buffer_live() {
    let mut service = opened();
    let original = service.clients[0].intent(10, b"b").unwrap();
    let lookup = InteractionOutcomeRequest::new(
        &service.clients[0].session,
        original.stamp(),
    )
    .unwrap();
    assert!(
        service
            .endpoint(0)
            .unwrap()
            .submit_intent(original)
            .non_admission()
            .is_none()
    );
    service.progress().unwrap();
    let handle = service.clients[0].handle().unwrap();
    assert_eq!(
        service.runtime.detach_client(handle, budget(&service)),
        Ok(ShutdownOutcome::IncompleteShutdown)
    );
    let outcome =
        service.endpoint(0).unwrap().take_outcome(lookup).outcome().unwrap();
    let accepted = outcome.accepted_state().unwrap();
    assert_eq!(outcome.request(), lookup.request());
    assert_eq!(outcome.cause(), None);
    assert_eq!(
        service.endpoint(0).unwrap().take_outcome(lookup).status(),
        InteractionOutcomeDeliveryStatus::Unavailable(
            InteractionOutcomeUnavailable::AlreadyDelivered
        )
    );
    assert_eq!(
        service.runtime.detach_client(handle, budget(&service)),
        Ok(ShutdownOutcome::Closed)
    );
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].state(), Some(accepted.result()));
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01b"[..]));
    let next = service.edit(1, 11, b"c").unwrap();
    assert_ne!(next.cause(), Some(InteractionOutcomeCause::Closing));
    assert_eq!(
        next.accepted_state().unwrap().history_edge().parent(),
        accepted.result().history().node()
    );
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01c"[..]));
    service.close().unwrap();
}
