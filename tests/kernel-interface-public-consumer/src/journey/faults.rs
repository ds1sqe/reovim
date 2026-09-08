use reovim_kernel_interface::cancellation::CancellationCause;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionIntentAdmissionStatus, InteractionOutcomeRequest,
    InteractionReconcileRequest, InteractionReconcileStatus,
    InteractionReconcileUnavailable,
};
use reovim_kernel_interface::interaction::identity;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDeliveryStatus, BufferChangeError, BufferChangePort,
};
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeCause;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::InteractionRootPort;
use reovim_kernel_interface::lifecycle::ShutdownOutcome;

use super::{cases, session::next_stamp};
use crate::first_edit::native::{models::ByteModel, support as native};

/// Exercises actual request rejection and replay through the public endpoint.
///
/// # Panics
/// Panics if refused work changes data or an uncertain replay runs again.
pub fn exercise_request_faults() {
    exact_replay();
    cancelled_replay();
    invalid_meaning();
    detach_during_edit();
}

fn exact_replay() {
    let mut case = cases::harness(ByteModel::new(), b"# a\n\xff");
    let accepted = cases::edit(&mut case, b"b");
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let changed = session.intent_at(4000, b"c", &case.initial);
    let refused = session.submit_intent(changed);
    assert_eq!(
        refused.non_admission().unwrap().outcome().cause(),
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    assert_eq!(
        refused.non_admission().unwrap().intent().semantic().bytes(),
        b"c"
    );
    let stale = session.intent_at(4100, b"d", &case.initial);
    assert_eq!(
        session.submit_intent(stale).non_admission().unwrap().outcome().cause(),
        Some(InteractionOutcomeCause::StaleState)
    );
    let retry = session.intent_at(4000, b"b", &case.initial);
    let stamp = retry.stamp();
    assert_eq!(
        session.submit_intent(retry).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    assert!(session.pending.is_none());
    let lookup =
        InteractionOutcomeRequest::new(&session.fixture.session, stamp)
            .unwrap();
    assert_eq!(
        session.take_outcome(lookup).outcome().unwrap().accepted_state(),
        Some(accepted.accepted())
    );
    let invented = InteractionReconcileRequest::new(
        &session.fixture.session,
        stamp,
        identity::InteractionReconcileIdentity::new(9990),
    )
    .unwrap();
    assert_eq!(
        session.reconcile_outcome(invented).status(),
        InteractionReconcileStatus::Unavailable(
            InteractionReconcileUnavailable::UnknownRequest
        )
    );
    assert_eq!(session.current_stamp(), accepted.accepted().result());
    assert_eq!(
        case.shared.borrow().current.as_deref(),
        Some(b"# b\n\xff".as_slice())
    );
    cases::finish(&mut case);
}

fn cancelled_replay() {
    let mut case = cases::harness(ByteModel::new(), b"# a\n\xff");
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let intent = session.intent(4100, b"b");
    let stamp = intent.stamp();
    assert_eq!(
        session.submit_intent(intent).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    let cancel = InteractionCancellationRequest::new(
        &session.fixture.session,
        stamp,
        CancellationCause::ExplicitRequest,
    )
    .unwrap();
    assert!(matches!(
        session.request_cancellation(cancel),
        InteractionCancellationResult::Requested(_)
    ));
    let retry = session.intent(4100, b"b");
    assert_eq!(
        session.submit_intent(retry).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    assert_eq!(
        session.complete_edit(&next_stamp(&case.initial, 1, 7)),
        Err(InteractionOutcomeCause::Cancelled)
    );
    let lookup =
        InteractionOutcomeRequest::new(&session.fixture.session, stamp)
            .unwrap();
    assert_eq!(
        session.take_outcome(lookup).outcome().unwrap().cause(),
        Some(InteractionOutcomeCause::Cancelled)
    );
    assert_eq!(case.shared.borrow().stamp, case.initial);
    assert_eq!(
        case.shared.borrow().current.as_deref(),
        Some(b"# a\n\xff".as_slice())
    );
    cases::finish(&mut case);
}

fn invalid_meaning() {
    let mut case = cases::harness(ByteModel::new(), b"# a\n\xff");
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let intent = session.intent(4200, b"!");
    let stamp = intent.stamp();
    assert_eq!(
        session.submit_intent(intent).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    assert_eq!(
        session.complete_edit(&next_stamp(&case.initial, 1, 7)),
        Err(InteractionOutcomeCause::InvalidIntent)
    );
    let lookup =
        InteractionOutcomeRequest::new(&session.fixture.session, stamp)
            .unwrap();
    assert_eq!(
        session.take_outcome(lookup).outcome().unwrap().cause(),
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    assert_eq!(case.shared.borrow().stamp, case.initial);
    assert!(session.notice.is_none());
    assert!(session.update.is_none());
    cases::finish(&mut case);
}

fn detach_during_edit() {
    let mut case = cases::harness(ByteModel::new(), b"# a\n\xff");
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let intent = session.intent(4300, b"b");
    let stamp = intent.stamp();
    assert_eq!(
        session.submit_intent(intent).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    assert_eq!(
        case.root.detach_client(case.first, case.root.budget()).unwrap(),
        ShutdownOutcome::IncompleteShutdown
    );
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    assert_eq!(
        session.complete_edit(&next_stamp(&case.initial, 1, 7)),
        Err(InteractionOutcomeCause::Cancelled)
    );
    let lookup =
        InteractionOutcomeRequest::new(&session.fixture.session, stamp)
            .unwrap();
    assert_eq!(
        session.take_outcome(lookup).outcome().unwrap().cause(),
        Some(InteractionOutcomeCause::Cancelled)
    );
    assert_eq!(case.shared.borrow().stamp, case.initial);
    cases::finish(&mut case);
}

/// Exercises independent bounded notification delivery and fresh checkpoints.
///
/// # Panics
/// Panics if a slow observer blocks acceptance or a stale cursor resumes.
pub fn exercise_delivery_faults() {
    let mut case = cases::harness(ByteModel::new(), b"# a\n\xff");
    let first = case
        .root
        .endpoint(case.first, case.initial.buffer())
        .unwrap()
        .subscribe_changes(native::count(1))
        .unwrap();
    let second = case
        .root
        .endpoint(case.second, case.initial.buffer())
        .unwrap()
        .subscribe_changes(native::count(1))
        .unwrap();
    let notice = cases::edit(&mut case, b"b");
    let endpoint =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    endpoint.publish_change(notice.clone());
    assert_eq!(
        endpoint
            .take_change(&first, case.initial.version())
            .unwrap()
            .notification(),
        Some(&notice)
    );
    let endpoint =
        case.root.endpoint(case.second, case.initial.buffer()).unwrap();
    endpoint.publish_change(notice.clone());
    endpoint.publish_change(notice.clone());
    let gap = endpoint.take_change(&second, case.initial.version()).unwrap();
    assert_eq!(gap.status(), BufferChangeDeliveryStatus::Gap);
    assert_eq!(gap.checkpoint(), Some(notice.accepted().result()));
    let replacement = endpoint.resynchronize_changes(&second).unwrap();
    assert_eq!(
        endpoint.take_change(&second, case.initial.version()),
        Err(BufferChangeError::StaleSubscription)
    );
    let snapshot = cases::receive_snapshot(endpoint, 3, 4, 1);
    assert_eq!(
        snapshot
            .update
            .stamp()
            .resolve_target(&snapshot.session, &snapshot.attachment),
        Ok(replacement.checkpoint)
    );
    assert_eq!(
        endpoint
            .take_change(&replacement, replacement.checkpoint.version())
            .unwrap()
            .status(),
        BufferChangeDeliveryStatus::Pending
    );
    assert_eq!(case.shared.borrow().stamp, notice.accepted().result());
    cases::finish(&mut case);
}
