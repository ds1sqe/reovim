#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    access::Reply,
    buffer::{ModelOps, ModelProvider, State, StateReply},
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Module},
    notify::Next,
    provider::{Contribution, Permissions},
    requests,
    session::{self, Observation, Watching},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::{Cell, RefCell},
    future::{Future, ready},
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[derive(Default)]
struct Gate {
    result: Option<Result<(), Error>>,
    wake: Option<Waker>,
    dropped: bool,
}

struct Pending {
    gate: Rc<RefCell<Gate>>,
    value: Box<[u8]>,
}

impl Future for Pending {
    type Output = Result<State, Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut gate = self.gate.borrow_mut();
        let Some(result) = gate.result else {
            gate.wake = Some(cx.waker().clone());
            return Poll::Pending;
        };
        Poll::Ready(result.map(|()| State::new(self.value.clone(), 1)))
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.gate.borrow_mut().dropped = true;
    }
}

struct Code(Rc<Cell<bool>>);
impl Drop for Code {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

fn prepare(gate: &Rc<RefCell<Gate>>, input: Message) -> StateReply {
    if input.data.as_ref() == [2] {
        return Box::pin(Pending { gate: gate.clone(), value: input.data });
    }
    Box::pin(ready(Ok(State::new(input.data, 1))))
}

struct Harness {
    runtime: Runtime,
    client: Connection,
    initial: Observation,
    participant: ResourceId,
    instance: ResourceId,
    gate: Rc<RefCell<Gate>>,
    lifecycle: Rc<Cell<Lifecycle>>,
    released: Rc<Cell<bool>>,
}

fn setup() -> Harness {
    let mut runtime = Runtime::new(234, Limits::default()).unwrap();
    runtime.start().unwrap();
    let gate = Rc::new(RefCell::new(Gate::default()));
    let lifecycle = Rc::new(Cell::new(Lifecycle::Ready));
    let released = Rc::new(Cell::new(false));
    let code = Rc::new(Code(released.clone()));
    let model = ModelProvider::new(
        gate.clone(),
        ModelOps {
            inspect: None,
            unload: None,
            initialize: |_, input| {
                Box::pin(ready(Ok(State::new(input.data, 1))))
            },
            prepare: |gate, _, input| prepare(gate, input),
            observe: |_, state, input| {
                Ok(Message::new(
                    input.contract,
                    state.get::<Box<[u8]>>().unwrap(),
                ))
            },
        },
        Contract::new("initial", 1),
        Contract::new("focus", 1),
        Contract::new("set", 1),
        code.clone(),
    );
    let module = Module::new(
        lifecycle.clone(),
        DriverOps {
            status: |state| state.get(),
            advance: |state, _| {
                Ok(if state.get() == Lifecycle::Closed {
                    Progress::Stopped
                } else {
                    Progress::Idle
                })
            },
            close: |state| {
                state.set(Lifecycle::Closed);
                Ok(())
            },
        },
        vec![Contribution::Model { path: "model".into(), provider: model }],
        code,
    );
    let instance = runtime
        .activate_module("module", module)
        .map_err(|(error, _)| error)
        .unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let model = runtime.resource("model").unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[(model, Permissions::ALL), (instance, Permissions::ALL)])
            .unwrap(),
    );
    let model = client.resolve("model").into_inner().unwrap();
    let initial = finish(
        &mut runtime,
        session::create(
            &client,
            model,
            &Message::new(Contract::new("initial", 1), &[1]),
        ),
    )
    .unwrap();
    let participant = session::attached(
        &finish(&mut runtime, session::attach(&client, initial.session))
            .unwrap(),
    )
    .unwrap();
    let instance = client.resolve("module").into_inner().unwrap();
    Harness {
        runtime,
        client,
        initial,
        participant,
        instance,
        gate,
        lifecycle,
        released,
    }
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

fn edit(
    client: &Connection,
    participant: ResourceId,
    initial: &Observation,
    value: u8,
) -> Reply {
    client.write(
        participant,
        &session::update(
            initial,
            Message::new(Contract::new("set", 1), &[value]),
        )
        .unwrap(),
    )
}

fn follow(h: &mut Harness) -> Watching {
    session::followed(
        &finish(&mut h.runtime, session::follow(&h.client, h.participant))
            .unwrap(),
    )
    .unwrap()
}

fn read(
    runtime: &mut Runtime,
    client: &Connection,
    subject: ResourceId,
) -> Result<Message, RequestError> {
    finish(
        runtime,
        client.read(subject, &Message::new(Contract::new("focus", 1), &[])),
    )
}

fn park(h: &mut Harness, waiting: &mut Reply) {
    let mut cx = Context::from_waker(Waker::noop());
    assert!(waiting.as_mut().poll(&mut cx).is_pending());
    h.runtime.advance(&mut cx).unwrap();
    h.runtime.advance(&mut cx).unwrap();
    assert!(h.gate.borrow().wake.is_some(), "preparation must be parked");
    assert!(!h.gate.borrow().dropped);
}

fn open(gate: &Rc<RefCell<Gate>>) {
    let wake = {
        let mut gate = gate.borrow_mut();
        gate.result = Some(Ok(()));
        gate.wake.take()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}

fn busy(h: &mut Harness) {
    assert_eq!(
        finish(&mut h.runtime, session::detach(&h.client, h.participant)),
        Err(RequestError::Failed(Error::BUSY))
    );
    assert_eq!(
        finish(&mut h.runtime, session::close(&h.client, h.initial.session)),
        Err(RequestError::Failed(Error::BUSY))
    );
    assert_eq!(
        finish(&mut h.runtime, module::unload(&h.client, h.instance)),
        Err(RequestError::Failed(Error::BUSY))
    );
    assert!(!h.released.get());
}

fn participation_path(client: &Connection, participant: ResourceId) -> String {
    let root = client.resolve("participations").into_inner().unwrap();
    let name = client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == participant)
        .unwrap()
        .name;
    format!("participations/{name}")
}

#[test]
fn cancelled_preparation_retains_recovery_and_never_installs_late_state() {
    let mut h = setup();
    let path = participation_path(&h.client, h.participant);
    let mut waiting = edit(&h.client, h.participant, &h.initial, 2);
    park(&mut h, &mut waiting);
    busy(&mut h);
    let next = h.client.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        finish(&mut h.runtime, waiting)
    else {
        panic!("retained request");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    finish(
        &mut h.runtime,
        next.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
    )
    .unwrap();
    assert!(!h.gate.borrow().dropped);
    open(&h.gate);
    assert_eq!(
        finish(&mut h.runtime, requests::result(&next, work)),
        Ok(Err(RequestError::Failed(Error::CANCELLED)))
    );
    assert!(h.gate.borrow().dropped);
    next.close(work).into_inner().unwrap();
    h.client = next;
    h.participant = h.client.resolve(&path).into_inner().unwrap();
    h.instance = h.client.resolve("module").into_inner().unwrap();
    let observed = follow(&mut h);
    assert_eq!(observed.observation.value.data.as_ref(), &[1]);
    finish(&mut h.runtime, session::detach(&h.client, h.participant)).unwrap();
    finish(
        &mut h.runtime,
        session::close(&h.client, observed.observation.session),
    )
    .unwrap();
    finish(&mut h.runtime, module::unload(&h.client, h.instance)).unwrap();
    assert!(h.released.get());
    assert_eq!(
        read(&mut h.runtime, &h.client, observed.observation.snapshot)
            .unwrap()
            .data
            .as_ref(),
        &[1]
    );
    h.client.close(observed.watch).into_inner().unwrap();
    finish(
        &mut h.runtime,
        session::release(&h.client, observed.observation.snapshot),
    )
    .unwrap();
    h.runtime.close().unwrap();
}

#[test]
fn pending_candidate_revalidates_after_another_context_update() {
    let mut h = setup();
    let observed = follow(&mut h);
    let mut waiting = edit(&h.client, h.participant, &h.initial, 2);
    park(&mut h, &mut waiting);
    busy(&mut h);
    let changed = session::observation(
        finish(
            &mut h.runtime,
            edit(&h.client, h.initial.session, &h.initial, 3),
        )
        .unwrap(),
    )
    .unwrap();
    open(&h.gate);
    assert_eq!(
        finish(&mut h.runtime, waiting),
        Err(RequestError::Failed(Error::STALE))
    );
    assert_eq!(
        read(&mut h.runtime, &h.client, h.participant).unwrap().data.as_ref(),
        &[3]
    );
    let edge = session::change(
        &finish(
            &mut h.runtime,
            session::next(&h.client, observed.watch, false),
        )
        .unwrap(),
    )
    .unwrap();
    let Next::Item(edge) = edge else {
        panic!("accepted peer update");
    };
    assert_eq!(edge.before, observed.revision);
    assert_eq!(edge.after, observed.revision + 1);
    assert_eq!(
        session::change(
            &finish(
                &mut h.runtime,
                session::next(&h.client, observed.watch, false)
            )
            .unwrap()
        )
        .unwrap(),
        Next::Pending
    );
    finish(&mut h.runtime, session::close(&h.client, changed.session)).unwrap();
    finish(&mut h.runtime, module::unload(&h.client, h.instance)).unwrap();
    assert!(h.released.get());
    assert_eq!(
        read(&mut h.runtime, &h.client, changed.snapshot)
            .unwrap()
            .data
            .as_ref(),
        &[3]
    );
    h.client.close(observed.watch).into_inner().unwrap();
    h.runtime.close().unwrap();
}

#[test]
fn provider_failure_before_installation_preserves_context_and_cleanup() {
    let mut h = setup();
    let observed = follow(&mut h);
    let mut waiting = edit(&h.client, h.participant, &h.initial, 2);
    park(&mut h, &mut waiting);
    h.lifecycle.set(Lifecycle::Failed(Error::PROVIDER));
    h.runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    open(&h.gate);
    assert_eq!(
        finish(&mut h.runtime, waiting),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    assert_eq!(
        session::change(
            &finish(
                &mut h.runtime,
                session::next(&h.client, observed.watch, false)
            )
            .unwrap()
        )
        .unwrap(),
        Next::Pending
    );
    assert_eq!(
        read(&mut h.runtime, &h.client, observed.observation.snapshot)
            .unwrap()
            .data
            .as_ref(),
        &[1]
    );
    finish(&mut h.runtime, session::detach(&h.client, h.participant)).unwrap();
    finish(&mut h.runtime, session::close(&h.client, h.initial.session))
        .unwrap();
    finish(&mut h.runtime, module::unload(&h.client, h.instance)).unwrap();
    assert!(h.released.get());
    h.client.close(observed.watch).into_inner().unwrap();
    h.runtime.close().unwrap();
}

#[test]
fn client_retirement_settles_its_parked_watch_without_closing_shared_context() {
    let mut h = setup();
    let root = h.client.resolve("sessions").into_inner().unwrap();
    let name = h
        .client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == h.initial.session)
        .unwrap()
        .name;
    let shared = h.runtime.resource(&format!("sessions/{name}")).unwrap();
    h.runtime
        .register("clients/peer/marker", memory::provider(b"private"))
        .map_err(|(error, _)| error)
        .unwrap();
    let root = h.runtime.resource("clients/peer").unwrap();
    let binding = h
        .runtime
        .bind_at(root, &[(root, Permissions::ALL), (shared, Permissions::READ)])
        .unwrap();
    h.runtime.mount(&binding, "session", shared).unwrap();
    let peer = reovim_sdk::local(binding.connection());
    let subject = peer.resolve("session").into_inner().unwrap();
    let participant = session::attached(
        &finish(&mut h.runtime, session::attach(&peer, subject)).unwrap(),
    )
    .unwrap();
    let followed = session::followed(
        &finish(&mut h.runtime, session::follow(&peer, participant)).unwrap(),
    )
    .unwrap();
    let mut waiting = session::next(&peer, followed.watch, true);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(waiting.as_mut().poll(&mut cx).is_pending());
    h.runtime.advance(&mut cx).unwrap();
    h.runtime.advance(&mut cx).unwrap();
    assert!(waiting.as_mut().poll(&mut cx).is_pending());
    assert_eq!(h.runtime.retire_client(&binding), Err(Error::BUSY));
    assert_eq!(
        finish(&mut h.runtime, waiting),
        Err(RequestError::Failed(Error::CLOSED))
    );
    h.runtime.retire_client(&binding).unwrap();
    assert_eq!(
        peer.root().into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        read(&mut h.runtime, &h.client, h.initial.session)
            .unwrap()
            .data
            .as_ref(),
        &[1]
    );
    finish(&mut h.runtime, session::close(&h.client, h.initial.session))
        .unwrap();
    finish(&mut h.runtime, module::unload(&h.client, h.instance)).unwrap();
    assert!(h.released.get());
    h.runtime.close().unwrap();
}

#[test]
fn accepted_update_recovery_never_replays_over_a_later_update() {
    let mut h = setup();
    let path = participation_path(&h.client, h.participant);
    let waiting = edit(&h.client, h.participant, &h.initial, 3);
    let next = h.client.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        finish(&mut h.runtime, waiting)
    else {
        panic!("retained update");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let original =
        finish(&mut h.runtime, requests::result(&next, work)).unwrap().unwrap();
    let accepted = session::observation(original.clone()).unwrap();
    assert_eq!(accepted.value.data.as_ref(), &[3]);
    let participant = next.resolve(&path).into_inner().unwrap();
    let changed = session::observation(
        finish(&mut h.runtime, edit(&next, participant, &accepted, 4)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        finish(&mut h.runtime, requests::result(&next, work)),
        Ok(Ok(original))
    );
    assert_eq!(
        read(&mut h.runtime, &next, participant).unwrap().data.as_ref(),
        &[4]
    );
    assert_eq!(
        finish(&mut h.runtime, session::close(&next, changed.session)),
        Err(RequestError::Failed(Error::BUSY))
    );
    next.close(work).into_inner().unwrap();
    finish(&mut h.runtime, session::detach(&next, participant)).unwrap();
    finish(&mut h.runtime, session::close(&next, changed.session)).unwrap();
    let instance = next.resolve("module").into_inner().unwrap();
    finish(&mut h.runtime, module::unload(&next, instance)).unwrap();
    assert!(h.released.get());
    assert_eq!(
        read(&mut h.runtime, &next, accepted.snapshot).unwrap().data.as_ref(),
        &[3]
    );
    h.runtime.close().unwrap();
}

#[test]
fn seek_recovery_does_not_restore_history_after_a_later_follow_and_edit() {
    let mut h = setup();
    finish(&mut h.runtime, edit(&h.client, h.participant, &h.initial, 3))
        .unwrap();
    let path = participation_path(&h.client, h.participant);
    let seeking = session::seek(&h.client, h.participant, h.initial.snapshot);
    let next = h.client.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        finish(&mut h.runtime, seeking)
    else {
        panic!("retained seek");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let original =
        finish(&mut h.runtime, requests::result(&next, work)).unwrap().unwrap();
    assert_eq!(
        original,
        Message::new(Contract::new("kernel.session.seek", 1), &[])
    );
    let participant = next.resolve(&path).into_inner().unwrap();
    let historical =
        session::observation(read(&mut h.runtime, &next, participant).unwrap())
            .unwrap();
    assert_eq!(historical.value.data.as_ref(), &[1]);
    let live = session::followed(
        &finish(&mut h.runtime, session::follow(&next, participant)).unwrap(),
    )
    .unwrap();
    assert_eq!(live.observation.value.data.as_ref(), &[3]);
    let changed = session::observation(
        finish(&mut h.runtime, edit(&next, participant, &live.observation, 4))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        finish(&mut h.runtime, requests::result(&next, work)),
        Ok(Ok(original))
    );
    assert_eq!(
        read(&mut h.runtime, &next, participant).unwrap().data.as_ref(),
        &[4]
    );
    next.close(work).into_inner().unwrap();
    finish(&mut h.runtime, session::detach(&next, participant)).unwrap();
    finish(&mut h.runtime, session::close(&next, changed.session)).unwrap();
    let instance = next.resolve("module").into_inner().unwrap();
    finish(&mut h.runtime, module::unload(&next, instance)).unwrap();
    assert!(h.released.get());
    assert_eq!(
        read(&mut h.runtime, &next, historical.snapshot).unwrap().data.as_ref(),
        &[1]
    );
    next.close(live.watch).into_inner().unwrap();
    h.runtime.close().unwrap();
}
