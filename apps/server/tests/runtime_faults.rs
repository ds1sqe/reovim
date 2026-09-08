use std::{cell::Cell, rc::Rc};

use reovim_kernel_interface::cancellation::{CancellationCause, DeadlineState};
use reovim_kernel_interface::event::editing::EditingStage;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationRequest, InteractionCancellationResult,
    InteractionOutcomeRequest,
};
use reovim_kernel_interface::interaction::descriptor;
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDeliveryStatus, BufferChangePort,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcome, InteractionOutcomeCause, InteractionOutcomeStatus,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootFailure, RootLifecycleRequest, RootOpenRequest,
    RootOpenTarget,
};
use reovim_kernel_interface::interaction::session::{
    InteractionClientStamp, InteractionSessionStamp,
};
use reovim_kernel_interface::interaction::update::RepresentationUpdateLineage;
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly;

#[path = "runtime_faults/harness.rs"]
mod harness;
#[path = "runtime_faults/providers.rs"]
mod providers;
#[path = "runtime_faults/retention.rs"]
mod retention;

use harness::build;
use providers::{DeferredOpen, SnapshotMode};

#[test]
fn forged_same_handle_role_cannot_change_live_authority() {
    let mode = Rc::new(Cell::new(SnapshotMode::Good));
    let mut service = build(core::convert::identity, mode, false);
    service.boot();
    let selected = service.client.session.clone();
    service.client.session = forged_role(&selected);
    let request = service.client.intent(1, b"b").unwrap();
    let refused =
        service.port().submit_intent(request).into_non_admission().unwrap();
    assert_eq!(
        refused.outcome().cause(),
        Some(InteractionOutcomeCause::Denied)
    );
    service.client.session = selected;
    let accepted = service.edit(1, b"b");
    assert_eq!(accepted.status(), InteractionOutcomeStatus::Accepted);
    let update = service.next_update();
    service.client.install(update).unwrap();
    assert_eq!(service.client.payload(), Some([b'H', 1, b'b'].as_slice()));
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn detached_client_cannot_reopen_through_a_retained_handle() {
    let mut service = assembly::heading(
        assembly::life(1),
        b"# a\n".to_vec(),
        harness::telemetry(),
    )
    .unwrap();
    service.open(0).unwrap();
    let handle = service
        .runtime
        .attach_client(
            service.clients[1].attach_request(service.runtime.owner()),
        )
        .unwrap();
    let lifecycle = RootLifecycleRequest {
        root: service.runtime.owner(),
        work: profile::count(64),
        deadline: DeadlineState::Pending,
    };
    assert_eq!(
        service.runtime.detach_client(handle, lifecycle).unwrap(),
        ShutdownOutcome::Closed
    );
    let request = RootOpenRequest {
        client: handle,
        request: InteractionRequestIdentity::new(2),
        target: RootOpenTarget::Existing(
            service.clients[0].state().unwrap().buffer(),
        ),
        work: profile::count(64),
    };
    assert_eq!(service.runtime.open(request), Err(RootFailure::Closed));
    assert_eq!(
        service.runtime.shutdown(lifecycle).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn snapshot_failure_or_schema_replacement_preserves_acceptance_and_frontier() {
    for failure in [SnapshotMode::Failure, SnapshotMode::OtherSchema] {
        invalidation_after_acceptance(failure, false);
    }
}

#[test]
fn empty_data_queue_pressure_invalidates_the_transferred_projection() {
    invalidation_after_acceptance(SnapshotMode::Good, true);
}

fn invalidation_after_acceptance(failure: SnapshotMode, pressure: bool) {
    let mode = Rc::new(Cell::new(SnapshotMode::Good));
    let mut service =
        build(core::convert::identity, Rc::clone(&mode), pressure);
    service.boot();
    let initial = service.client.state().unwrap();
    let projection = service.client.attachment.projection();
    let next_cursor = service.client.cursor() + 1;
    let subscription =
        service.port().subscribe_changes(profile::count(1)).unwrap();
    mode.set(failure);
    let outcome = service.edit(1, b"b");
    assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
    let notice =
        service.port().take_change(&subscription, initial.version()).unwrap();
    assert_eq!(
        notice.notification().unwrap().accepted(),
        outcome.accepted_state().unwrap()
    );
    let update = service.next_update();
    assert_eq!(update.stamp().cursor().value(), next_cursor);
    assert_eq!(
        update.lineage(),
        RepresentationUpdateLineage::Invalidation {
            invalidated: projection.predecessor(),
        }
    );
    assert_eq!(
        update
            .stamp()
            .resolve_projection(
                &service.client.session,
                &service.client.attachment
            )
            .unwrap(),
        projection
    );
    assert!(update.carrier().is_none());
    assert_failure_trace(&service.telemetry, &outcome);
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

fn assert_failure_trace(telemetry: &Telemetry, outcome: &InteractionOutcome) {
    let mut after = 0;
    let records: Vec<_> = std::iter::from_fn(|| {
        let record = telemetry.next_record(after)?;
        after = record.sequence().value();
        Some(record)
    })
    .filter(|record| {
        record.context().correlation
            == Some(outcome.request().semantic().correlation())
    })
    .collect();
    let committed = records
        .iter()
        .find(|record| record.stage() == EditingStage::Committed)
        .expect("actual accepted transaction diagnostic");
    let failed = records
        .iter()
        .find(|record| record.stage() == EditingStage::UpdateFailed)
        .expect("distinct semantic delivery failure diagnostic");
    assert!(committed.sequence().is_before(failed.sequence()));
    assert_eq!(failed.context().parent, Some(committed.sequence()));
    assert!(
        records.iter().all(|record| record.stage() != EditingStage::Rejected)
    );
}

#[test]
fn cancellation_reaches_the_real_pending_resource_and_drains_its_completion() {
    let seen = Rc::new(Cell::new(false));
    let mode = Rc::new(Cell::new(SnapshotMode::Good));
    let mut service = build(
        |resource| DeferredOpen::new(resource, Rc::clone(&seen)),
        mode,
        false,
    );
    let ticket = service.begin();
    service.runtime.progress(profile::count(1)).unwrap();
    assert!(service.runtime.take_open(ticket).unwrap().is_none());
    service
        .runtime
        .cancel_open(ticket, CancellationCause::ExplicitRequest)
        .unwrap();
    service.runtime.progress(profile::count(1)).unwrap();
    assert!(seen.get());
    let completion = service.runtime.take_open(ticket).unwrap().unwrap();
    assert_eq!(completion.outcome.unwrap_err().cause, RootFailure::Cancelled);
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn cancelled_admitted_edit_has_one_terminal_and_no_change() {
    let mut service = build(
        core::convert::identity,
        Rc::new(Cell::new(SnapshotMode::Good)),
        false,
    );
    service.boot();
    let state = service.client.state().unwrap();
    let subscription =
        service.port().subscribe_changes(profile::count(1)).unwrap();
    let request = service.client.intent(1, b"b").unwrap();
    let lookup = InteractionOutcomeRequest::new(
        &service.client.session,
        request.stamp(),
    )
    .unwrap();
    let cancel = InteractionCancellationRequest::new(
        &service.client.session,
        request.stamp(),
        CancellationCause::ExplicitRequest,
    )
    .unwrap();
    assert!(service.port().submit_intent(request).admitted_request().is_some());
    assert!(matches!(
        service.port().request_cancellation(cancel),
        InteractionCancellationResult::Requested(_)
    ));
    service.runtime.progress(profile::count(64)).unwrap();
    let outcome = service.port().take_outcome(lookup).outcome().unwrap();
    assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::Cancelled));
    assert_eq!(outcome.request(), lookup.request());
    assert!(outcome.accepted_state().is_none());
    let changed =
        service.port().take_change(&subscription, state.version()).unwrap();
    assert_eq!(changed.status(), BufferChangeDeliveryStatus::Pending);
    assert_eq!(
        service.runtime.shutdown(service.lifecycle()).unwrap(),
        ShutdownOutcome::Closed
    );
}

fn forged_role(
    selected: &descriptor::InteractionSessionDescriptor,
) -> descriptor::InteractionSessionDescriptor {
    let original = selected.stamp();
    let client = original.client();
    let client = InteractionClientStamp::new(
        client.client(),
        client.lifecycle(),
        profile::schema(0xdead),
    );
    let stamp = InteractionSessionStamp::new(
        original.session(),
        original.contract(),
        original.lifecycle(),
        client,
    );
    descriptor::InteractionSessionDescriptor::new(
        stamp,
        selected.scope(),
        selected.capabilities(),
        selected.bounds().clone(),
        selected.update_contract().clone(),
        selected.operations(),
    )
    .unwrap()
}
