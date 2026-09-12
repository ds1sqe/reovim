use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, ModelOps, ModelProvider},
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    notify::Next,
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};
use reovim_module_text::{
    TEXT,
    api::{self, Unit},
};
use std::{
    future::{Future, ready},
    rc::Rc,
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn text_module(kernel: Connection, path: &str) -> Module {
    let provider = (TEXT.construct)(kernel).remove(0);
    Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |status| *status,
            advance: |status, _| {
                if *status == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *status = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |status| {
                *status = Lifecycle::Closed;
                Ok(())
            },
        },
        vec![Contribution::Resource { path: path.into(), provider }],
        Rc::new(()),
    )
}

fn source() -> ModelProvider {
    use reovim_module_source::SOURCE;
    ModelProvider::new(
        (),
        ModelOps {
            unload: None,
            initialize: |(), input| Box::pin(ready((SOURCE.initialize)(input))),
            prepare: |(), state, input| {
                Box::pin(ready((SOURCE.prepare)(state, input)))
            },
            observe: |(), state, input| (SOURCE.observe)(state, input),
        },
        Contract::new("source.initial", 1),
        Contract::new("source.bytes", 1),
        Contract::new("source.splice", 1),
        Rc::new(()),
    )
}

#[test]
fn public_text_api_qualifies_edits_and_keeps_detached_snapshots_readable() {
    let mut runtime = Runtime::new(205, Limits::default()).unwrap();
    runtime.start().unwrap();
    let model =
        runtime.register_model("source", source()).map_err(|(e, _)| e).unwrap();
    let root = runtime.resource("").unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let binding = runtime.bind_for(&owner, &[]).unwrap();
    let module = text_module(reovim_sdk::local(binding.connection()), "text");
    runtime
        .activate_bound_module(&binding, "instance", module)
        .map_err(|(e, _)| e)
        .unwrap();
    let client = reovim_sdk::local(owner.connection());
    let instance = client.resolve("instance").into_inner().unwrap();
    finish(
        &mut runtime,
        client.read(
            instance,
            &Message::new(Contract::new("kernel.module.ready", 1), &[]),
        ),
    )
    .unwrap();
    let source_id = client.resolve("source").into_inner().unwrap();
    let initial = finish(
        &mut runtime,
        reovim_module_source::api::create(&client, source_id, b"# Draft\n\xff"),
    )
    .unwrap();
    let factory = client.resolve("text").into_inner().unwrap();
    let view = finish(
        &mut runtime,
        api::attach(&client, factory, initial.state.buffer, true),
    )
    .unwrap();
    let first = finish(&mut runtime, api::observe(&client, view)).unwrap();
    let changed =
        finish(&mut runtime, api::replace(&client, &first, 2..7, "한글"))
            .unwrap();
    let current = finish(&mut runtime, api::observe(&client, view)).unwrap();
    let mut reading = Box::pin(api::read(&client, &current, 0, 20));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(reading.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert_eq!(
        client.close(current.snapshot()).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let window = finish(&mut runtime, reading).unwrap();
    client.close(current.snapshot()).into_inner().unwrap();
    let expected: Vec<_> = "# 한글\n"
        .chars()
        .map(Unit::Scalar)
        .chain([Unit::Opaque(255)])
        .collect();
    assert_eq!(window.units, expected);
    assert_eq!(window.observed, current.observed());
    let Next::Item(change) =
        finish(&mut runtime, api::next(&client, &first, false)).unwrap()
    else {
        panic!("accepted semantic change")
    };
    assert_eq!(change.before, first.observed());
    assert_eq!(change.after.buffer, changed.state);
    assert_eq!(change.after.space, first.observed().space);
    client.close(view).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, api::read(&client, &first, 2, 5)).unwrap().units,
        "Draft".chars().map(Unit::Scalar).collect::<Vec<_>>()
    );
    for observed in [first, current] {
        client.close(observed.snapshot()).into_inner().unwrap();
        client.close(observed.watch()).into_inner().unwrap();
    }
    finish(
        &mut runtime,
        buffer::close(
            &client,
            changed.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    runtime.unregister(model).unwrap();
    finish(
        &mut runtime,
        client.write(
            instance,
            &Message::new(Contract::new("kernel.module.unload", 1), &[]),
        ),
    )
    .unwrap();
    runtime.close().unwrap();
}

fn peer(
    runtime: &mut Runtime,
    name: &str,
    shared: ResourceId,
    rights: Permissions,
) -> (Connection, ResourceId) {
    let path = format!("clients/{name}");
    let root = runtime.register_namespace(&path).unwrap();
    let owner = runtime
        .bind_at(root, &[(root, Permissions::ALL), (shared, rights)])
        .unwrap();
    runtime.mount(&owner, "shared", shared).unwrap();
    let binding = runtime.bind_for(&owner, &[]).unwrap();
    let module = text_module(
        reovim_sdk::local(binding.connection()),
        &format!("{path}/text"),
    );
    runtime
        .activate_bound_module(&binding, &format!("{path}/instance"), module)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(owner.connection());
    let instance = client.resolve("instance").into_inner().unwrap();
    finish(
        runtime,
        client.read(
            instance,
            &Message::new(Contract::new("kernel.module.ready", 1), &[]),
        ),
    )
    .unwrap();
    let factory = client.resolve("text").into_inner().unwrap();
    let buffer = client.resolve("shared").into_inner().unwrap();
    let view = finish(
        runtime,
        api::attach(&client, factory, buffer, rights == Permissions::ALL),
    )
    .unwrap();
    (client, view)
}

#[test]
fn separate_client_namespaces_share_source_but_keep_their_own_text_access() {
    let mut runtime = Runtime::new(206, Limits::default()).unwrap();
    runtime.start().unwrap();
    let model =
        runtime.register_model("source", source()).map_err(|(e, _)| e).unwrap();
    let assembly = runtime.bind(&[(model, Permissions::ALL)]).unwrap();
    let admin = assembly.connection();
    let model = admin.resolve("source").into_inner().unwrap();
    let initial = finish(
        &mut runtime,
        reovim_module_source::api::create(&admin, model, b"# Draft\n\xff"),
    )
    .unwrap();
    let buffers = admin.resolve("buffers").into_inner().unwrap();
    let entries = admin.children(buffers).into_inner().unwrap();
    let shared =
        assembly.resource(&format!("buffers/{}", entries[0].name)).unwrap();
    let (writer, edit_view) =
        peer(&mut runtime, "writer", shared, Permissions::ALL);
    let (reader, read_view) =
        peer(&mut runtime, "reader", shared, Permissions::READ);
    let before =
        finish(&mut runtime, api::observe(&writer, edit_view)).unwrap();
    let old_peer =
        finish(&mut runtime, api::observe(&reader, read_view)).unwrap();
    let changed =
        finish(&mut runtime, api::replace(&writer, &before, 2..7, "é"))
            .unwrap();
    let Next::Item(event) =
        finish(&mut runtime, api::next(&reader, &old_peer, false)).unwrap()
    else {
        panic!("peer change")
    };
    assert_eq!(event.before.buffer.revision, initial.state.revision);
    assert_eq!(event.after.buffer.revision, changed.state.revision);
    assert_eq!(event.before.space, old_peer.observed().space);
    assert_eq!(
        finish(&mut runtime, api::read(&reader, &old_peer, 2, 5))
            .unwrap()
            .units,
        "Draft".chars().map(Unit::Scalar).collect::<Vec<_>>()
    );
    let current_peer =
        finish(&mut runtime, api::observe(&reader, read_view)).unwrap();
    let actual =
        finish(&mut runtime, api::read(&reader, &current_peer, 0, 20)).unwrap();
    assert_eq!(
        actual.units,
        "# é\n"
            .chars()
            .map(Unit::Scalar)
            .chain([Unit::Opaque(255)])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        finish(&mut runtime, api::replace(&reader, &current_peer, 0..0, "bad")),
        Err(RequestError::Failed(Error::DENIED))
    );
    writer.close(edit_view).into_inner().unwrap();
    reader.close(read_view).into_inner().unwrap();
    for (connection, observed) in
        [(&writer, before), (&reader, old_peer), (&reader, current_peer)]
    {
        connection.close(observed.snapshot()).into_inner().unwrap();
        connection.close(observed.watch()).into_inner().unwrap();
    }
    let state = buffer::BufferState {
        revision: changed.state.revision,
        ..initial.state
    };
    finish(
        &mut runtime,
        buffer::close(&admin, state, buffer::ClosePolicy::DiscardLocal),
    )
    .unwrap();
    for client in [&writer, &reader] {
        let instance = client.resolve("instance").into_inner().unwrap();
        finish(
            &mut runtime,
            client.write(
                instance,
                &Message::new(Contract::new("kernel.module.unload", 1), &[]),
            ),
        )
        .unwrap();
    }
    runtime.close().unwrap();
}
