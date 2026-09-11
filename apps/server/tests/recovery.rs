use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    provider::Permissions, requests,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::Cell, future::Future, rc::Rc};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
fn counter() -> (Runtime, Connection, ResourceId, Rc<Cell<u64>>) {
    let mut runtime =
        Runtime::new(101, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let provider =
        reovim_kernel_interface_public_consumer::counter(Rc::clone(&value));
    let id = runtime
        .register("counter", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let id = finish(&mut runtime, client.resolve("counter")).unwrap();
    (runtime, client, id, value)
}
fn locator(error: RequestError) -> Box<str> {
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("expected retained recovery evidence");
    };
    recovery
}
#[test]
fn recovery_distinguishes_original_failure_from_retrieval_failure() {
    let (mut runtime, client, id, value) = counter();
    let input =
        Message::new(Contract::new("external.counter", 1), b"malformed");
    let waiting = client.write(id, &input);
    let next = client.reconnect().unwrap();
    let recovery = locator(finish(&mut runtime, waiting).unwrap_err());
    let work = finish(&mut runtime, next.resolve(&recovery)).unwrap();
    assert_eq!(
        finish(&mut runtime, requests::result(&next, work)),
        Ok(Err(RequestError::Failed(Error::INVALID)))
    );
    assert_eq!(value.get(), 0);
    assert_eq!(
        finish(&mut runtime, requests::result(&next, work)),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    runtime.close().unwrap();
}
#[test]
fn cancelled_queued_work_is_recovered_without_executing_the_effect() {
    let (mut runtime, client, id, value) = counter();
    let input = Message::new(
        Contract::new("external.counter", 1),
        &7_u64.to_le_bytes(),
    );
    let waiting = client.write(id, &input);
    let next = client.reconnect().unwrap();
    let recovery = locator(finish(&mut runtime, waiting).unwrap_err());
    let work = finish(&mut runtime, next.resolve(&recovery)).unwrap();
    finish(
        &mut runtime,
        next.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(
        finish(&mut runtime, requests::result(&next, work)),
        Ok(Err(RequestError::Failed(Error::CANCELLED)))
    );
    assert_eq!(value.get(), 0);
    runtime.close().unwrap();
}
#[test]
fn malformed_recovery_envelope_is_a_retrieval_error() {
    let envelope =
        Message::new(Contract::new("kernel.request.result", 1), &[0]);
    assert_eq!(requests::decode(envelope), Err(Error::INVALID));
}
