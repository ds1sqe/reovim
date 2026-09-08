use alloc::rc::Rc;
use core::cell::RefCell;

use reovim_kernel_interface::cancellation::{
    CancellationCause, DeadlineState, ExecutionOutcome,
};
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionIntentAdmissionStatus, InteractionOutcomeRequest,
};
use reovim_kernel_interface::interaction::identity::{
    ClientIdentity, InteractionRequestIdentity,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootAttachRequest, RootClientHandle, RootFailure,
    RootOpenFailure, RootOpenRequest, RootOpenTarget,
};
use reovim_kernel_interface::interaction::session::InteractionClientStamp;
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, LifecycleState, ReadinessOutcome,
    RollbackOutcome, ShutdownOutcome,
};
use reovim_kernel_interface::resource::access::ResourceOpenFailure;

use super::{Root, Shared};
use crate::first_edit::native::{models::ByteModel, support};

pub fn exercise_root_faults() {
    startup_and_authorization();
    startup_deadline_rolls_back();
    failed_open_rolls_back();
    cancelled_open_retains_incomplete_owner();
    detach_pending_open_fences_publication();
    open_replay_preserves_original_result();
    detach_keeps_terminal_drain_route();
}

fn startup_and_authorization() {
    let mut root = fixture();
    let client = client(1);
    let authorization = root.authorization_for(client);
    let request =
        RootAttachRequest { root: root.owner(), client, authorization };
    assert_eq!(root.attach_client(request), Err(RootFailure::NotReady));
    assert_eq!(root.start(root.budget()), Ok(None));
    assert_eq!(root.state(), LifecycleState::Starting);
    assert_eq!(root.attach_client(request), Err(RootFailure::NotReady));
    assert_eq!(root.start(root.budget()), Ok(Some(ReadinessOutcome::Ready)));
    let forged = RootAttachRequest {
        authorization: LifecycleIdentity::new(999, 1, Generation::new(1)),
        ..request
    };
    assert_eq!(root.attach_client(forged), Err(RootFailure::Denied));
    let attached = root.attach_client(request).unwrap();
    let buffer = root.shared.borrow().initial_stamp().buffer();
    assert!(matches!(
        root.endpoint(attached, buffer),
        Err(RootFailure::NotReady)
    ));
    assert!(root.shared.borrow().current.is_none());
}

fn failed_open_rolls_back() {
    let mut root = fixture();
    boot(&mut root);
    let client = attach(&mut root, 1);
    root.resource
        .fail_next(ResourceOpenFailure::Unavailable, RollbackOutcome::Reverted);
    let request = open_request(&root, client);
    let ticket = root.open(request).unwrap();
    assert_eq!(root.take_open(ticket).unwrap(), None);
    assert_eq!(
        root.take_open(ticket).unwrap().unwrap().outcome,
        Err(RootOpenFailure {
            cause: RootFailure::Unavailable,
            rollback: RollbackOutcome::Reverted
        })
    );
    let buffer = root.shared.borrow().initial_stamp().buffer();
    assert!(matches!(
        root.endpoint(client, buffer),
        Err(RootFailure::NotReady)
    ));
    assert!(root.shared.borrow().current.is_none());
    assert_eq!(root.shutdown(root.budget()), Ok(ShutdownOutcome::Closed));
}

fn startup_deadline_rolls_back() {
    let mut root = fixture();
    assert_eq!(root.start(root.budget()), Ok(None));
    let mut budget = root.budget();
    budget.deadline = DeadlineState::Reached;
    assert_eq!(
        root.start(budget),
        Ok(Some(ReadinessOutcome::ActivationFailed(RollbackOutcome::Reverted)))
    );
    assert_eq!(root.state(), LifecycleState::Closed);
    assert_eq!(root.start(root.budget()), Err(RootFailure::Closed));
    assert!(root.shared.borrow().current.is_none());
}

fn detach_pending_open_fences_publication() {
    let mut root = fixture();
    boot(&mut root);
    let client = attach(&mut root, 1);
    let ticket = root.open(open_request(&root, client)).unwrap();
    assert_eq!(
        root.detach_client(client, root.budget()),
        Ok(ShutdownOutcome::IncompleteShutdown)
    );
    assert_eq!(root.take_open(ticket), Ok(None));
    assert_eq!(
        root.take_open(ticket).unwrap().unwrap().outcome,
        Err(RootOpenFailure {
            cause: RootFailure::Cancelled,
            rollback: RollbackOutcome::Reverted,
        })
    );
    let buffer = root.shared.borrow().initial_stamp().buffer();
    assert!(root.shared.borrow().current.is_none());
    assert!(matches!(
        root.endpoint(client, buffer),
        Err(RootFailure::NotReady)
    ));
    assert_eq!(
        root.detach_client(client, root.budget()),
        Ok(ShutdownOutcome::Closed)
    );
    assert_eq!(root.shutdown(root.budget()), Ok(ShutdownOutcome::Closed));
}

fn cancelled_open_retains_incomplete_owner() {
    let mut root = fixture();
    boot(&mut root);
    let client = attach(&mut root, 1);
    root.resource.cancellation_rollback(RollbackOutcome::IncompleteShutdown);
    let ticket = root.open(open_request(&root, client)).unwrap();
    let cancellation =
        root.cancel_open(ticket, CancellationCause::ExplicitRequest).unwrap();
    assert_eq!(cancellation.execution(), ExecutionOutcome::OwnedNonterminal);
    assert_eq!(root.take_open(ticket).unwrap(), None);
    let result = root.take_open(ticket).unwrap().unwrap();
    assert_eq!(
        result.outcome,
        Err(RootOpenFailure {
            cause: RootFailure::Cancelled,
            rollback: RollbackOutcome::IncompleteShutdown
        })
    );
    let buffer = root.shared.borrow().initial_stamp().buffer();
    assert!(matches!(
        root.endpoint(client, buffer),
        Err(RootFailure::NotReady)
    ));
    assert!(root.shared.borrow().current.is_none());
    assert_eq!(
        root.shutdown(root.budget()),
        Ok(ShutdownOutcome::IncompleteShutdown)
    );
    assert!(root.resource.incomplete);
    root.resource.retire_rollback();
    assert_eq!(root.shutdown(root.budget()), Ok(ShutdownOutcome::Closed));
    assert_eq!(root.shutdown(root.budget()), Ok(ShutdownOutcome::Closed));
}

fn open_replay_preserves_original_result() {
    let mut root = fixture();
    boot(&mut root);
    let client = attach(&mut root, 1);
    let request = open_request(&root, client);
    let ticket = root.open(request).unwrap();
    assert_eq!(root.take_open(ticket).unwrap(), None);
    let result = root.take_open(ticket).unwrap().unwrap();
    let opened = result.outcome.unwrap();
    assert_eq!(root.open(request), Ok(ticket));
    assert_eq!(root.take_open(ticket), Ok(Some(result)));
    let conflict = RootOpenRequest {
        target: RootOpenTarget::Existing(opened.state.buffer()),
        ..request
    };
    assert_eq!(root.open(conflict), Err(RootFailure::ConflictingRequest));
    assert!(root.endpoint(client, opened.state.buffer()).is_ok());
    assert_eq!(
        root.shared.borrow().current.as_deref(),
        Some(b"# a\n\xff".as_slice())
    );
    assert_eq!(
        root.detach_client(client, root.budget()),
        Ok(ShutdownOutcome::Closed)
    );
    // Detach closes only the endpoint, never the shared Buffer.
    assert_eq!(
        root.shared.borrow().current.as_deref(),
        Some(b"# a\n\xff".as_slice())
    );
}

fn detach_keeps_terminal_drain_route() {
    let mut case =
        crate::journey::cases::harness(ByteModel::new(), b"# a\n\xff");
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let intent = session.intent(4000, b"b");
    let request = intent.stamp();
    assert_eq!(
        session.submit_intent(intent).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    let result = crate::journey::session::next_stamp(&case.initial, 1, 7);
    let changed = session.complete_edit(&result).unwrap();
    let lookup =
        InteractionOutcomeRequest::new(&session.fixture.session, request)
            .unwrap();
    assert_eq!(
        case.root.detach_client(case.first, case.root.budget()),
        Ok(ShutdownOutcome::IncompleteShutdown)
    );
    assert_eq!(
        case.root.shutdown(case.root.budget()),
        Ok(ShutdownOutcome::IncompleteShutdown)
    );
    assert!(!case.root.resource.is_closed());
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    assert_eq!(
        session.take_outcome(lookup).outcome().unwrap().accepted_state(),
        Some(changed.accepted())
    );
    assert_eq!(
        case.root.detach_client(case.first, case.root.budget()),
        Ok(ShutdownOutcome::Closed)
    );
    assert_eq!(
        case.shared.borrow().current.as_deref(),
        Some(b"# b\n\xff".as_slice())
    );
    assert_eq!(
        case.root.shutdown(case.root.budget()),
        Ok(ShutdownOutcome::Closed)
    );
    assert!(case.root.resource.is_closed());
    assert!(matches!(
        case.root.endpoint(case.first, case.initial.buffer()),
        Err(RootFailure::Closed)
    ));
}

fn fixture() -> Root<ByteModel> {
    let shared = Rc::new(RefCell::new(Shared::new(ByteModel::new())));
    let source = shared.borrow().initial_stamp().source();
    let observation = support::observation_at(source, b"# a\n\xff");
    Root::new(shared, observation)
}

fn boot(root: &mut Root<ByteModel>) {
    assert_eq!(root.start(root.budget()), Ok(None));
    assert_eq!(root.start(root.budget()), Ok(Some(ReadinessOutcome::Ready)));
}

fn client(identity: u128) -> InteractionClientStamp {
    let role = crate::interaction::support::fixture()
        .unwrap()
        .session
        .stamp()
        .client()
        .role();
    InteractionClientStamp::new(
        ClientIdentity::new(identity),
        LifecycleIdentity::new(8400 + identity, 8401, Generation::new(1)),
        role,
    )
}

fn attach(root: &mut Root<ByteModel>, identity: u128) -> RootClientHandle {
    let client = client(identity);
    let authorization = root.authorization_for(client);
    root.attach_client(RootAttachRequest {
        root: root.owner(),
        client,
        authorization,
    })
    .unwrap()
}

fn open_request(
    root: &Root<ByteModel>,
    client: RootClientHandle,
) -> RootOpenRequest {
    RootOpenRequest {
        client,
        request: InteractionRequestIdentity::new(8500),
        target: RootOpenTarget::Independent(root.resource.open_request(8501)),
        work: support::count(8),
    }
}
