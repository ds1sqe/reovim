#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    buffer::{ModelOps, ModelProvider, State},
    notify::Next,
    provider::Permissions,
    session::{self, Observation, Watching},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    future::{Future, ready},
    rc::Rc,
    task::{Context, Waker},
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |cx| runtime.advance(cx).map(|_| ())).unwrap()
}
fn setup()
-> (Runtime, Connection, reovim_kernel_interface::ResourceId, Observation) {
    let mut runtime = Runtime::new(214, Limits::default()).unwrap();
    runtime.start().unwrap();
    let model = ModelProvider::new(
        (),
        ModelOps {
            unload: None,
            inspect: None,
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
    let model =
        runtime.register_model("model", model).map_err(|(e, _)| e).unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(model, Permissions::ALL)]).unwrap(),
    );
    let id = client.resolve("model").into_inner().unwrap();
    let initial = finish(
        &mut runtime,
        session::create(
            &client,
            id,
            &Message::new(Contract::new("initial", 1), &[0]),
        ),
    )
    .unwrap();
    (runtime, client, model, initial)
}
fn observe(
    runtime: &mut Runtime,
    client: &Connection,
    current: &Observation,
) -> Watching {
    session::watching(
        &finish(runtime, session::observe(client, current.session)).unwrap(),
    )
    .unwrap()
}
fn update(
    runtime: &mut Runtime,
    client: &Connection,
    current: &Observation,
    value: u8,
) -> Observation {
    let input = session::update(
        current,
        Message::new(Contract::new("set", 1), &[value]),
    )
    .unwrap();
    session::observation(
        finish(runtime, client.write(current.session, &input)).unwrap(),
    )
    .unwrap()
}
fn release(runtime: &mut Runtime, client: &Connection, observed: &Watching) {
    finish(runtime, session::release(client, observed.observation.snapshot))
        .unwrap();
    client.close(observed.watch).into_inner().unwrap();
}

#[test]
fn observation_and_subscription_share_a_cut_and_close_wakes_the_reader() {
    let (mut runtime, client, model, initial) = setup();
    let observed = observe(&mut runtime, &client, &initial);
    assert_eq!(observed.observation.value.data.as_ref(), &[0]);
    let changed = update(&mut runtime, &client, &observed.observation, 1);
    let event = session::change(
        &finish(&mut runtime, session::next(&client, observed.watch, false))
            .unwrap(),
    )
    .unwrap();
    let Next::Item(change) = event else {
        panic!("accepted change");
    };
    assert_eq!(change.before, observed.revision);
    let renewed = observe(&mut runtime, &client, &changed);
    assert_eq!(change.after, renewed.revision);
    assert_eq!(renewed.observation.value.data.as_ref(), &[1]);
    let old = finish(
        &mut runtime,
        client.read(
            observed.observation.snapshot,
            &Message::new(Contract::new("focus", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(old.data.as_ref(), &[0]);
    let mut waiting = session::next(&client, observed.watch, true);
    let mut context = Context::from_waker(Waker::noop());
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    let ended =
        session::change(&finish(&mut runtime, waiting).unwrap()).unwrap();
    assert_eq!(ended, Next::End { lost: false, error: None });
    runtime.unregister(model).unwrap();
    release(&mut runtime, &client, &observed);
    release(&mut runtime, &client, &renewed);
    finish(&mut runtime, session::release(&client, initial.snapshot)).unwrap();
    finish(&mut runtime, session::release(&client, changed.snapshot)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn slow_reader_gaps_independently_and_renewal_observes_current_value() {
    let (mut runtime, client, _, mut current) = setup();
    let slow = observe(&mut runtime, &client, &current);
    let fast = observe(&mut runtime, &client, &current);
    let mut revision = fast.revision;
    for value in 0..34 {
        let next = update(&mut runtime, &client, &current, value % 2);
        finish(&mut runtime, session::release(&client, current.snapshot))
            .unwrap();
        current = next;
        let event = session::change(
            &finish(&mut runtime, session::next(&client, fast.watch, false))
                .unwrap(),
        )
        .unwrap();
        let Next::Item(change) = event else {
            panic!("fast reader kept up");
        };
        assert_eq!(change.before, revision);
        revision = change.after;
    }
    assert_eq!(
        session::change(
            &finish(&mut runtime, session::next(&client, slow.watch, false))
                .unwrap()
        )
        .unwrap(),
        Next::Gap
    );
    let renewed = observe(&mut runtime, &client, &current);
    assert_eq!(renewed.revision, revision);
    assert_eq!(renewed.observation.value, current.value);
    finish(&mut runtime, session::close(&client, current.session)).unwrap();
    assert_eq!(
        session::change(
            &finish(&mut runtime, session::next(&client, slow.watch, false))
                .unwrap()
        )
        .unwrap(),
        Next::End { lost: true, error: None }
    );
    for watched in [&slow, &fast, &renewed] {
        release(&mut runtime, &client, watched);
    }
    finish(&mut runtime, session::release(&client, current.snapshot)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn reader_pressure_publishes_nothing_and_close_reclaims_subscription_capacity()
{
    let (mut runtime, client, _, initial) = setup();
    let mut readers = Vec::new();
    for _ in 0..32 {
        readers.push(observe(&mut runtime, &client, &initial));
    }
    let before = client.children(initial.session).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, session::observe(&client, initial.session)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert_eq!(client.children(initial.session).into_inner().unwrap(), before);
    release(&mut runtime, &client, &readers.pop().unwrap());
    let renewed = observe(&mut runtime, &client, &initial);
    assert_eq!(renewed.observation.value, initial.value);
    release(&mut runtime, &client, &renewed);
    for observed in readers {
        release(&mut runtime, &client, &observed);
    }
    finish(&mut runtime, session::close(&client, initial.session)).unwrap();
    finish(&mut runtime, session::release(&client, initial.snapshot)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn observation_cannot_publish_owned_handles_through_a_shared_alias() {
    let (mut runtime, owner, _, initial) = setup();
    let sessions = runtime.resource("sessions").unwrap();
    let foreign = runtime.register_namespace("foreign").unwrap();
    let binding = runtime
        .bind(&[(sessions, Permissions::READ), (foreign, Permissions::READ)])
        .unwrap();
    runtime.mount(&binding, "snapshots", foreign).unwrap();
    let reader = reovim_sdk::local(binding.connection());
    let root = reader.resolve("sessions").into_inner().unwrap();
    let target = reader
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .resource;
    let shared = reader.resolve("snapshots").into_inner().unwrap();
    let before = reader.children(shared).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, session::observe(&reader, target)),
        Err(RequestError::Failed(Error::DENIED))
    );
    assert_eq!(reader.children(shared).into_inner().unwrap(), before);
    finish(&mut runtime, session::close(&owner, initial.session)).unwrap();
    finish(&mut runtime, session::release(&owner, initial.snapshot)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn semantic_and_metadata_slots_fit_before_observation_is_published() {
    let mut runtime =
        Runtime::new(215, Limits { resources: 32, ..Limits::default() })
            .unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("value", memory::provider(b"value"))
        .map_err(|(e, _)| e)
        .unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let mut model = ModelProvider::new(
        binding.connection(),
        ModelOps {
            unload: None,
            inspect: None,
            initialize: |kernel: &mut Connection, input| {
                let id = input.resources[0];
                let result = kernel
                    .capture(id, Permissions::READ)
                    .map(|access| State::new((id, access), 1));
                Box::pin(ready(result))
            },
            prepare: |_, state, _| Box::pin(ready(Ok(state.clone()))),
            observe: |_, state, input| {
                let mut value = Message::new(input.contract, &[]);
                let id = state
                    .get::<(reovim_kernel_interface::ResourceId, Connection)>()
                    .unwrap()
                    .0;
                value.resources = vec![id; 30].into();
                Ok(value)
            },
        },
        Contract::new("initial", 1),
        Contract::new("references", 1),
        Contract::new("set", 1),
        Rc::new(()),
    );
    model.references = Some(Permissions::READ);
    let model = runtime
        .register_bound(
            &binding,
            vec![reovim_kernel_interface::provider::Contribution::Model {
                path: "model".into(),
                provider: model,
            }],
        )
        .map_err(|(e, _)| e)
        .unwrap()[0];
    let client = runtime
        .connect(&[(source, Permissions::READ), (model, Permissions::ALL)])
        .unwrap();
    let id = client.resolve("model").into_inner().unwrap();
    let value = client.resolve("value").into_inner().unwrap();
    let observed = finish(
        &mut runtime,
        session::create(
            &client,
            id,
            &Message::reference(Contract::new("initial", 1), value),
        ),
    )
    .unwrap();
    let before = client.children(observed.session).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, session::observe(&client, observed.session)),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert_eq!(client.children(observed.session).into_inner().unwrap(), before);
    finish(&mut runtime, session::close(&client, observed.session)).unwrap();
    finish(&mut runtime, session::release(&client, observed.snapshot)).unwrap();
    runtime.unregister(model).unwrap();
    runtime.unregister(source).unwrap();
    runtime.close().unwrap();
}
