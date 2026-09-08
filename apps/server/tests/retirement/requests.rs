use reovim_kernel_interface::cancellation::CancellationCause;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionOutcomeRequest, InteractionReconcileRequest,
    InteractionReconcileStatus, InteractionReconcileUnavailable,
};
use reovim_kernel_interface::interaction::identity;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDeliveryStatus, BufferChangePort,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_resource_local::profile;

use super::support::opened;

#[test]
fn exact_edit_replay_rejects_changed_payload_without_another_transition() {
    let mut service = opened();
    let initial = service.clients[0].state().unwrap();
    let subscription = service
        .endpoint(0)
        .unwrap()
        .subscribe_changes(profile::count(1))
        .unwrap();
    let original = service.clients[0].intent(10, b"b").unwrap();
    let retry = service.clients[0].intent(10, b"b").unwrap();
    let altered = service.clients[0].intent(10, b"c").unwrap();
    let lookup = InteractionOutcomeRequest::new(
        &service.clients[0].session,
        original.stamp(),
    )
    .unwrap();
    let endpoint = service.endpoint(0).unwrap();
    assert!(endpoint.submit_intent(original).non_admission().is_none());
    let refused = endpoint.submit_intent(altered).into_non_admission().unwrap();
    assert_eq!(
        refused.outcome().cause(),
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    assert_eq!(refused.intent().semantic().bytes(), b"c");
    service.progress().unwrap();
    let accepted =
        service.endpoint(0).unwrap().take_outcome(lookup).outcome().unwrap();
    let current = accepted.accepted_state().unwrap().result();
    let first = service
        .endpoint(0)
        .unwrap()
        .take_change(&subscription, initial.version())
        .unwrap();
    assert_eq!(first.notification().unwrap().accepted().result(), current);
    assert!(
        service
            .endpoint(0)
            .unwrap()
            .submit_intent(retry)
            .non_admission()
            .is_none()
    );
    service.progress().unwrap();
    assert_eq!(
        service.endpoint(0).unwrap().take_outcome(lookup).outcome(),
        Some(accepted)
    );
    let altered = service.clients[0].intent(10, b"c").unwrap();
    assert_eq!(
        service
            .endpoint(0)
            .unwrap()
            .submit_intent(altered)
            .non_admission()
            .unwrap()
            .outcome()
            .cause(),
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    let invented = InteractionReconcileRequest::new(
        &service.clients[0].session,
        lookup.request(),
        identity::InteractionReconcileIdentity::new(999),
    )
    .unwrap();
    assert_eq!(
        service.endpoint(0).unwrap().reconcile_outcome(invented).status(),
        InteractionReconcileStatus::Unavailable(
            InteractionReconcileUnavailable::UnknownRequest
        )
    );
    assert_eq!(
        service
            .endpoint(0)
            .unwrap()
            .take_change(&subscription, current.version())
            .unwrap()
            .status(),
        BufferChangeDeliveryStatus::Pending
    );
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), Some(current));
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01b"[..]));
    service.close().unwrap();
}

#[test]
fn identical_pending_edit_replay_preserves_requested_cancellation() {
    let mut service = opened();
    let initial = service.clients[0].state().unwrap();
    let original = service.clients[0].intent(10, b"b").unwrap();
    let retry = service.clients[0].intent(10, b"b").unwrap();
    let lookup = InteractionOutcomeRequest::new(
        &service.clients[0].session,
        original.stamp(),
    )
    .unwrap();
    let cancel = InteractionCancellationRequest::new(
        &service.clients[0].session,
        original.stamp(),
        CancellationCause::ExplicitRequest,
    )
    .unwrap();
    let endpoint = service.endpoint(0).unwrap();
    assert!(endpoint.submit_intent(original).non_admission().is_none());
    assert!(matches!(
        endpoint.request_cancellation(cancel),
        InteractionCancellationResult::Requested(_)
    ));
    assert!(endpoint.submit_intent(retry).non_admission().is_none());
    service.progress().unwrap();
    let cancelled =
        service.endpoint(0).unwrap().take_outcome(lookup).outcome().unwrap();
    assert_eq!(cancelled.cause(), Some(InteractionOutcomeCause::Cancelled));
    assert_eq!(cancelled.request(), lookup.request());
    assert_eq!(service.clients[0].state(), Some(initial));
    // A later valid edit proves that the cancelled proposal never installed.
    let next = service.edit(1, 11, b"c").unwrap();
    let accepted = next.accepted_state().unwrap();
    assert_eq!(accepted.history_edge().parent(), initial.history().node());
    assert_eq!(
        accepted.result().version().value(),
        initial.version().value() + 1
    );
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01c"[..]));
    service.close().unwrap();
}
