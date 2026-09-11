use reovim_host_reference::run;
use reovim_kernel_interface::buffer::{integer, observation};
use reovim_kernel_interface::provider::Permissions;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn counter()
-> (Runtime, Connection, Rc<Cell<u64>>, reovim_kernel_interface::ResourceId) {
    let mut runtime =
        Runtime::new(17, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let provider =
        reovim_kernel_interface_public_consumer::counter(Rc::clone(&value));
    let registration = runtime
        .register("modules/counter/add", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(registration, Permissions::ALL)]).unwrap();
    (runtime, client, value, registration)
}

#[test]
fn independent_provider_is_registered_after_start_and_only_runtime_executes_it()
{
    let (mut runtime, client, value, registration) = counter();
    let resource =
        finish(&mut runtime, client.resolve("modules/counter/add")).unwrap();
    let input = Message::new(
        Contract::new("external.counter", 1),
        &7_u64.to_le_bytes(),
    );
    let mut reply = client.write(resource, &input);
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(reply.as_mut().poll(&mut context), Poll::Pending));
    assert_eq!(value.get(), 0);
    assert_eq!(runtime.unregister(registration), Err(Error::BUSY));
    let result = finish(&mut runtime, &mut reply).unwrap();
    assert_eq!(integer(&result.data), Ok(7));
    assert_eq!(value.get(), 7);
    runtime.unregister(registration).unwrap();
    assert!(
        finish(&mut runtime, client.resolve("modules/counter/add")).is_err()
    );
    runtime.close().unwrap();
}

#[test]
fn dropped_wait_and_reconnect_recover_the_original_effect() {
    let (mut runtime, client, value, _) = counter();
    let resource =
        finish(&mut runtime, client.resolve("modules/counter/add")).unwrap();
    let input = Message::new(
        Contract::new("external.counter", 1),
        &3_u64.to_le_bytes(),
    );
    let pending = client.write(resource, &input);
    let next = client.reconnect().unwrap();
    let unknown = finish(&mut runtime, pending).unwrap_err();
    let RequestError::Unknown { recovery, .. } = unknown else {
        panic!("recovery required");
    };
    let mut context = Context::from_waker(Waker::noop());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert_eq!(value.get(), 3);
    let work = finish(&mut runtime, next.resolve(&recovery)).unwrap();
    let result = finish(
        &mut runtime,
        next.read(
            work,
            &Message::new(Contract::new("kernel.request.result", 1), &[]),
        ),
    )
    .unwrap();
    let result =
        reovim_kernel_interface::requests::decode(result).unwrap().unwrap();
    assert_eq!(integer(&result.data), Ok(3));
    assert_eq!(value.get(), 3);
    assert!(finish(&mut runtime, next.resolve(&recovery)).is_err());
    assert_eq!(
        finish(&mut runtime, next.write(resource, &input)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    runtime.close().unwrap();
}

#[test]
fn unknown_contract_and_foreign_namespace_are_rejected_before_execution() {
    let (mut runtime, client, value, registration) = counter();
    let other = runtime.connect(&[(registration, Permissions::READ)]).unwrap();
    let resource =
        finish(&mut runtime, client.resolve("modules/counter/add")).unwrap();
    let input = Message::new(
        Contract::new("external.counter", 1),
        &1_u64.to_le_bytes(),
    );
    assert_eq!(
        finish(&mut runtime, other.write(resource, &input)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let read_only =
        finish(&mut runtime, other.resolve("modules/counter/add")).unwrap();
    assert_eq!(
        finish(&mut runtime, other.write(read_only, &input)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let wrong = Message::new(Contract::new("external.counter", 2), &input.data);
    assert_eq!(
        finish(&mut runtime, client.write(resource, &wrong)),
        Err(RequestError::Rejected(Error::UNSUPPORTED))
    );
    assert_eq!(value.get(), 0);
    let reply = finish(&mut runtime, client.write(resource, &input)).unwrap();
    assert_eq!(integer(&reply.data), Ok(1));
    runtime.close().unwrap();
}

#[test]
fn two_models_share_the_same_erased_runtime_and_preserve_history() {
    let mut service = reovim_server::assemble().unwrap();
    let edited = finish(
        &mut service.runtime,
        reovim_server::client::heading(&service.client),
    )
    .unwrap();
    assert_eq!(&*edited.value.data, b"# b\n\xffopaque");
    let mut undo = reovim_kernel_interface::buffer::edit(
        edited.state,
        Message::new(Contract::new("kernel.history.undo", 1), &[]),
    );
    finish(&mut service.runtime, service.client.write(edited.history, &undo))
        .unwrap();
    let restored = finish(
        &mut service.runtime,
        service.client.read(
            edited.state.buffer,
            &Message::new(Contract::new("heading.source", 1), &[]),
        ),
    )
    .unwrap();
    let restored = observation(restored).unwrap();
    assert_eq!(&*restored.value.data, b"# a\n\xffopaque");
    assert_ne!(restored.state.revision, edited.state.revision);
    assert_eq!(restored.node, 0);
    undo.contract = Contract::new("kernel.history.redo", 1);
    assert_eq!(
        finish(
            &mut service.runtime,
            service.client.write(edited.history, &undo)
        ),
        Err(RequestError::Failed(Error::STALE))
    );
    let model =
        finish(&mut service.runtime, service.client.resolve("models/keys"))
            .unwrap();
    let mut initial = 9001_u64.to_le_bytes().to_vec();
    initial.push(0);
    let created = finish(
        &mut service.runtime,
        service.client.write(
            model,
            &Message::new(Contract::new("keys.initial", 1), &initial),
        ),
    )
    .unwrap();
    let keyed = observation(created).unwrap();
    let edit = reovim_kernel_interface::buffer::edit(
        keyed.state,
        Message::new(Contract::new("keys.toggle", 1), &9001_u64.to_le_bytes()),
    );
    let toggled = observation(
        finish(
            &mut service.runtime,
            service.client.write(keyed.state.buffer, &edit),
        )
        .unwrap(),
    )
    .unwrap();
    let values = reovim_domain_basic::decode_keys(&toggled.value.data).unwrap();
    assert_eq!(values.0.get(&9001), Some(&true));
    assert_eq!(service.runtime.unregister(service.heading), Err(Error::BUSY));
    service.runtime.close().unwrap();
}
