use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{ModelOps, ModelProvider, State},
    provider::Permissions,
    session::{self, Observation},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    future::{Future, ready},
    rc::Rc,
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

fn setup() -> (Runtime, Connection, Observation) {
    setup_with(Limits::default())
}

fn setup_with(limits: Limits) -> (Runtime, Connection, Observation) {
    let mut runtime = Runtime::new(232, limits).unwrap();
    runtime.start().unwrap();
    let model = ModelProvider::new(
        (),
        ModelOps {
            inspect: None,
            unload: None,
            initialize: |(), input| {
                Box::pin(ready(Ok(State::new(input.data, 1))))
            },
            prepare: |(), _, input| {
                Box::pin(ready(Ok(State::new(input.data, 1))))
            },
            observe: |(), state, input| {
                Ok(Message::new(
                    input.contract,
                    state.get::<Box<[u8]>>().unwrap(),
                ))
            },
        },
        Contract::new("initial", 1),
        Contract::new("focus", 1),
        Contract::new("set", 1),
        Rc::new(()),
    );
    let model = runtime
        .register_model("model", model)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(model, Permissions::ALL)]).unwrap(),
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
    (runtime, client, initial)
}

fn attach(
    runtime: &mut Runtime,
    client: &Connection,
    session: ResourceId,
) -> ResourceId {
    session::attached(
        &finish(runtime, session::attach(client, session)).unwrap(),
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

fn update(
    runtime: &mut Runtime,
    client: &Connection,
    observed: &Observation,
    value: u8,
) -> Observation {
    let input = session::update(
        observed,
        Message::new(Contract::new("set", 1), &[value]),
    )
    .unwrap();
    session::observation(
        finish(runtime, client.write(observed.session, &input)).unwrap(),
    )
    .unwrap()
}

fn peer(
    runtime: &mut Runtime,
    client: &Connection,
    session: ResourceId,
) -> (Connection, ResourceId) {
    let sessions = client.resolve("sessions").into_inner().unwrap();
    let name = client
        .children(sessions)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == session)
        .unwrap()
        .name;
    let root = runtime.resource("sessions").unwrap();
    let peer = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::READ)]).unwrap(),
    );
    let id = peer.resolve(&format!("sessions/{name}")).into_inner().unwrap();
    (peer, id)
}

#[test]
fn independent_views_retain_history_after_input_release_and_detach() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let (peer, session) = peer(&mut runtime, &client, initial.session);
    let other = attach(&mut runtime, &peer, session);
    let changed = update(&mut runtime, &client, &initial, 2);
    finish(&mut runtime, session::seek(&client, own, initial.snapshot))
        .unwrap();
    finish(&mut runtime, session::release(&client, initial.snapshot)).unwrap();
    let historical =
        session::observation(read(&mut runtime, &client, own).unwrap())
            .unwrap();
    assert_eq!(historical.session, initial.session);
    assert_eq!(historical.value.data.as_ref(), &[1]);
    assert_ne!(historical.snapshot, initial.snapshot);
    let live = session::observation(read(&mut runtime, &peer, other).unwrap())
        .unwrap();
    assert_eq!(live.session, session);
    assert_eq!(live.value.data.as_ref(), &[2]);
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    assert_eq!(
        read(&mut runtime, &client, own),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        client.capture(own, Permissions::READ).err(),
        Some(Error::CLOSED)
    );
    assert_eq!(
        read(&mut runtime, &client, historical.snapshot).unwrap().data.as_ref(),
        &[1]
    );
    assert_eq!(read(&mut runtime, &peer, other).unwrap().data.as_ref(), &[2]);
    finish(&mut runtime, session::close(&client, changed.session)).unwrap();
    assert_eq!(
        read(&mut runtime, &peer, other),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        read(&mut runtime, &peer, live.snapshot).unwrap().data.as_ref(),
        &[2]
    );
    finish(&mut runtime, session::detach(&peer, other)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn admitted_seek_retains_released_input_and_foreign_seek_preserves_view() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let model = client.resolve("model").into_inner().unwrap();
    let foreign = finish(
        &mut runtime,
        session::create(
            &client,
            model,
            &Message::new(Contract::new("initial", 1), &[9]),
        ),
    )
    .unwrap();
    let release = client.write(
        initial.snapshot,
        &Message::new(Contract::new("kernel.session.release", 1), &[]),
    );
    // Admission captures the snapshot before its public handle is released.
    let seek = session::seek(&client, own, initial.snapshot);
    finish(&mut runtime, release).unwrap();
    finish(&mut runtime, seek).unwrap();
    assert_eq!(
        finish(&mut runtime, session::seek(&client, own, foreign.snapshot)),
        Err(RequestError::Failed(Error::STALE))
    );
    assert_eq!(read(&mut runtime, &client, own).unwrap().data.as_ref(), &[1]);
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    finish(&mut runtime, session::close(&client, foreign.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn retained_read_blocks_detach_until_its_snapshot_is_adopted() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let reading =
        client.read(own, &Message::new(Contract::new("focus", 1), &[]));
    assert_eq!(
        finish(&mut runtime, session::detach(&client, own)),
        Err(RequestError::Failed(Error::BUSY))
    );
    let observed =
        session::observation(finish(&mut runtime, reading).unwrap()).unwrap();
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    assert_eq!(
        read(&mut runtime, &client, observed.snapshot).unwrap().data.as_ref(),
        &[1]
    );
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn namespace_grants_do_not_transfer_participation_ownership() {
    let (mut runtime, client, initial) = setup();
    let own = attach(&mut runtime, &client, initial.session);
    let root = client.resolve("participations").into_inner().unwrap();
    let name = client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == own)
        .unwrap()
        .name;
    let root = runtime.resource("participations").unwrap();
    let foreign = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    let id = foreign
        .resolve(&format!("participations/{name}"))
        .into_inner()
        .unwrap();
    assert_eq!(
        read(&mut runtime, &foreign, id),
        Err(RequestError::Failed(Error::DENIED))
    );
    assert_eq!(
        finish(&mut runtime, session::detach(&foreign, id)),
        Err(RequestError::Failed(Error::DENIED))
    );
    assert_eq!(read(&mut runtime, &client, own).unwrap().data.as_ref(), &[1]);
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn lost_attach_recovers_the_original_participation() {
    let (mut runtime, client, initial) = setup();
    let attaching = session::attach(&client, initial.session);
    let next = client.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        finish(&mut runtime, attaching)
    else {
        panic!("retained attach");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let first = finish(
        &mut runtime,
        reovim_kernel_interface::requests::result(&next, work),
    )
    .unwrap()
    .unwrap();
    let repeated = finish(
        &mut runtime,
        reovim_kernel_interface::requests::result(&next, work),
    )
    .unwrap()
    .unwrap();
    assert_eq!(first, repeated);
    next.close(work).into_inner().unwrap();
    let own = session::attached(&first).unwrap();
    let observed =
        session::observation(read(&mut runtime, &next, own).unwrap()).unwrap();
    assert_eq!(observed.value.data.as_ref(), &[1]);
    finish(&mut runtime, session::detach(&next, own)).unwrap();
    finish(&mut runtime, session::close(&next, observed.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn private_client_retirement_releases_view_without_closing_shared_session() {
    let (mut runtime, client, initial) = setup();
    let root = client.resolve("sessions").into_inner().unwrap();
    let name = client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == initial.session)
        .unwrap()
        .name;
    let shared = runtime.resource(&format!("sessions/{name}")).unwrap();
    runtime
        .register(
            "clients/peer/marker",
            reovim_resource_local::memory(b"private"),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let root = runtime.resource("clients/peer").unwrap();
    let binding = runtime
        .bind_at(root, &[(root, Permissions::ALL), (shared, Permissions::READ)])
        .unwrap();
    runtime.mount(&binding, "session", shared).unwrap();
    let peer = reovim_sdk::local(binding.connection());
    let subject = peer.resolve("session").into_inner().unwrap();
    let own = attach(&mut runtime, &peer, subject);
    let historical =
        session::observation(read(&mut runtime, &peer, own).unwrap()).unwrap();
    finish(&mut runtime, session::seek(&peer, own, historical.snapshot))
        .unwrap();
    finish(&mut runtime, session::release(&peer, historical.snapshot)).unwrap();
    runtime.retire_client(&binding).unwrap();
    assert_eq!(
        peer.root().into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        read(&mut runtime, &client, initial.session).unwrap().data.as_ref(),
        &[1]
    );
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn failed_snapshot_copy_preserves_historical_selection() {
    let limits = Limits { resources: 32, ..Limits::default() };
    let (mut runtime, client, initial) = setup_with(limits);
    let own = attach(&mut runtime, &client, initial.session);
    finish(&mut runtime, session::seek(&client, own, initial.snapshot))
        .unwrap();
    let changed = update(&mut runtime, &client, &initial, 2);
    let root = client.resolve("participations").into_inner().unwrap();
    let before = client.children(root).into_inner().unwrap();
    let mut blockers: Vec<_> = (0..limits.resources)
        .map_while(|index| blocker(&mut runtime, index))
        .collect();
    runtime.unregister(blockers.pop().expect("filled capacity")).unwrap();
    assert_eq!(
        finish(&mut runtime, session::attach(&client, initial.session)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert_eq!(client.children(root).into_inner().unwrap(), before);
    assert_eq!(
        read(&mut runtime, &client, own),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    runtime.unregister_contributions(&blockers).unwrap();
    assert_eq!(read(&mut runtime, &client, own).unwrap().data.as_ref(), &[1]);
    assert_eq!(
        read(&mut runtime, &client, changed.session).unwrap().data.as_ref(),
        &[2]
    );
    finish(&mut runtime, session::detach(&client, own)).unwrap();
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    runtime.close().unwrap();
}

fn blocker(runtime: &mut Runtime, index: usize) -> Option<ResourceId> {
    match runtime.register(
        &format!("held-{index}"),
        reovim_resource_local::memory(b"held"),
    ) {
        Ok(id) => Some(id),
        Err((error, _)) => {
            assert_eq!(error, Error::PRESSURE);
            None
        }
    }
}
