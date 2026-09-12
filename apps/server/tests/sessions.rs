use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{ModelOps, ModelProvider, State, StateReply},
    provider::Permissions,
    session::{self, Observation},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::Cell, future::ready, rc::Rc};

struct Code(Rc<Cell<bool>>);
impl Drop for Code {
    fn drop(&mut self) {
        self.0.set(true);
    }
}
fn value(input: &Message) -> StateReply {
    let result = match input.data.as_ref() {
        [number] if *number <= 9 => Ok(State::new(*number, 1)),
        _ => Err(Error::INVALID),
    };
    Box::pin(ready(result))
}
fn model(released: Rc<Cell<bool>>) -> ModelProvider {
    ModelProvider::new(
        (),
        ModelOps {
            inspect: None,
            unload: None,
            initialize: |(), input| value(&input),
            prepare: |(), _, input| value(&input),
            observe: |(), state, _| {
                let number = state.get::<u8>().ok_or(Error::INVALID)?;
                Ok(Message::new(Contract::new("context.value", 1), &[*number]))
            },
        },
        Contract::new("context.initial", 1),
        Contract::new("context.value", 1),
        Contract::new("context.set", 1),
        Rc::new(Code(released)),
    )
}
fn setup() -> (Runtime, Connection, ResourceId, Rc<Cell<bool>>) {
    let mut runtime = Runtime::new(181, Limits::default()).unwrap();
    runtime.start().unwrap();
    let released = Rc::new(Cell::new(false));
    let model = runtime
        .register_model("schema", model(Rc::clone(&released)))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(model, Permissions::ALL)]).unwrap(),
    );
    (runtime, client, model, released)
}
fn create(
    runtime: &mut Runtime,
    client: &Connection,
    number: u8,
) -> Observation {
    let model = client.resolve("schema").into_inner().unwrap();
    run_with(
        session::create(
            client,
            model,
            &Message::new(Contract::new("context.initial", 1), &[number]),
        ),
        runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap()
}
fn update(
    runtime: &mut Runtime,
    client: &Connection,
    observed: &Observation,
    number: u8,
) -> Result<Observation, RequestError> {
    let input = session::update(
        observed,
        Message::new(Contract::new("context.set", 1), &[number]),
    )
    .map_err(RequestError::Rejected)?;
    run_with(client.write(observed.session, &input), runtime, &Runtime::DRIVER)
        .unwrap()
        .map(|value| session::observation(value).unwrap())
}
fn read(
    runtime: &mut Runtime,
    client: &Connection,
    subject: ResourceId,
) -> Message {
    run_with(
        client.read(
            subject,
            &Message::new(Contract::new("context.value", 1), &[]),
        ),
        runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap()
}

#[test]
fn immutable_snapshots_and_aba_currentness_use_the_public_caller_abi() {
    let (mut runtime, client, _, _) = setup();
    let first = create(&mut runtime, &client, 0);
    assert_eq!(first.value.data.as_ref(), &[0]);
    let second = update(&mut runtime, &client, &first, 1).unwrap();
    let third = update(&mut runtime, &client, &second, 0).unwrap();
    assert_eq!(third.value.data.as_ref(), &[0]);
    assert_ne!(third.snapshot, first.snapshot);
    assert_eq!(
        update(&mut runtime, &client, &first, 2),
        Err(RequestError::Failed(Error::STALE))
    );
    assert_eq!(read(&mut runtime, &client, first.snapshot).data.as_ref(), &[0]);
    assert_eq!(
        read(&mut runtime, &client, second.snapshot).data.as_ref(),
        &[1]
    );
    let fresh =
        session::observation(read(&mut runtime, &client, first.session))
            .unwrap();
    assert_ne!(fresh.snapshot, third.snapshot);
    let final_state = update(&mut runtime, &client, &fresh, 3).unwrap();
    assert_eq!(final_state.value.data.as_ref(), &[3]);
    run_with(
        session::close(&client, first.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn snapshot_survives_session_close_and_schema_code_release() {
    let (mut runtime, client, model, released) = setup();
    let observed = create(&mut runtime, &client, 4);
    assert_eq!(runtime.unregister(model), Err(Error::BUSY));
    run_with(
        session::close(&client, observed.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.unregister(model).unwrap();
    assert!(released.get());
    assert_eq!(
        read(&mut runtime, &client, observed.snapshot).data.as_ref(),
        &[4]
    );
    run_with(
        session::release(&client, observed.snapshot),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        run_with(
            client.read(
                observed.snapshot,
                &Message::new(Contract::new("context.value", 1), &[])
            ),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    runtime.close().unwrap();
}

#[test]
fn admitted_update_keeps_its_precondition_after_snapshot_release() {
    let (mut runtime, client, _, _) = setup();
    let observed = create(&mut runtime, &client, 0);
    let release = client.write(
        observed.snapshot,
        &Message::new(Contract::new("kernel.session.release", 1), &[]),
    );
    let update = client.write(
        observed.session,
        &session::update(
            &observed,
            Message::new(Contract::new("context.set", 1), &[8]),
        )
        .unwrap(),
    );
    run_with(release, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    let changed = session::observation(
        run_with(update, &mut runtime, &Runtime::DRIVER).unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(changed.value.data.as_ref(), &[8]);
    run_with(
        session::close(&client, changed.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn schema_failure_and_foreign_snapshot_preserve_each_session() {
    let (mut runtime, client, _, _) = setup();
    let first = create(&mut runtime, &client, 2);
    let second = create(&mut runtime, &client, 5);
    assert_eq!(
        update(&mut runtime, &client, &first, 255),
        Err(RequestError::Failed(Error::INVALID))
    );
    let forged = Observation {
        session: first.session,
        snapshot: second.snapshot,
        value: second.value.clone(),
    };
    assert_eq!(
        update(&mut runtime, &client, &forged, 7),
        Err(RequestError::Rejected(Error::STALE))
    );
    assert_eq!(read(&mut runtime, &client, first.session).data.as_ref(), &[2]);
    assert_eq!(read(&mut runtime, &client, second.session).data.as_ref(), &[5]);
    let model = client.resolve("schema").into_inner().unwrap();
    assert_eq!(
        run_with(
            session::create(
                &client,
                model,
                &Message::new(Contract::new("context.initial", 2), &[0])
            ),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::UNSUPPORTED))
    );
    run_with(
        session::close(&client, first.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(
        session::close(&client, second.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn shared_context_updates_preserve_separate_snapshot_and_close_owners() {
    let (mut runtime, client, _, _) = setup();
    let original = create(&mut runtime, &client, 1);
    let root = runtime.resource("sessions").unwrap();
    let peer = runtime.connect(&[(root, Permissions::ALL)]).unwrap();
    let root = peer.resolve("sessions").into_inner().unwrap();
    let peer_session = peer
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .resource;
    let observed =
        session::observation(read(&mut runtime, &peer, peer_session)).unwrap();
    let changed = update(&mut runtime, &peer, &observed, 6).unwrap();
    assert_eq!(
        read(&mut runtime, &client, original.session).data.as_ref(),
        &[6]
    );
    assert_eq!(
        run_with(
            session::close(&peer, peer_session),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::DENIED))
    );
    let name = client
        .children(original.session)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == original.snapshot)
        .unwrap()
        .name;
    let foreign = peer.resolve_at(peer_session, &name).into_inner().unwrap();
    assert_eq!(
        run_with(
            session::release(&peer, foreign),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::DENIED))
    );
    run_with(
        session::release(&peer, changed.snapshot),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(
        session::close(&client, original.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn retained_update_result_blocks_close_until_adopted() {
    let (mut runtime, client, _, _) = setup();
    let original = create(&mut runtime, &client, 2);
    let updating = client.write(
        original.session,
        &session::update(
            &original,
            Message::new(Contract::new("context.set", 1), &[4]),
        )
        .unwrap(),
    );
    assert_eq!(
        run_with(
            session::close(&client, original.session),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::BUSY))
    );
    let updated = session::observation(
        run_with(updating, &mut runtime, &Runtime::DRIVER).unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(updated.value.data.as_ref(), &[4]);
    run_with(
        session::close(&client, original.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn models_cannot_shadow_common_session_controls() {
    let mut runtime = Runtime::new(182, Limits::default()).unwrap();
    let mut provider = model(Rc::new(Cell::new(false)));
    provider.create = Contract::new("kernel.session.create", 1);
    let (error, mut provider) =
        runtime.register_model("schema", provider).err().unwrap();
    assert_eq!(error, Error::INVALID);
    provider.create = Contract::new("context.initial", 1);
    provider.edit = Contract::new("kernel.session.close", 1);
    let (error, _) = runtime.register_model("schema", *provider).err().unwrap();
    assert_eq!(error, Error::INVALID);
}

#[test]
fn reconnect_recovers_the_created_session_and_snapshot() {
    let (mut runtime, client, _, _) = setup();
    let model = client.resolve("schema").into_inner().unwrap();
    let initial = Message::new(Contract::new("context.initial", 1), &[7]);
    let mut creating = Box::pin(session::create(&client, model, &initial));
    let mut wake = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(
        std::future::Future::poll(creating.as_mut(), &mut wake).is_pending()
    );
    let next = client.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        run_with(creating, &mut runtime, &Runtime::DRIVER).unwrap()
    else {
        panic!("Session creation must retain its original outcome");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let recovered = run_with(
        reovim_kernel_interface::requests::result(&next, work),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap()
    .unwrap();
    let repeated = run_with(
        reovim_kernel_interface::requests::result(&next, work),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(repeated, recovered); // Same Session/snapshot, no new access.
    next.close(work).into_inner().unwrap();
    let observed = session::observation(recovered).unwrap();
    assert_eq!(observed.value.data.as_ref(), &[7]);
    assert_eq!(
        read(&mut runtime, &next, observed.snapshot).data.as_ref(),
        &[7]
    );
    let changed = update(&mut runtime, &next, &observed, 8).unwrap();
    assert_eq!(changed.session, observed.session);
    assert_eq!(changed.value.data.as_ref(), &[8]);
    run_with(
        session::close(&next, observed.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
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

#[test]
fn snapshot_pressure_preserves_the_current_context() {
    let limits = Limits { resources: 14, ..Limits::default() };
    let mut runtime = Runtime::new(183, limits).unwrap();
    runtime.start().unwrap();
    let schema = runtime
        .register_model("schema", model(Rc::new(Cell::new(false))))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(schema, Permissions::ALL)]).unwrap();
    let observed = create(&mut runtime, &client, 3);
    let mut blockers: Vec<_> = (0..limits.resources)
        .map_while(|index| blocker(&mut runtime, index))
        .collect();
    runtime.unregister(blockers.pop().expect("filled capacity")).unwrap();
    assert_eq!(
        update(&mut runtime, &client, &observed, 5),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    runtime.unregister_contributions(&blockers).unwrap();
    assert_eq!(
        read(&mut runtime, &client, observed.session).data.as_ref(),
        &[3]
    );
    let retry = update(&mut runtime, &client, &observed, 5).unwrap();
    assert_eq!(retry.value.data.as_ref(), &[5]);
    run_with(
        session::close(&client, observed.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn concurrent_close_requests_do_not_block_each_other() {
    let (mut runtime, client, model, released) = setup();
    let observed = create(&mut runtime, &client, 4);
    let input = Message::new(Contract::new("kernel.session.close", 1), &[]);
    let first = client.write(observed.session, &input);
    let second = client.write(observed.session, &input);
    run_with(first, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert_eq!(runtime.unregister(model), Err(Error::BUSY));
    run_with(second, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    runtime.unregister(model).unwrap();
    assert!(released.get());
    assert_eq!(
        read(&mut runtime, &client, observed.snapshot).data.as_ref(),
        &[4]
    );
    runtime.close().unwrap();
}

#[test]
fn unsupported_context_references_are_rejected_without_discarding_them() {
    let (mut runtime, client, _, _) = setup();
    let observed = create(&mut runtime, &client, 1);
    let proposed =
        Message::reference(Contract::new("context.set", 1), observed.snapshot);
    assert_eq!(session::update(&observed, proposed), Err(Error::UNSUPPORTED));
    let model = client.resolve("schema").into_inner().unwrap();
    let initial = Message::reference(
        Contract::new("context.initial", 1),
        observed.snapshot,
    );
    assert_eq!(
        run_with(
            session::create(&client, model, &initial),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::UNSUPPORTED))
    );
    assert_eq!(
        read(&mut runtime, &client, observed.session).data.as_ref(),
        &[1]
    );
    run_with(
        session::close(&client, observed.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn session_services_have_separate_names_and_block_premature_close() {
    let (mut runtime, client, _, _) = setup();
    let first = create(&mut runtime, &client, 1);
    let second = create(&mut runtime, &client, 2);
    let root = client.resolve("sessions").into_inner().unwrap();
    let entries = client.children(root).into_inner().unwrap();
    let name = |id| {
        entries.iter().find(|entry| entry.resource == id).unwrap().name.clone()
    };
    let first_path =
        format!("sessions/{}/modules/service", name(first.session));
    let second_path =
        format!("sessions/{}/modules/service", name(second.session));
    let one = runtime
        .register(&first_path, reovim_resource_local::memory(b"first"))
        .map_err(|(error, _)| error)
        .unwrap();
    let two = runtime
        .register(&second_path, reovim_resource_local::memory(b"second"))
        .map_err(|(error, _)| error)
        .unwrap();
    for (session, expected) in [
        (first.session, b"first".as_slice()),
        (second.session, b"second".as_slice()),
    ] {
        let service =
            client.resolve_at(session, "modules/service").into_inner().unwrap();
        let reply = run_with(
            client.read(
                service,
                &Message::new(Contract::new("resource.bytes", 1), &[]),
            ),
            &mut runtime,
            &Runtime::DRIVER,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reply.data.as_ref(), expected);
    }
    assert_eq!(
        run_with(
            session::close(&client, first.session),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::BUSY))
    );
    runtime.unregister(one).unwrap();
    run_with(
        session::close(&client, first.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    let error = runtime
        .register(&first_path, reovim_resource_local::memory(b"closed"))
        .err()
        .unwrap()
        .0;
    assert_eq!(error, Error::INVALID);
    assert_eq!(read(&mut runtime, &client, first.snapshot).data.as_ref(), &[1]);
    assert_eq!(read(&mut runtime, &client, second.session).data.as_ref(), &[2]);
    runtime.unregister(two).unwrap();
    run_with(
        session::close(&client, second.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}
