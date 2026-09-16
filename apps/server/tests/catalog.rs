#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Factory, Module},
    provider::{Contribution, Permissions},
    requests,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

const fn advance(state: &mut Lifecycle) -> Progress {
    if matches!(*state, Lifecycle::Closing) {
        *state = Lifecycle::Closed;
        Progress::Stopped
    } else {
        *state = Lifecycle::Ready;
        Progress::Idle
    }
}

fn factory(constructed: Rc<Cell<Option<ResourceId>>>) -> Factory {
    Factory::new(
        constructed,
        |constructed, instance| {
            assert_eq!(constructed.replace(Some(instance)), None);
            Ok(Module::new(
                Lifecycle::Constructed,
                DriverOps {
                    status: |state| *state,
                    advance: |state, _| Ok(advance(state)),
                    close: |state| {
                        *state = Lifecycle::Closing;
                        Ok(())
                    },
                },
                vec![Contribution::Resource {
                    path: "service/value".into(),
                    provider: memory::provider(b"catalog instance"),
                }],
                Rc::new(()),
            ))
        },
        Rc::new(()),
    )
}
fn setup(identity: u64) -> (Runtime, ResourceId, Rc<Cell<Option<ResourceId>>>) {
    let mut runtime = Runtime::new(identity, Limits::default()).unwrap();
    runtime.start().unwrap();
    let constructed = Rc::new(Cell::new(None));
    let catalog = runtime
        .register_factory(
            "modules/catalog/example",
            Contract::new("example", 3),
            factory(Rc::clone(&constructed)),
        )
        .map_err(|(error, _, _)| error)
        .unwrap();
    (runtime, catalog, constructed)
}
fn close(runtime: &mut Runtime) {
    assert_eq!(runtime.close(), Err(Error::BUSY));
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    runtime.close().unwrap();
}

#[test]
fn catalog_is_passive_and_concurrent_activation_joins_one_instance() {
    let (mut runtime, catalog, constructed) = setup(131);
    let client = reovim_sdk::local(
        runtime.connect(&[(catalog, Permissions::ALL)]).unwrap(),
    );
    let local = client.resolve("modules/catalog/example").into_inner().unwrap();
    let info = run_with(
        client.read(
            local,
            &Message::new(Contract::new("kernel.module.info", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(u32::from_le_bytes(info.data[..4].try_into().unwrap()), 3);
    assert_eq!(&info.data[4..], b"example");
    assert_eq!(constructed.get(), None);
    let request = Message::new(Contract::new("kernel.module.activate", 1), &[]);
    let mut first = client.write(local, &request);
    let second = client.write(local, &request);
    let mut wake = Context::from_waker(Waker::noop());
    assert!(first.as_mut().poll(&mut wake).is_pending());
    assert_eq!(constructed.get(), None);
    let first =
        run_with(first, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    let second =
        run_with(second, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert_eq!(first.resources, second.resources);
    let instance = first.resources[0];
    run_with(module::ready(&client, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    let again = run_with(
        module::activate(&client, local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(again, instance);
    assert!(constructed.get().is_some());
    let export = runtime.resource("service/value").unwrap();
    let reader = runtime.connect(&[(export, Permissions::READ)]).unwrap();
    let value = reader.resolve("service/value").into_inner().unwrap();
    let reply = run_with(
        reader.read(
            value,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"catalog instance");
    let owner = constructed.get().unwrap();
    assert_eq!(runtime.close_module(owner), Err(Error::BUSY));
    runtime.advance(&mut wake).unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::CLOSED))
    );
    assert_eq!(constructed.get(), Some(owner));
    runtime.close().unwrap();
}

#[test]
fn read_only_catalog_grant_cannot_activate_the_factory() {
    let (mut runtime, catalog, constructed) = setup(132);
    let client = runtime.connect(&[(catalog, Permissions::READ)]).unwrap();
    let local = client.resolve("modules/catalog/example").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(constructed.get(), None);
    runtime.close().unwrap();
}

#[test]
fn reconnect_recovers_activation_without_constructing_a_second_instance() {
    let (mut runtime, catalog, constructed) = setup(133);
    let client = reovim_sdk::local(
        runtime.connect(&[(catalog, Permissions::ALL)]).unwrap(),
    );
    let local = client.resolve("modules/catalog/example").into_inner().unwrap();
    let waiting = client.write(
        local,
        &Message::new(Contract::new("kernel.module.activate", 1), &[]),
    );
    let next = client.reconnect().unwrap();
    let error =
        run_with(waiting, &mut runtime, &Runtime::DRIVER).unwrap().unwrap_err();
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("accepted activation must remain recoverable");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let result =
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap()
            .unwrap();
    next.close(work).into_inner().unwrap();
    assert_eq!(result.contract, Contract::new("kernel.module.activate", 1));
    let instance = result.resources[0];
    run_with(module::ready(&next, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(constructed.get().is_some());
    close(&mut runtime);
}

#[test]
fn activation_adds_inspection_to_an_existing_write_only_instance_grant() {
    let (mut runtime, catalog, constructed) = setup(134);
    let first = runtime.connect(&[(catalog, Permissions::WRITE)]).unwrap();
    let local = first.resolve("modules/catalog/example").into_inner().unwrap();
    run_with(module::activate(&first, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    let owner = constructed.get().unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[
                (catalog, Permissions::WRITE),
                (owner, Permissions::WRITE),
            ])
            .unwrap(),
    );
    let path = format!("modules/instances/{}", catalog.0);
    let instance = client.resolve(&path).into_inner().unwrap();
    assert_eq!(
        run_with(
            module::ready(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let selected =
        client.resolve("modules/catalog/example").into_inner().unwrap();
    let joined = run_with(
        module::activate(&client, selected),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(joined, instance);
    run_with(module::ready(&client, joined), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert_eq!(constructed.get(), Some(owner));
    close(&mut runtime);
}

#[test]
fn selection_pressure_keeps_the_factory_unconstructed_and_retryable() {
    let limits = Limits { resources: 10, ..Limits::default() };
    let mut runtime = Runtime::new(135, limits).unwrap();
    runtime.start().unwrap();
    let constructed = Rc::new(Cell::new(None));
    let catalog = runtime
        .register_factory(
            "catalog",
            Contract::new("example", 3),
            factory(Rc::clone(&constructed)),
        )
        .map_err(|(error, _, _)| error)
        .unwrap();
    let mut blockers = Vec::new();
    for name in ["a", "b", "c"] {
        blockers.push(
            runtime
                .register(name, memory::provider(b"held"))
                .map_err(|(error, _)| error)
                .unwrap(),
        );
    }
    let client = runtime.connect(&[(catalog, Permissions::ALL)]).unwrap();
    let local = client.resolve("catalog").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert_eq!(constructed.get(), None);
    assert_eq!(runtime.resource("modules"), Err(Error::UNAVAILABLE));
    runtime.unregister_contributions(&blockers).unwrap();
    let instance = run_with(
        module::activate(&client, local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(module::ready(&client, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(constructed.get().is_some());
    close(&mut runtime);
}

#[test]
fn cancelled_activation_does_not_select_or_construct_a_factory() {
    let (mut runtime, catalog, constructed) = setup(136);
    let client = runtime.connect(&[(catalog, Permissions::ALL)]).unwrap();
    let local = client.resolve("modules/catalog/example").into_inner().unwrap();
    let waiting = client.write(
        local,
        &Message::new(Contract::new("kernel.module.activate", 1), &[]),
    );
    let next = client.reconnect().unwrap();
    let error =
        run_with(waiting, &mut runtime, &Runtime::DRIVER).unwrap().unwrap_err();
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("expected retained activation work");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    run_with(
        next.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap(),
        Err(RequestError::Failed(Error::CANCELLED))
    );
    next.close(work).into_inner().unwrap();
    assert_eq!(constructed.get(), None);
    assert_eq!(runtime.resource("modules/instances"), Err(Error::UNAVAILABLE));
    let local = next.resolve("modules/catalog/example").into_inner().unwrap();
    run_with(module::activate(&next, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(constructed.get().is_some());
    close(&mut runtime);
}
