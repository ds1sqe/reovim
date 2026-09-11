use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, buffer::integer,
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

#[allow(unsafe_code)]
fn roundtrip(connection: Connection) -> Connection {
    // Both adapters implement the same ABI; this fixture has no loaded library.
    unsafe { reovim_sdk::import(reovim_sdk::export(connection), Rc::new(())) }
}
fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
struct Signal(AtomicBool);
impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }
}
#[test]
fn caller_abi_preserves_execution_ownership_and_wakes_the_waiter() {
    let mut runtime =
        Runtime::new(81, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let id = runtime
        .register(
            "modules/counter",
            reovim_kernel_interface_public_consumer::counter(Rc::clone(&value)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = roundtrip(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let subject =
        finish(&mut runtime, client.resolve("modules/counter")).unwrap();
    let parent = finish(&mut runtime, client.resolve("modules")).unwrap();
    let children = finish(&mut runtime, client.children(parent)).unwrap();
    assert!(
        children.iter().any(|entry| entry.name.as_ref() == "counter"
            && entry.resource == subject)
    );
    let input = Message::new(
        Contract::new("external.counter", 1),
        &7_u64.to_le_bytes(),
    );
    let mut future = client.write(subject, &input);
    let signal = Arc::new(Signal(AtomicBool::new(false)));
    let waker = Waker::from(Arc::clone(&signal));
    let mut context = Context::from_waker(&waker);
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    assert_eq!(value.get(), 0);
    assert_eq!(runtime.unregister(id), Err(Error::BUSY));
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert!(signal.0.load(Ordering::Acquire));
    let Poll::Ready(Ok(result)) = future.as_mut().poll(&mut context) else {
        panic!("Runtime settled the admitted request");
    };
    assert_eq!(integer(&result.data), Ok(7));
    assert_eq!(value.get(), 7);
    // Keeping a completed caller Future must not retain the provider.
    runtime.unregister(id).unwrap();
    runtime.close().unwrap();
}
#[test]
fn caller_abi_preserves_unknown_recovery_and_reconnect_fencing() {
    let mut runtime =
        Runtime::new(82, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let id = runtime
        .register(
            "counter",
            reovim_kernel_interface_public_consumer::counter(Rc::clone(&value)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = roundtrip(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let subject = finish(&mut runtime, client.resolve("counter")).unwrap();
    let input = Message::new(
        Contract::new("external.counter", 1),
        &3_u64.to_le_bytes(),
    );
    let waiting = client.write(subject, &input);
    let next = client.reconnect().unwrap();
    let RequestError::Unknown { recovery, .. } =
        finish(&mut runtime, waiting).unwrap_err()
    else {
        panic!("reconnect must preserve the recovery path");
    };
    let work = finish(&mut runtime, next.resolve(&recovery)).unwrap();
    let output = finish(
        &mut runtime,
        next.read(
            work,
            &Message::new(Contract::new("kernel.request.result", 1), &[]),
        ),
    )
    .unwrap();
    let output =
        reovim_kernel_interface::requests::decode(output).unwrap().unwrap();
    assert_eq!(integer(&output.data), Ok(3));
    assert_eq!(value.get(), 3);
    assert_eq!(
        finish(&mut runtime, next.write(subject, &input)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    runtime.close().unwrap();
}
#[test]
fn buffer_observation_and_edit_references_cross_the_caller_abi() {
    let mut service = reovim_server::assemble().unwrap();
    let client = roundtrip(service.client);
    let result =
        finish(&mut service.runtime, reovim_server::client::heading(&client))
            .unwrap();
    assert_eq!(result.value.data.as_ref(), b"# b\n\xffopaque");
    assert_eq!(result.state.revision, 1);
    service.runtime.close().unwrap();
}
