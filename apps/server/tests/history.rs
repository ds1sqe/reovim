use std::rc::Rc;

use reovim_host_reference::DeterministicClock;
use reovim_kernel_interface::buffer::{
    BufferHistoryNodeIdentity, BufferStateStamp,
};
use reovim_kernel_interface::clock::ClockDescriptor;
use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::notification::{
    BufferChangeDeliveryStatus, BufferChangePort, BufferChangeSubscription,
};
use reovim_kernel_interface::interaction::outcome::{
    InteractionAcceptedState, InteractionOutcomeCause, InteractionOutcomeStatus,
};
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootOpenRequest, RootOpenTarget,
};
use reovim_kernel_interface::lifecycle::LifecycleIdentity;
use reovim_kernel_interface::resource::state::ResourceStateModel;
use reovim_kernel_interface::semantic::domain::DomainPolicyPort;
use reovim_kernel_interface::semantic::representation::RepresentationPolicyPort;
use reovim_kernel_interface::semantic::snapshot::RepresentationSnapshotPort;
use reovim_kernel_runtime::diagnostics::Telemetry;
use reovim_resource_local::profile;
use reovim_server::assembly::{self, Service, life};

#[test]
fn heading_history_restores_both_siblings_and_remembers_explicit_redo() {
    let owner = life(11);
    let service =
        assembly::heading(owner, b"# a\n\xffopaque".to_vec(), telemetry(owner))
            .unwrap();
    exercise_tree(
        service,
        &BranchData {
            edit_b: b"b",
            edit_c: b"c",
            state_a: b"H\x01a",
            state_b: b"H\x01b",
            state_c: b"H\x01c",
        },
    );
}

#[test]
fn keyed_history_restores_named_records_without_a_text_edit_contract() {
    let owner = life(12);
    let service =
        assembly::keyed(owner, vec![93, 1, 17, 0], telemetry(owner)).unwrap();
    exercise_tree(
        service,
        &BranchData {
            edit_b: &[17],
            edit_c: &[93],
            state_a: &[b'K', 17, 0, 93, 1],
            state_b: &[b'K', 17, 1, 93, 1],
            state_c: &[b'K', 17, 0, 93, 0],
        },
    );
}

#[test]
fn negotiated_history_does_not_require_a_representation_attachment() {
    let owner = life(13);
    let mut service =
        assembly::heading(owner, b"# a\n\xff".to_vec(), telemetry(owner))
            .unwrap();
    service.open(0).unwrap();
    let node_a = service.clients[0].state().unwrap().history().node();
    let edit = service.edit(0, 10, b"b").unwrap();
    let before = edit.accepted_state().unwrap().result();
    service.receive(0).unwrap();
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
    assert!(opened.history);
    service.clients[1].opened(handle, opened).unwrap();
    let client = &service.clients[1];
    let endpoint = service.runtime.endpoint(handle, before.buffer()).unwrap();
    client.negotiate(endpoint).unwrap();
    assert_eq!(service.clients[1].payload(), None);
    let outcome =
        service.history(1, 11, InteractionHistoryAction::Undo).unwrap();
    assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
    let current = outcome.accepted_state().unwrap().result();
    assert_eq!(current.history().node(), node_a);
    assert_ne!(current.version(), before.version());
    let client = &mut service.clients[1];
    let endpoint = service.runtime.endpoint(handle, before.buffer()).unwrap();
    client.initialize(endpoint).unwrap();
    assert_eq!(client.state(), Some(current));
    assert_eq!(client.payload(), Some(&b"H\x01a"[..]));
    service.receive(0).unwrap();
    assert_eq!(service.clients[0].state(), Some(current));
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01a"[..]));
    service.close().unwrap();
}

struct BranchData<'a> {
    edit_b: &'a [u8],
    edit_c: &'a [u8],
    state_a: &'a [u8],
    state_b: &'a [u8],
    state_c: &'a [u8],
}

fn exercise_tree<M, D, R, S>(
    service: Service<M, D, R, S>,
    data: &BranchData<'_>,
) where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    use InteractionHistoryAction::{Redo, Undo};

    let mut journey = Journey::new(service);
    let node_a = journey.state().history().node();
    journey.assert_views(data.state_a);
    journey.reject(0, 10, Undo);
    let node_b =
        journey.edit(0, 11, data.edit_b, data.state_b).history().node();
    journey.reject(1, 12, Redo { child: None });
    journey.restore(1, 13, Undo, node_a, data.state_a);
    let node_c =
        journey.edit(1, 14, data.edit_c, data.state_c).history().node();
    assert_ne!(node_b, node_c);
    // B exists, but is C's sibling rather than its redo child.
    journey.reject(0, 15, Redo { child: Some(node_b) });
    journey.restore(0, 16, Undo, node_a, data.state_a);
    journey.restore(1, 17, Redo { child: None }, node_c, data.state_c);
    journey.restore(0, 18, Undo, node_a, data.state_a);
    journey.restore(1, 19, Redo { child: Some(node_b) }, node_b, data.state_b);
    journey.restore(0, 20, Undo, node_a, data.state_a);
    journey.restore(1, 21, Redo { child: None }, node_b, data.state_b);
    journey.service.close().unwrap();
}

struct Journey<M: ResourceStateModel, D, R, S> {
    service: Service<M, D, R, S>,
    subscriptions: [BufferChangeSubscription; 2],
}

impl<M, D, R, S> Journey<M, D, R, S>
where
    M: ResourceStateModel,
    D: DomainPolicyPort<State = M::State>,
    R: RepresentationPolicyPort<State = M::State>,
    S: RepresentationSnapshotPort<State = M::State>,
{
    fn new(mut service: Service<M, D, R, S>) -> Self {
        service.open(0).unwrap();
        service.open(1).unwrap();
        assert_ne!(service.clients[0].handle(), service.clients[1].handle());
        let first = service
            .endpoint(0)
            .unwrap()
            .subscribe_changes(profile::count(1))
            .unwrap();
        let second = service
            .endpoint(1)
            .unwrap()
            .subscribe_changes(profile::count(1))
            .unwrap();
        assert_eq!(first.session, service.clients[0].session.stamp().handle());
        assert_eq!(second.session, service.clients[1].session.stamp().handle());
        Self { service, subscriptions: [first, second] }
    }

    fn state(&self) -> BufferStateStamp {
        self.service.clients[0].state().unwrap()
    }

    fn assert_views(&self, expected: &[u8]) {
        assert_eq!(self.service.clients[0].payload(), Some(expected));
        assert_eq!(self.service.clients[1].payload(), Some(expected));
        assert_eq!(
            self.service.clients[0].state(),
            self.service.clients[1].state()
        );
    }

    fn edit(
        &mut self,
        writer: usize,
        request: u128,
        input: &[u8],
        expected: &[u8],
    ) -> BufferStateStamp {
        let prior = self.state();
        let outcome = self.service.edit(writer, request, input).unwrap();
        assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
        let accepted = outcome.accepted_state().unwrap();
        assert_eq!(accepted.history_direction(), None);
        assert_eq!(accepted.history_edge().parent(), prior.history().node());
        assert_eq!(
            accepted.history_edge().child(),
            accepted.result().history().node()
        );
        self.observe(writer, request, &prior, &accepted, expected)
    }

    fn restore(
        &mut self,
        writer: usize,
        request: u128,
        action: InteractionHistoryAction,
        destination: BufferHistoryNodeIdentity,
        expected: &[u8],
    ) {
        let prior = self.state();
        let outcome = self.service.history(writer, request, action).unwrap();
        assert_eq!(outcome.status(), InteractionOutcomeStatus::Accepted);
        assert_eq!(outcome.request().action(), action);
        assert_eq!(outcome.request().target(), prior);
        let accepted = outcome.accepted_state().unwrap();
        let edge = accepted.history_edge();
        assert_eq!(accepted.history_direction(), Some(action.direction()));
        assert_eq!(edge.from(action.direction()), prior.history().node());
        assert_eq!(edge.to(action.direction()), destination);
        assert_eq!(accepted.result().history().node(), destination);
        assert_eq!(
            accepted.transaction(),
            outcome.request().plan().transaction()
        );
        assert_eq!(
            accepted.history(),
            outcome.request().plan().history_transition()
        );
        self.observe(writer, request, &prior, &accepted, expected);
    }

    fn reject(
        &mut self,
        writer: usize,
        request: u128,
        action: InteractionHistoryAction,
    ) {
        let prior = self.state();
        let payload = self.service.clients[0].payload().unwrap().to_vec();
        let outcome = self.service.history(writer, request, action).unwrap();
        assert_eq!(outcome.status(), InteractionOutcomeStatus::NotAccepted);
        assert_eq!(outcome.cause(), Some(InteractionOutcomeCause::NoHistory));
        assert_eq!(outcome.accepted_state(), None);
        assert_eq!(outcome.request().target(), prior);
        assert_eq!(self.state(), prior);
        self.assert_views(&payload);
        self.assert_no_changes(&prior);
    }

    fn observe(
        &mut self,
        writer: usize,
        request: u128,
        prior: &BufferStateStamp,
        accepted: &InteractionAcceptedState,
        expected: &[u8],
    ) -> BufferStateStamp {
        let result = accepted.result();
        assert_eq!(result.source(), prior.source());
        assert_eq!(result.buffer(), prior.buffer());
        assert_eq!(result.history().lifecycle(), prior.history().lifecycle());
        assert_eq!(
            result.version().value(),
            prior.version().value().checked_add(1).unwrap()
        );
        self.observe_notices(writer, request, prior, accepted);
        assert_eq!(self.service.clients[0].state(), Some(*prior));
        assert_eq!(self.service.clients[1].state(), Some(*prior));
        self.service.receive(1).unwrap();
        assert_eq!(self.service.clients[0].state(), Some(*prior));
        assert_eq!(self.service.clients[1].state(), Some(result));
        self.service.receive(0).unwrap();
        self.assert_views(expected);
        assert_eq!(self.state(), result);
        result
    }

    fn observe_notices(
        &mut self,
        writer: usize,
        request: u128,
        prior: &BufferStateStamp,
        accepted: &InteractionAcceptedState,
    ) {
        let first = self
            .service
            .endpoint(0)
            .unwrap()
            .take_change(&self.subscriptions[0], prior.version())
            .unwrap();
        let second = self
            .service
            .endpoint(1)
            .unwrap()
            .take_change(&self.subscriptions[1], prior.version())
            .unwrap();
        assert_eq!(first.status(), BufferChangeDeliveryStatus::Changed);
        assert_eq!(second.status(), BufferChangeDeliveryStatus::Changed);
        assert_eq!(first.subscription(), self.subscriptions[0].lifecycle);
        assert_eq!(second.subscription(), self.subscriptions[1].lifecycle);
        let notice = first.notification().unwrap();
        assert_eq!(Some(notice), second.notification());
        assert_eq!(
            notice.origin(),
            self.service.clients[writer].session.stamp().handle()
        );
        assert_eq!(notice.request(), InteractionRequestIdentity::new(request));
        assert_eq!(notice.before(), *prior);
        assert_eq!(notice.accepted(), *accepted);
        assert_eq!(first.checkpoint(), Some(accepted.result()));
        assert_eq!(second.checkpoint(), Some(accepted.result()));
    }

    fn assert_no_changes(&mut self, current: &BufferStateStamp) {
        let first = self
            .service
            .endpoint(0)
            .unwrap()
            .take_change(&self.subscriptions[0], current.version())
            .unwrap();
        let second = self
            .service
            .endpoint(1)
            .unwrap()
            .take_change(&self.subscriptions[1], current.version())
            .unwrap();
        assert_eq!(first.status(), BufferChangeDeliveryStatus::Pending);
        assert_eq!(second.status(), BufferChangeDeliveryStatus::Pending);
    }
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
