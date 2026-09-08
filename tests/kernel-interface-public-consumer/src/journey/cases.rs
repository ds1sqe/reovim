use alloc::{collections::BTreeMap, rc::Rc};
use core::{cell::RefCell, fmt::Debug};

use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferStateStamp,
};
use reovim_kernel_interface::interaction::delivery::intent::{
    InteractionIntentAdmissionStatus, InteractionOutcomeRequest,
};
use reovim_kernel_interface::interaction::delivery::session::{
    InteractionNegotiationRequest, InteractionNegotiationStatus,
    RepresentationAttachmentRequest,
    RepresentationAttachmentRequestBindingOutcome,
};
use reovim_kernel_interface::interaction::delivery::update::{
    RepresentationUpdateAcknowledgement,
    RepresentationUpdateAcknowledgementResult, RepresentationUpdateApplication,
    RepresentationUpdateRequest,
};
use reovim_kernel_interface::interaction::descriptor::{
    InteractionSessionDescriptor, RepresentationAttachmentDescriptor,
};
use reovim_kernel_interface::interaction::identity::{
    ClientIdentity, InteractionRequestIdentity,
};
use reovim_kernel_interface::interaction::notification::{
    BufferChangeNotification, BufferChangePort,
};
use reovim_kernel_interface::interaction::port::InteractionSessionPort;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootAttachRequest, RootClientHandle, RootOpenRequest,
    RootOpenTarget, RootOpenTicket, RootOpenedBuffer,
};
use reovim_kernel_interface::interaction::session::InteractionClientStamp;
use reovim_kernel_interface::interaction::update::RepresentationUpdate;
use reovim_kernel_interface::interaction::{
    history::InteractionHistoryAction, history_delivery::InteractionHistoryPort,
};
use reovim_kernel_interface::lifecycle::{
    Generation, LifecycleIdentity, ReadinessOutcome, ShutdownOutcome,
};

use super::{
    root::Root,
    session::{Session, Shared},
};
use crate::first_edit::{
    Family,
    native::{
        models::{ByteModel, Key, KeyedModel},
        support as native,
    },
};

pub struct Harness<Model: Family> {
    pub root: Root<Model>,
    pub shared: Rc<RefCell<Shared<Model>>>,
    pub first: RootClientHandle,
    pub second: RootClientHandle,
    pub initial: BufferStateStamp,
}

pub struct ReceivedSnapshot {
    pub session: InteractionSessionDescriptor,
    pub attachment: RepresentationAttachmentDescriptor,
    pub update: RepresentationUpdate,
    pub previous_projection:
        reovim_kernel_interface::semantic::stamp::ProjectionStamp,
}

pub struct ObservationPair {
    pub owner: Harness<KeyedModel>,
    pub before: BufferStateStamp,
    pub notice: BufferChangeNotification,
    pub first: ReceivedSnapshot,
    pub second: ReceivedSnapshot,
}

pub fn exercise_heading_journey() {
    round_trip(
        ByteModel::new(),
        b"# a\n\xff",
        b"b",
        &b"# a\n\xff".to_vec(),
        &b"# b\n\xff".to_vec(),
        (b"H\x01a", b"H\x01b"),
    );
}

pub fn exercise_keyed_journey() {
    round_trip(
        KeyedModel::new(),
        &[0, 1],
        &[17],
        &BTreeMap::from([(Key::Alpha, false), (Key::Omega, true)]),
        &BTreeMap::from([(Key::Alpha, true), (Key::Omega, true)]),
        (&[b'K', 17, 0, 93, 1], &[b'K', 17, 1, 93, 1]),
    );
}

pub fn harness<Model: Family>(model: Model, source: &[u8]) -> Harness<Model>
where
    Model::State: Clone,
{
    let shared = Rc::new(RefCell::new(Shared::new(model)));
    let initial = shared.borrow().initial_stamp();
    let observation = native::observation_at(initial.source(), source);
    let mut root = Root::new(Rc::clone(&shared), observation);
    assert_eq!(root.start(root.budget()).unwrap(), None);
    assert_eq!(
        root.start(root.budget()).unwrap(),
        Some(ReadinessOutcome::Ready)
    );
    let first = attach_client(&mut root, 1200);
    let second = attach_client(&mut root, 2200);
    let ticket = root
        .open(RootOpenRequest {
            client: first,
            request: InteractionRequestIdentity::new(3000),
            target: RootOpenTarget::Independent(
                root.resource.open_request(3000),
            ),
            work: native::count(8),
        })
        .unwrap();
    let opened = settle_open(&mut root, ticket);
    assert_eq!(opened.state, initial);
    let ticket = root
        .open(RootOpenRequest {
            client: second,
            request: InteractionRequestIdentity::new(3001),
            target: RootOpenTarget::Existing(initial.buffer()),
            work: native::count(8),
        })
        .unwrap();
    assert_eq!(settle_open(&mut root, ticket).state, initial);
    attach_endpoint(root.endpoint(first, initial.buffer()).unwrap());
    attach_endpoint(root.endpoint(second, initial.buffer()).unwrap());
    Harness { root, shared, first, second, initial }
}

fn attach_client<Model: Family>(
    root: &mut Root<Model>,
    id: u128,
) -> RootClientHandle
where
    Model::State: Clone,
{
    let client = InteractionClientStamp::new(
        ClientIdentity::new(id),
        LifecycleIdentity::new(id + 1, id + 2, Generation::new(1)),
        native::selection(1064),
    );
    let authorization = root.authorization_for(client);
    root.attach_client(RootAttachRequest {
        root: root.owner(),
        client,
        authorization,
    })
    .unwrap()
}

fn settle_open<Model: Family>(
    root: &mut Root<Model>,
    ticket: RootOpenTicket,
) -> RootOpenedBuffer
where
    Model::State: Clone,
{
    // Bounded fixture progress, not a production executor or timed wait.
    for _ in 0..4 {
        if let Some(completed) = root.take_open(ticket).unwrap() {
            assert_eq!(completed.ticket, ticket);
            return completed
                .outcome
                .expect("Resource and initial state ready");
        }
    }
    panic!("fixture open exceeded four bounded progress steps")
}

fn attach_endpoint<Model: Family>(session: &mut Session<Model>)
where
    Model::State: Clone,
{
    let request = InteractionNegotiationRequest::new(
        &session.fixture.session,
        session.fixture.session.stamp(),
        session.fixture.session.scope(),
        session.fixture.session.capabilities(),
    )
    .unwrap();
    assert_eq!(
        session.negotiate(request).status(),
        InteractionNegotiationStatus::Negotiated
    );
    let RepresentationAttachmentRequestBindingOutcome::Bound(request) =
        RepresentationAttachmentRequest::bind(
            &session.fixture.session,
            session.fixture.attachment.clone(),
        )
    else {
        panic!("Representation binding")
    };
    let attached = session.attach(request);
    attached
        .ready_state()
        .expect("actual initial snapshot")
        .initial()
        .validate(&session.fixture.session, &session.fixture.attachment)
        .unwrap();
}

fn round_trip<Model: Family>(
    model: Model,
    source: &[u8],
    payload: &[u8],
    before: &Model::State,
    after: &Model::State,
    snapshots: (&[u8], &[u8]),
) where
    Model::State: Clone + Debug + PartialEq,
{
    let mut case = harness(model, source);
    let subscription = case
        .root
        .endpoint(case.first, case.initial.buffer())
        .unwrap()
        .subscribe_changes(native::count(1))
        .unwrap();
    let changed = edit(&mut case, payload);
    assert_eq!(case.shared.borrow().current.as_ref(), Some(after));
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    session.publish_change(changed.clone());
    let notification =
        session.take_change(&subscription, case.initial.version()).unwrap();
    assert_eq!(notification.notification(), Some(&changed));
    let update = receive_snapshot(session, 3, 4, 1);
    assert_eq!(update.update.carrier().unwrap().bytes(), snapshots.1);
    let undo = session.history_request(5000, InteractionHistoryAction::Undo);
    assert_eq!(
        session.submit_history(undo).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    let result = super::session::next_stamp(&session.current_stamp(), 2, 900);
    let restored =
        session.complete_history(&result, native::edge(900, 7)).unwrap();
    let terminal =
        session.take_history_outcome(undo).outcome().expect("undo terminal");
    assert_eq!(terminal.accepted_state(), Some(restored.accepted()));
    let update = receive_snapshot(session, 4, 5, 2);
    assert_eq!(update.update.carrier().unwrap().bytes(), snapshots.0);
    assert_eq!(case.shared.borrow().current.as_ref(), Some(before));
    let redo = session.history_request(
        5001,
        InteractionHistoryAction::Redo {
            child: Some(BufferHistoryNodeIdentity::new(7)),
        },
    );
    assert_eq!(
        session.submit_history(redo).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    let result = super::session::next_stamp(&session.current_stamp(), 3, 7);
    let redone =
        session.complete_history(&result, native::edge(900, 7)).unwrap();
    assert_eq!(
        session.take_history_outcome(redo).outcome().unwrap().accepted_state(),
        Some(redone.accepted())
    );
    let update = receive_snapshot(session, 5, 6, 3);
    assert_eq!(update.update.carrier().unwrap().bytes(), snapshots.1);
    assert_eq!(case.shared.borrow().current.as_ref(), Some(after));
    assert_eq!(
        session.submit_history(undo).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    assert_eq!(session.take_history_outcome(undo).outcome(), Some(terminal));
    assert_eq!(case.shared.borrow().stamp, result);
    session.unsubscribe_changes(&subscription).unwrap();
    session.unsubscribe_changes(&subscription).unwrap();
    finish(&mut case);
}

pub fn edit<Model: Family>(
    case: &mut Harness<Model>,
    payload: &[u8],
) -> BufferChangeNotification
where
    Model::State: Clone,
{
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let intent = session.intent(4000, payload);
    let request = intent.stamp();
    assert_eq!(
        session.submit_intent(intent).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    let result = super::session::next_stamp(&case.initial, 1, 7);
    let notice = session.complete_edit(&result).expect("actual native edit");
    let lookup =
        InteractionOutcomeRequest::new(&session.fixture.session, request)
            .unwrap();
    let outcome =
        session.take_outcome(lookup).outcome().expect("edit terminal");
    assert_eq!(outcome.accepted_state(), Some(notice.accepted()));
    notice
}

pub fn receive_snapshot<Model: Family>(
    session: &mut Session<Model>,
    predecessor: u64,
    result: u64,
    cursor: u64,
) -> ReceivedSnapshot
where
    Model::State: Clone,
{
    assert!(session.update.is_none());
    session.update = Some(session.make_update(predecessor, result, cursor));
    let stamp = session.update.as_ref().unwrap().stamp().identity();
    let request = RepresentationUpdateRequest::new(
        &session.fixture.session,
        &session.fixture.attachment,
        stamp.cursor(),
    )
    .unwrap();
    let update = session
        .take_update(request)
        .into_update()
        .expect("public update delivery");
    let ack = RepresentationUpdateAcknowledgement::new(
        &session.fixture.session,
        &session.fixture.attachment,
        stamp,
        RepresentationUpdateApplication::Installed,
    )
    .unwrap();
    assert_eq!(
        session.acknowledge_update(ack),
        RepresentationUpdateAcknowledgementResult::Accepted
    );
    ReceivedSnapshot {
        session: session.fixture.session.clone(),
        attachment: session.fixture.attachment.clone(),
        update,
        previous_projection: crate::interaction::support::projection(
            &session.fixture,
            predecessor,
        ),
    }
}

pub fn keyed_observation() -> ObservationPair {
    let mut case = harness(KeyedModel::new(), &[0, 1]);
    let edited = edit(&mut case, &[17]);
    let before = edited.accepted().result();
    let _ = receive_snapshot(
        case.root.endpoint(case.first, case.initial.buffer()).unwrap(),
        3,
        4,
        1,
    );
    let _ = receive_snapshot(
        case.root.endpoint(case.second, case.initial.buffer()).unwrap(),
        3,
        4,
        1,
    );
    let first_sub = case
        .root
        .endpoint(case.first, case.initial.buffer())
        .unwrap()
        .subscribe_changes(native::count(1))
        .unwrap();
    let second_sub = case
        .root
        .endpoint(case.second, case.initial.buffer())
        .unwrap()
        .subscribe_changes(native::count(1))
        .unwrap();
    let session =
        case.root.endpoint(case.first, case.initial.buffer()).unwrap();
    let undo = session.history_request(5100, InteractionHistoryAction::Undo);
    assert_eq!(
        session.submit_history(undo).status(),
        InteractionIntentAdmissionStatus::Admitted
    );
    let result = super::session::next_stamp(&before, 2, 900);
    let notice =
        session.complete_history(&result, native::edge(900, 7)).unwrap();
    assert_eq!(
        session.take_history_outcome(undo).outcome().unwrap().accepted_state(),
        Some(notice.accepted())
    );
    session.publish_change(notice.clone());
    let first_notice = session
        .take_change(&first_sub, before.version())
        .unwrap()
        .into_notification()
        .unwrap();
    session.unsubscribe_changes(&first_sub).unwrap();
    let first = receive_snapshot(session, 4, 5, 2);
    let session =
        case.root.endpoint(case.second, case.initial.buffer()).unwrap();
    session.publish_change(notice.clone());
    let second_notice = session
        .take_change(&second_sub, before.version())
        .unwrap()
        .into_notification()
        .unwrap();
    session.unsubscribe_changes(&second_sub).unwrap();
    let second = receive_snapshot(session, 4, 5, 2);
    assert_eq!(first_notice, second_notice);
    assert_eq!(first_notice, notice);
    assert_ne!(first.session.stamp().client(), second.session.stamp().client());
    assert_ne!(first.session.stamp().handle(), second.session.stamp().handle());
    ObservationPair { owner: case, before, notice, first, second }
}

pub fn finish<Model: Family>(case: &mut Harness<Model>)
where
    Model::State: Clone,
{
    assert_eq!(
        case.root.detach_client(case.first, case.root.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
    assert!(case.root.endpoint(case.second, case.initial.buffer()).is_ok());
    assert_eq!(
        case.root.detach_client(case.first, case.root.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
    assert_eq!(
        case.root.detach_client(case.second, case.root.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
    assert_eq!(
        case.root.shutdown(case.root.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
    assert_eq!(
        case.root.shutdown(case.root.budget()).unwrap(),
        ShutdownOutcome::Closed
    );
}
