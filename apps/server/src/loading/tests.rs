use super::*;
use core::{
    cell::Cell,
    task::{Poll, Waker},
};
use reovim_kernel_interface::{
    Pending, Value, call::Cancellation, declaration::Operation,
};
use reovim_kernel_runtime::Limits;

fn setup(capacity: usize) -> (Runtime, Loader, Rc<Cell<u32>>) {
    let mut runtime = crate::assemble([0xb1; 16], Limits::default()).unwrap();
    let imports = Rc::new(runtime.bind(&[]).unwrap());
    let recipient = Rc::new(runtime.bind(&[]).unwrap());
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let loader = Loader::new(
        Target {
            registry: ResourceId { epoch: [0xb1; 16], serial: 1 },
            namespace: ResourceId { epoch: [0xb1; 16], serial: 1 },
            imports,
            recipient,
            approved: Box::new([]),
        },
        Rc::new(move |_| {
            observed.set(observed.get() + 1);
            Err(Error::UNAVAILABLE)
        }),
        capacity,
        Limits::default().values,
    )
    .unwrap();
    (runtime, loader, calls)
}
fn input(path: &[u8]) -> Value {
    let mut data = u32::try_from(path.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(path);
    Value::copy(&data, &[]).unwrap()
}
fn outcome(pending: &mut Pending) -> Outcome {
    let cx = &mut Context::from_waker(Waker::noop());
    assert_eq!(pending.poll(cx), Ok(Poll::Ready(())));
    let value = pending.take().unwrap();
    pending.finish().unwrap();
    value.outcome
}
#[test]
fn queue_pressure_and_cancellation_execute_no_artifact_code() {
    let (mut runtime, mut loader, calls) = setup(1);
    let mut provider = loader.provider().unwrap();
    let mut cancelled =
        provider.begin(0, Operation::Exec, &input(b"/one")).unwrap();
    assert!(matches!(
        provider.begin(0, Operation::Exec, &input(b"/two")),
        Err(Error::PRESSURE)
    ));
    assert_eq!(cancelled.finish(), Err(Error::BUSY));
    assert_eq!(cancelled.cancel(), Ok(Cancellation::Accepted));
    let cx = &Context::from_waker(Waker::noop());
    assert_eq!(loader.advance(&mut runtime, cx), Progress::More);
    assert_eq!(outcome(&mut cancelled), Outcome::Failed(Error::CANCELLED));
    assert_eq!(calls.get(), 0);
    let mut next = provider.begin(0, Operation::Exec, &input(b"/two")).unwrap();
    loader.advance(&mut runtime, cx);
    assert_eq!(calls.get(), 1);
    assert_eq!(outcome(&mut next), Outcome::Failed(Error::UNAVAILABLE));
    provider.stop().unwrap();
    runtime.close().unwrap();
}
#[test]
fn dropping_loader_finishes_queued_results_and_fences_retained_provider() {
    let (mut runtime, loader, calls) = setup(2);
    let mut provider = loader.provider().unwrap();
    let mut closed =
        provider.begin(0, Operation::Exec, &input(b"/one")).unwrap();
    let mut cancelled =
        provider.begin(0, Operation::Exec, &input(b"/two")).unwrap();
    cancelled.cancel().unwrap();
    drop(loader);
    assert_eq!(outcome(&mut closed), Outcome::Failed(Error::CLOSED));
    assert_eq!(outcome(&mut cancelled), Outcome::Failed(Error::CANCELLED));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        provider.begin(0, Operation::Exec, &input(b"/three")),
        Err(Error::CLOSED)
    ));
    provider.stop().unwrap();
    runtime.close().unwrap();
}
#[test]
fn invalid_native_paths_reject_before_queuing() {
    let (mut runtime, mut loader, calls) = setup(1);
    let mut provider = loader.provider().unwrap();
    for path in [&b""[..], b"/bad\0path"] {
        assert!(matches!(
            provider.begin(0, Operation::Exec, &input(path)),
            Err(Error::INVALID)
        ));
    }
    assert_eq!(
        loader.advance(&mut runtime, &Context::from_waker(Waker::noop())),
        Progress::Idle
    );
    assert_eq!(calls.get(), 0);
    provider.stop().unwrap();
    runtime.close().unwrap();
}
