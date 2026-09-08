use std::rc::Rc;

use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::buffer::plan::{
    BufferCancellationIdentity, BufferDeadline, BufferDeadlineIdentity,
    BufferPlanStamp,
};
use reovim_kernel_interface::cancellation::{CancellationCause, DeadlineState};
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionCancellationResult, InteractionOutcomeDeliveryStatus,
    InteractionOutcomeRequest, InteractionOutcomeUnavailable,
};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionCloseOutcome, InteractionCloseRequest,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionOutcomeCause, InteractionOutcomeStatus,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootLifecycleRequest,
};
use reovim_kernel_interface::interaction::{
    history::{InteractionHistoryAction, InteractionHistoryRequest},
    history_delivery::InteractionHistoryPort,
};
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::{ByteModel, profile};
use reovim_server::assembly::{self, HeadingService, life};

#[test]
fn pending_and_terminal_history_replays_share_the_edit_namespace() {
    let mut service = opened();
    service.edit(0, 1, b"b").unwrap();
    service.receive(0).unwrap();
    let undo =
        service.clients[0].history(2, InteractionHistoryAction::Undo).unwrap();
    let changed = service.clients[0]
        .history(2, InteractionHistoryAction::Redo { child: None })
        .unwrap();
    assert!(
        service.endpoint(0).unwrap().submit_history(undo).rejection.is_none()
    );
    assert!(
        service.endpoint(0).unwrap().submit_history(undo).rejection.is_none()
    );
    let plan = undo.plan();
    let changed_plan = BufferPlanStamp::new(
        plan.owner(),
        plan.transaction(),
        plan.history_transition(),
        BufferCancellationIdentity::new(
            plan.cancellation().value().checked_add(1).unwrap(),
        ),
        plan.correlation(),
        plan.deadline(),
    );
    let changed_metadata = bind_plan(&service, &undo, changed_plan);
    assert_eq!(
        service.endpoint(0).unwrap().submit_history(changed_metadata).rejection,
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    assert_eq!(
        service.endpoint(0).unwrap().request_history_cancellation(
            changed_metadata,
            CancellationCause::ExplicitRequest
        ),
        InteractionCancellationResult::UnknownRequest
    );
    assert_eq!(
        service.endpoint(0).unwrap().submit_history(changed).rejection,
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    let conflicting = service.clients[0].intent(2, b"c").unwrap();
    let rejection = service
        .endpoint(0)
        .unwrap()
        .submit_intent(conflicting)
        .into_non_admission()
        .unwrap();
    assert_eq!(
        rejection.outcome().cause(),
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    service.progress().unwrap();
    let first = service
        .endpoint(0)
        .unwrap()
        .take_history_outcome(undo)
        .outcome()
        .unwrap();
    assert_eq!(first.status(), InteractionOutcomeStatus::Accepted);
    assert!(
        service.endpoint(0).unwrap().submit_history(undo).rejection.is_none()
    );
    service.progress().unwrap();
    let replay = service
        .endpoint(0)
        .unwrap()
        .take_history_outcome(undo)
        .outcome()
        .unwrap();
    assert_eq!(replay, first);
    service.receive(0).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01a"[..]));
    let changed_target =
        service.clients[0].history(2, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(
        service.endpoint(0).unwrap().submit_history(changed_target).rejection,
        Some(InteractionOutcomeCause::InvalidIntent)
    );
    let redo = service
        .history(0, 3, InteractionHistoryAction::Redo { child: None })
        .unwrap();
    assert_eq!(
        redo.accepted_state().unwrap().result().version().value(),
        first.accepted_state().unwrap().result().version().value() + 1
    );
    service.receive(0).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01b"[..]));
    service.close().unwrap();
}

#[test]
fn no_history_retains_its_exact_id_without_admitting_a_changed_kind() {
    let mut service = opened();
    let request =
        service.clients[0].history(1, InteractionHistoryAction::Undo).unwrap();
    let outcome =
        service.history(0, 1, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::NoHistory));
    let edit = service.edit(0, 1, b"b").unwrap();
    assert_eq!(edit.cause(), Some(InteractionOutcomeCause::InvalidIntent));
    assert!(
        service
            .endpoint(0)
            .unwrap()
            .submit_history(request)
            .rejection
            .is_none()
    );
    assert_eq!(
        service.endpoint(0).unwrap().take_history_outcome(request).outcome(),
        Some(outcome)
    );
    assert_eq!(service.clients[0].state(), Some(request.target()));
    let close =
        InteractionCloseRequest::new(&service.clients[0].session, 64).unwrap();
    assert_eq!(
        service.endpoint(0).unwrap().close(close),
        InteractionCloseOutcome::Closed
    );
    assert_eq!(
        service.endpoint(0).unwrap().request_history_cancellation(
            request,
            CancellationCause::ExplicitRequest
        ),
        InteractionCancellationResult::Closed
    );
    service.close().unwrap();
}

#[test]
fn history_cancellation_and_lookup_require_the_complete_request() {
    let mut service = opened();
    service.edit(0, 1, b"b").unwrap();
    service.receive(0).unwrap();
    let current = service.clients[0].state().unwrap();
    let undo =
        service.clients[0].history(2, InteractionHistoryAction::Undo).unwrap();
    let wrong = service.clients[0]
        .history(2, InteractionHistoryAction::Redo { child: None })
        .unwrap();
    assert!(
        service.endpoint(0).unwrap().submit_history(undo).rejection.is_none()
    );
    assert_eq!(
        service.endpoint(0).unwrap().request_history_cancellation(
            wrong,
            CancellationCause::ExplicitRequest
        ),
        InteractionCancellationResult::UnknownRequest
    );
    assert_eq!(
        service.endpoint(0).unwrap().take_history_outcome(wrong).status(),
        InteractionOutcomeDeliveryStatus::Unavailable(
            InteractionOutcomeUnavailable::UnknownRequest
        )
    );
    assert!(matches!(
        service.endpoint(0).unwrap().request_history_cancellation(
            undo,
            CancellationCause::ExplicitRequest
        ),
        InteractionCancellationResult::Requested(_)
    ));
    service.progress().unwrap();
    let outcome = service
        .endpoint(0)
        .unwrap()
        .take_history_outcome(undo)
        .outcome()
        .unwrap();
    assert_eq!(outcome.request(), undo);
    assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::Cancelled));
    let next = service.history(0, 3, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(
        next.accepted_state().unwrap().result().version().value(),
        current.version().value() + 1
    );
    service.receive(0).unwrap();
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01a"[..]));
    service.close().unwrap();
}

#[test]
fn close_fences_both_record_kinds_and_keeps_their_exact_drain_route() {
    let mut service = opened();
    let edit = service.clients[0].intent(1, b"b").unwrap();
    let lookup = InteractionOutcomeRequest::new(
        &service.clients[0].session,
        edit.stamp(),
    )
    .unwrap();
    let undo =
        service.clients[0].history(2, InteractionHistoryAction::Undo).unwrap();
    assert!(
        service
            .endpoint(0)
            .unwrap()
            .submit_intent(edit)
            .admitted_request()
            .is_some()
    );
    assert!(
        service.endpoint(0).unwrap().submit_history(undo).rejection.is_none()
    );
    let close = RootLifecycleRequest {
        root: service.runtime.owner(),
        work: profile::count(64),
        deadline: DeadlineState::Pending,
    };
    assert_eq!(
        service.runtime.shutdown(close).unwrap(),
        ShutdownOutcome::IncompleteShutdown
    );
    let edit =
        service.endpoint(0).unwrap().take_outcome(lookup).outcome().unwrap();
    let history = service
        .endpoint(0)
        .unwrap()
        .take_history_outcome(undo)
        .outcome()
        .unwrap();
    assert_eq!(edit.request(), lookup.request());
    assert_eq!(history.request(), undo);
    assert_eq!(edit.cause(), Some(InteractionOutcomeCause::Closing));
    assert_eq!(history.cause(), Some(InteractionOutcomeCause::Closing));
    assert_eq!(
        service.runtime.shutdown(close).unwrap(),
        ShutdownOutcome::Closed
    );
}

#[test]
fn history_and_edits_exhaust_the_same_named_terminal_reservation() {
    let mut service = opened();
    let limit = service.clients[0]
        .session
        .bounds()
        .requests()
        .retained_outcomes()
        .aggregate()
        .value();
    for request in 1..=limit {
        let outcome = service
            .history(0, u128::from(request), InteractionHistoryAction::Undo)
            .unwrap();
        assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::NoHistory));
    }
    let next = u128::from(limit) + 1;
    let edit = service.edit(0, next, b"b").unwrap();
    let history =
        service.history(0, next, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(edit.cause(), Some(InteractionOutcomeCause::Pressure));
    assert_eq!(history.cause(), Some(InteractionOutcomeCause::Pressure));
    service.close().unwrap();
}

fn opened() -> HeadingService {
    let owner = life(30);
    let clock = ClockDescriptor::new(
        profile::schema(90),
        life(31),
        owner,
        1,
        u64::MAX,
        91,
    )
    .unwrap();
    let clock = Rc::new(DeterministicClock::new(clock));
    let telemetry =
        Rc::new(Telemetry::new(owner, life(32), clock, None, 32).unwrap());
    let mut service =
        assembly::heading(owner, b"# a\n\xff".to_vec(), telemetry).unwrap();
    service.open(0).unwrap();
    service
}

#[test]
fn unselected_history_deadline_is_refused_without_consuming_the_request_id() {
    let mut service = opened();
    let original =
        service.clients[0].history(1, InteractionHistoryAction::Undo).unwrap();
    let plan = original.plan();
    let plan = BufferPlanStamp::new(
        plan.owner(),
        plan.transaction(),
        plan.history_transition(),
        plan.cancellation(),
        plan.correlation(),
        BufferDeadline::Governed(BufferDeadlineIdentity::new(73)),
    );
    let governed = bind_plan(&service, &original, plan);
    assert_eq!(
        service.endpoint(0).unwrap().submit_history(governed).rejection,
        Some(InteractionOutcomeCause::Unsupported)
    );
    let outcome =
        service.history(0, 1, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::NoHistory));
    service.close().unwrap();
}

fn bind_plan(
    service: &HeadingService,
    request: &InteractionHistoryRequest,
    plan: BufferPlanStamp,
) -> InteractionHistoryRequest {
    let client = &service.clients[0];
    let model = ByteModel::new(
        client.attachment.representation().domain().lifecycle(),
        request.plan().owner(),
    );
    InteractionHistoryRequest::new(
        &client.session,
        model.plan_descriptor(),
        request.request(),
        plan,
        request.target(),
        request.action(),
    )
    .unwrap()
}
