use reovim_kernel_interface::buffer::{
    BufferIdentity, BufferIncarnationStamp, BufferStateStamp, BufferVersion,
};
use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDelivery, BufferChangeDeliveryStatus, BufferChangeError,
    BufferChangeNotification, BufferChangePort, BufferChangeSubscription,
};
use reovim_resource_local::profile;
use reovim_server::assembly::life;

use super::support::opened;

#[test]
fn notification_construction_rejects_nonacceptance_and_changed_request() {
    let mut service = opened();
    let initial = service.clients[0].state().unwrap();
    let subscription = service
        .endpoint(0)
        .unwrap()
        .subscribe_changes(profile::count(1))
        .unwrap();
    let missing =
        service.history(0, 10, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(
        BufferChangeNotification::from_history(missing),
        Err(BufferChangeError::NotAccepted)
    );
    let invalid = service.clients[0].intent(11, &[0xff]).unwrap();
    let refused = service.edit(0, 11, &[0xff]).unwrap();
    assert_eq!(
        BufferChangeNotification::from_edit(refused, &invalid),
        Err(BufferChangeError::NotAccepted)
    );
    assert_eq!(
        service
            .endpoint(0)
            .unwrap()
            .take_change(&subscription, initial.version())
            .unwrap()
            .status(),
        BufferChangeDeliveryStatus::Pending
    );
    let wrong = service.clients[0].intent(12, b"b").unwrap();
    let actual = service.clients[0].intent(13, b"b").unwrap();
    let accepted = service.edit(0, 13, b"b").unwrap();
    assert_eq!(
        BufferChangeNotification::from_edit(accepted, &wrong),
        Err(BufferChangeError::RequestMismatch)
    );
    let notice =
        BufferChangeNotification::from_edit(accepted, &actual).unwrap();
    assert_eq!(notice.accepted(), accepted.accepted_state().unwrap());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01b"[..]));
    service.close().unwrap();
}

#[test]
fn notifications_require_exact_history_scope_and_live_subscription() {
    let mut service = opened();
    let initial = service.clients[0].state().unwrap();
    let subscription = service
        .endpoint(0)
        .unwrap()
        .subscribe_changes(profile::count(1))
        .unwrap();
    let intent = service.clients[0].intent(10, b"b").unwrap();
    let accepted = service.edit(0, 10, b"b").unwrap();
    let current = accepted.accepted_state().unwrap().result();
    let notice =
        BufferChangeNotification::from_edit(accepted, &intent).unwrap();
    // Same Buffer version is not the same initial History checkpoint.
    let mut wrong_history = subscription.clone();
    wrong_history.checkpoint = BufferStateStamp::new(
        initial.source(),
        initial.buffer(),
        initial.version(),
        current.history(),
    );
    assert_eq!(
        BufferChangeDelivery::changed(
            &wrong_history,
            initial.version(),
            notice,
        ),
        Err(BufferChangeError::CheckpointMismatch)
    );
    assert_eq!(
        service
            .endpoint(0)
            .unwrap()
            .take_change(&wrong_history, initial.version()),
        Err(BufferChangeError::StaleSubscription)
    );
    assert_eq!(
        service
            .endpoint(1)
            .unwrap()
            .take_change(&subscription, initial.version()),
        Err(BufferChangeError::StaleSubscription)
    );
    assert_foreign_scope(&service.clients[0].session, &subscription);
    let replacement = service
        .endpoint(0)
        .unwrap()
        .resynchronize_changes(&subscription)
        .unwrap();
    assert_eq!(replacement.checkpoint, current);
    assert_eq!(
        service
            .endpoint(0)
            .unwrap()
            .take_change(&subscription, initial.version()),
        Err(BufferChangeError::StaleSubscription)
    );
    assert_eq!(
        BufferChangeDelivery::gap(&replacement, initial.version(), current),
        Err(BufferChangeError::CheckpointMismatch)
    );
    assert_eq!(
        service.endpoint(0).unwrap().take_change(
            &replacement,
            BufferVersion::new(current.version().value() + 1)
        ),
        Err(BufferChangeError::CheckpointMismatch)
    );
    assert_eq!(
        service
            .endpoint(0)
            .unwrap()
            .take_change(&replacement, current.version())
            .unwrap()
            .status(),
        BufferChangeDeliveryStatus::Pending
    );
    let endpoint = service.endpoint(0).unwrap();
    endpoint.unsubscribe_changes(&replacement).unwrap();
    endpoint.unsubscribe_changes(&replacement).unwrap();
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), Some(current));
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01b"[..]));
    service.close().unwrap();
}

fn assert_foreign_scope(
    session: &reovim_kernel_interface::interaction::descriptor::
        InteractionSessionDescriptor,
    subscription: &BufferChangeSubscription,
) {
    let initial = subscription.checkpoint;
    let foreign = BufferStateStamp::new(
        initial.source(),
        BufferIncarnationStamp::new(BufferIdentity::new(999), life(999)),
        initial.version(),
        initial.history(),
    );
    assert_eq!(
        BufferChangeSubscription::new(
            session,
            subscription.lifecycle,
            foreign,
            subscription.capacity,
        ),
        Err(BufferChangeError::ScopeMismatch)
    );
    assert_eq!(
        BufferChangeDelivery::gap(subscription, initial.version(), foreign),
        Err(BufferChangeError::ScopeMismatch)
    );
}
