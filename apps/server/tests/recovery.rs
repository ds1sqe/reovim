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
    let mut runtime = Runtime::new(101, Limits::default()).unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let provider = reovim_module_counter::provider(Rc::clone(&value));
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
        Ok(Err(RequestError::Failed(Error::INVALID)))
    );
    next.close(work).into_inner().unwrap();
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
    next.close(work).into_inner().unwrap();
    runtime.close().unwrap();
}
#[test]
fn malformed_recovery_envelope_is_a_retrieval_error() {
    let envelope =
        Message::new(Contract::new("kernel.request.result", 1), &[0]);
    assert_eq!(requests::decode(envelope), Err(Error::INVALID));
}

#[derive(Default)]
struct WakeFlag(std::sync::atomic::AtomicBool);
impl std::task::Wake for WakeFlag {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
impl WakeFlag {
    fn notified(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[test]
fn original_and_independent_recovery_waiters_are_all_notified() {
    use std::{
        sync::Arc,
        task::{Context, Waker},
    };
    let (mut runtime, client, id, value) = counter();
    let mut original = client.write(
        id,
        &Message::new(
            Contract::new("external.counter", 1),
            &7_u64.to_le_bytes(),
        ),
    );
    let root = client.resolve("requests").into_inner().unwrap();
    let entries = client.children(root).into_inner().unwrap();
    let work = entries.first().expect("original work").resource;
    let query = Message::new(Contract::new("kernel.request.result", 1), &[]);
    let mut first = client.read(work, &query);
    let mut second = client.read(work, &query);
    let flags: [Arc<WakeFlag>; 3] = std::array::from_fn(|_| Arc::default());
    for (waiting, flag) in
        [&mut original, &mut first, &mut second].into_iter().zip(&flags)
    {
        let waker = Waker::from(flag.clone());
        assert!(
            waiting
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
    }
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..8 {
        runtime.advance(&mut context).unwrap();
        if flags.iter().all(|flag| flag.notified()) {
            break;
        }
    }
    assert!(flags.iter().all(|flag| flag.notified()));
    assert_eq!(value.get(), 7);
    let original = finish(&mut runtime, original).unwrap();
    assert_eq!(
        client.close(work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let first = requests::decode(finish(&mut runtime, first).unwrap())
        .unwrap()
        .unwrap();
    let second = requests::decode(finish(&mut runtime, second).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(first, original);
    assert_eq!(second, original);
    client.close(work).into_inner().unwrap();
    assert_eq!(value.get(), 7);
    runtime.close().unwrap();
}
