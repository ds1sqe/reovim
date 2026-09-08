use reovim_kernel_interface::cancellation::{
    CancellationCause, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::delivery::intent;
use reovim_kernel_interface::interaction::outcome::InteractionOutcomeStatus;
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{self, InteractionRootPort};
use reovim_kernel_interface::lifecycle::{RollbackOutcome, ShutdownOutcome};
use reovim_resource_local::profile;

#[path = "open_rollback/harness.rs"]
mod harness;
#[path = "open_rollback/providers.rs"]
mod providers;

use harness::fixture;
use providers::{Call, Fault};

#[test]
fn completion_gaps_keep_exact_resource_rollback_owned_until_drained() {
    for fault in [Fault::OpenGap, Fault::ObservationGap] {
        let mut fixture = fixture(fault);
        let ticket = fixture.open(0, fixture.target, 1);
        fixture.runtime.progress(profile::count(64)).unwrap();
        let failed = fixture
            .runtime
            .take_open(ticket)
            .unwrap()
            .unwrap()
            .outcome
            .unwrap_err();
        assert_eq!(failed.cause, root::RootFailure::ProviderFailure);
        assert_eq!(failed.rollback, RollbackOutcome::IncompleteShutdown);
        fixture.assert_unpublished(ticket);
        assert_eq!(
            fixture
                .runtime
                .cancel_open(ticket, CancellationCause::ExplicitRequest)
                .unwrap()
                .execution(),
            ExecutionOutcome::OwnedNonterminal
        );
        assert_eq!(
            fixture
                .runtime
                .detach_client(ticket.client, fixture.budget())
                .unwrap(),
            ShutdownOutcome::IncompleteShutdown
        );
        assert_eq!(
            fixture.runtime.shutdown(fixture.budget()).unwrap(),
            ShutdownOutcome::IncompleteShutdown
        );
        let expected = fixture.control.operation.get().unwrap();
        assert!(
            fixture.control.calls.borrow().contains(&Call::Cancelled(expected))
        );
        assert!(
            fixture
                .control
                .calls
                .borrow()
                .iter()
                .filter_map(Call::cancelled)
                .all(|operation| operation == expected)
        );
        fixture.control.release.set(true);
        assert_eq!(
            fixture.runtime.shutdown(fixture.budget()).unwrap(),
            ShutdownOutcome::Closed
        );
        assert!(
            fixture.control.calls.borrow().contains(&Call::Delivered(expected))
        );
        assert!(fixture.control.calls.borrow().contains(&Call::Closed));
        assert_eq!(
            fixture.runtime.shutdown(fixture.budget()).unwrap(),
            ShutdownOutcome::Closed
        );
    }
}

#[test]
fn terminal_open_failure_retains_unfinished_resource_release() {
    let mut fixture = fixture(Fault::OpenFailure);
    let ticket = fixture.open(0, fixture.target, 1);
    fixture.runtime.progress(profile::count(64)).unwrap();
    let failed = fixture
        .runtime
        .take_open(ticket)
        .unwrap()
        .unwrap()
        .outcome
        .unwrap_err();
    assert_eq!(failed.cause, root::RootFailure::Unavailable);
    assert_eq!(failed.rollback, RollbackOutcome::IncompleteShutdown);
    fixture.assert_unpublished(ticket);
    assert_eq!(
        fixture.runtime.shutdown(fixture.budget()).unwrap(),
        ShutdownOutcome::IncompleteShutdown
    );
    fixture.control.release.set(true);
    assert_eq!(
        fixture.runtime.shutdown(fixture.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn cancelled_pending_open_never_publishes_a_late_ready_completion() {
    let mut fixture = fixture(Fault::PendingOpen);
    let ticket = fixture.open(0, fixture.target, 1);
    fixture.runtime.progress(profile::count(64)).unwrap();
    assert!(fixture.runtime.take_open(ticket).unwrap().is_none());
    fixture
        .runtime
        .cancel_open(ticket, CancellationCause::ExplicitRequest)
        .unwrap();
    fixture.runtime.progress(profile::count(64)).unwrap();
    let failed = fixture
        .runtime
        .take_open(ticket)
        .unwrap()
        .unwrap()
        .outcome
        .unwrap_err();
    assert_eq!(failed.cause, root::RootFailure::Cancelled);
    assert_eq!(failed.rollback, RollbackOutcome::IncompleteShutdown);
    fixture.assert_unpublished(ticket);
    fixture.control.release.set(true);
    fixture.runtime.progress(profile::count(64)).unwrap();
    fixture.assert_unpublished(ticket);
    assert_eq!(
        fixture.runtime.shutdown(fixture.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn failed_native_initialization_never_leaves_a_buffer() {
    let mut fixture = fixture(Fault::None);
    fixture.control.native_fail.set(true);
    let ticket = fixture.open(0, fixture.target, 1);
    fixture.runtime.progress(profile::count(64)).unwrap();
    let failed = fixture
        .runtime
        .take_open(ticket)
        .unwrap()
        .unwrap()
        .outcome
        .unwrap_err();
    assert_eq!(failed.cause, root::RootFailure::ProviderFailure);
    assert_eq!(failed.rollback, RollbackOutcome::Reverted);
    fixture.assert_unpublished(ticket);
    assert_eq!(
        fixture.runtime.shutdown(fixture.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn callback_time_replacement_fences_publication_and_preserves_failed_replay() {
    let mut fixture = fixture(Fault::ReplaceAfterInitial);
    let ticket = fixture.open(0, fixture.target, 1);
    fixture.runtime.progress(profile::count(64)).unwrap();
    let failed = fixture.runtime.take_open(ticket).unwrap().unwrap();
    assert_eq!(
        failed.outcome,
        Err(root::RootOpenFailure {
            cause: root::RootFailure::Stale,
            rollback: RollbackOutcome::IncompleteShutdown,
        })
    );
    fixture.assert_unpublished(ticket);
    fixture.control.replaced.set(false);
    fixture.runtime.progress(profile::count(64)).unwrap();
    assert!(fixture.control.calls.borrow().contains(&Call::Closed));
    let replay = fixture.open(0, fixture.target, 1);
    assert_eq!(replay, ticket);
    assert_eq!(fixture.runtime.take_open(replay).unwrap(), Some(failed));
    fixture.assert_unpublished(ticket);
    assert_eq!(
        fixture.runtime.shutdown(fixture.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn failed_join_snapshot_does_not_close_or_discard_the_shared_buffer() {
    let mut fixture = fixture(Fault::None);
    let ticket = fixture.open(0, fixture.target, 1);
    fixture.runtime.progress(profile::count(64)).unwrap();
    let ready =
        fixture.runtime.take_open(ticket).unwrap().unwrap().outcome.unwrap();
    fixture.clients[0].opened(ticket.client, ready).unwrap();
    fixture.clients[0]
        .initialize(
            fixture
                .runtime
                .endpoint(ticket.client, ready.state.buffer())
                .unwrap(),
        )
        .unwrap();
    fixture.control.snapshot_fail.set(true);
    let joined = fixture.open(
        1,
        root::RootOpenTarget::Existing(ready.state.buffer()),
        2,
    );
    fixture.runtime.progress(profile::count(64)).unwrap();
    let failed = fixture
        .runtime
        .take_open(joined)
        .unwrap()
        .unwrap()
        .outcome
        .unwrap_err();
    assert_eq!(failed.cause, root::RootFailure::ProviderFailure);
    assert_eq!(failed.rollback, RollbackOutcome::Reverted);
    assert!(
        fixture.runtime.endpoint(joined.client, ready.state.buffer()).is_err()
    );
    assert!(
        !fixture
            .control
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, Call::Closing | Call::Closed))
    );
    fixture.control.snapshot_fail.set(false);
    let edit = fixture.clients[0].intent(3, b"b").unwrap();
    let outcome = intent::InteractionOutcomeRequest::new(
        &fixture.clients[0].session,
        edit.stamp(),
    )
    .unwrap();
    assert!(
        fixture
            .runtime
            .endpoint(ticket.client, ready.state.buffer())
            .unwrap()
            .submit_intent(edit)
            .admitted_request()
            .is_some()
    );
    fixture.runtime.progress(profile::count(64)).unwrap();
    let result = fixture
        .runtime
        .endpoint(ticket.client, ready.state.buffer())
        .unwrap()
        .take_outcome(outcome)
        .outcome()
        .unwrap();
    assert_eq!(result.status(), InteractionOutcomeStatus::Accepted);
    assert_eq!(
        result.accepted_state().unwrap().result().buffer(),
        ready.state.buffer()
    );
    assert_eq!(
        fixture.runtime.shutdown(fixture.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
}
