use super::*;
use reovim_kernel_interface::provider::{Oversize, Receive};

fn receive(
    caller: &Caller,
    opened: AccessId,
    maximum: u64,
    oversize: Oversize,
) -> Call {
    call(
        caller,
        Operation::Receive,
        opened,
        Value::copy(&Receive { maximum, oversize }.encode(), &[]).unwrap(),
    )
}
fn item(
    value: &Value,
    sender: u32,
    data: &[u8],
    truncated: bool,
    original: u64,
) {
    let mut expected = 0_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&sender.to_le_bytes());
    expected
        .extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    expected.extend_from_slice(data);
    expected.push(u8::from(truncated));
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&original.to_le_bytes());
    assert_eq!(value.data.as_ref(), expected);
    assert!(value.references.is_empty());
}
fn result(
    runtime: &mut Runtime,
    caller: &Caller,
    request: Call,
    cx: &mut Context<'_>,
) -> Value {
    let outcome = finish(runtime, caller, request, cx);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    value.clone()
}

#[test]
fn caller_receive_distinguishes_empty_truncated_dropped_and_end() {
    let (library, provider) = native("packet.c");
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    let denied = receive(&caller, opened, 0, Oversize::Reject);
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    flags(&caller, opened, cx);
    let huge = receive(&caller, opened, u64::MAX, Oversize::Reject);
    assert_eq!(caller.submit(huge.clone()), Err(Error::PRESSURE));
    caller.release(huge.request).unwrap();
    let malformed = call(
        &caller,
        Operation::Receive,
        opened,
        Value::copy(&[0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0], &[]).unwrap(),
    );
    assert_eq!(caller.submit(malformed.clone()), Err(Error::INVALID));
    caller.release(malformed.request).unwrap();
    assert_eq!(library.count(c"fixture_received"), 0);

    item(
        &result(
            &mut runtime,
            &caller,
            receive(&caller, opened, 0, Oversize::Reject),
            cx,
        ),
        41,
        &[],
        false,
        0,
    );
    assert_eq!(
        finish(
            &mut runtime,
            &caller,
            receive(&caller, opened, 2, Oversize::Reject),
            cx
        )
        .as_ref(),
        &Outcome::Failed(Error::PRESSURE)
    );
    assert_eq!(library.count(c"fixture_received"), 1);
    item(
        &result(
            &mut runtime,
            &caller,
            receive(&caller, opened, 2, Oversize::Truncate),
            cx,
        ),
        42,
        &[b'a', 0xff],
        true,
        3,
    );
    let dropped = result(
        &mut runtime,
        &caller,
        receive(&caller, opened, 0, Oversize::Discard),
        cx,
    );
    let mut expected = vec![1, 0, 0, 0, 1, 0, 0, 0];
    expected.extend_from_slice(&1_u64.to_le_bytes());
    assert_eq!(dropped.data.as_ref(), expected);
    assert_eq!(
        result(
            &mut runtime,
            &caller,
            receive(&caller, opened, 8, Oversize::Reject),
            cx
        )
        .data
        .as_ref(),
        &[2, 0, 0, 0]
    );
    assert_eq!(library.count(c"fixture_received"), 3);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn waiting_receivers_keep_separate_packets_and_do_not_stall_send() {
    let (library, provider) = native("packet.c");
    let wait: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_wait") };
    let ready: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_ready") };
    unsafe { wait() };
    let (mut runtime, caller, opened) = setup(provider);
    let signal = Arc::new(Signal(AtomicBool::new(false)));
    let waker = Waker::from(signal.clone());
    let cx = &mut Context::from_waker(&waker);
    flags(&caller, opened, cx);
    let first = receive(&caller, opened, 8, Oversize::Reject);
    let second = receive(&caller, opened, 8, Oversize::Reject);
    caller.submit(first.clone()).unwrap();
    caller.submit(second.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert!(matches!(
        caller.result(first.request, cx).unwrap(),
        Status::Pending
    ));
    assert!(matches!(
        caller.result(second.request, cx).unwrap(),
        Status::Pending
    ));
    let accepted =
        result(&mut runtime, &caller, send(&caller, opened, input(b"ok")), cx);
    assert_eq!(number(&accepted), 2);
    let close = call(&caller, Operation::Close, opened, Value::unit());
    caller.submit(close.clone()).unwrap();
    runtime.advance(cx).unwrap();
    assert!(matches!(
        caller.result(close.request, cx).unwrap(),
        Status::Pending
    ));
    signal.0.store(false, Ordering::Relaxed);
    unsafe { ready() };
    assert!(signal.0.load(Ordering::Relaxed));
    runtime.advance(cx).unwrap();
    let left = outcome(&caller, first.request, cx);
    let right = outcome(&caller, second.request, cx);
    let (Outcome::Ok(left), Outcome::Ok(right)) =
        (left.as_ref(), right.as_ref())
    else {
        panic!("receive failed")
    };
    item(left, 41, &[], false, 0);
    item(right, 42, &[b'a', 0xff, b'b'], false, 3);
    caller.submit(first.clone()).unwrap();
    assert_eq!(library.count(c"fixture_received"), 2);
    caller.release(first.request).unwrap();
    caller.release(second.request).unwrap();
    runtime.advance(cx).unwrap();
    caller.release(close.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn malformed_receive_metadata_retains_custody_without_receiving_again() {
    for mode in [4, 5] {
        let (library, provider) = native("packet.c");
        let fault: unsafe extern "C" fn(u32) =
            unsafe { library.symbol(c"fixture_fault") };
        unsafe { fault(mode) };
        let (mut runtime, caller, opened) = setup(provider);
        let cx = &mut Context::from_waker(Waker::noop());
        flags(&caller, opened, cx);
        let request = receive(&caller, opened, 8, Oversize::Reject);
        caller.submit(request.clone()).unwrap();
        runtime.advance(cx).unwrap();
        runtime.advance(cx).unwrap();
        assert_eq!(
            outcome(&caller, request.request, cx).as_ref(),
            &Outcome::Unknown { error: Error::PROVIDER, progress: None }
        );
        caller.submit(request.clone()).unwrap();
        assert_eq!(library.count(c"fixture_received"), 1);
        caller.release(request.request).unwrap();
        assert_eq!(runtime.close(), Err(Error::INCOMPLETE));
    }
}

#[path = "packet_received_refs.rs"]
mod references;

#[test]
fn receive_options_respect_the_canonical_value_node_budget() {
    let (library, provider) = native("packet.c");
    let limits = Limits {
        values: reovim_kernel_interface::declaration::ValueLimits {
            elements: 3,
            ..LIMITS
        },
        ..Limits::default()
    };
    let mut runtime = Runtime::new([0x6b; 16], limits).unwrap();
    let id = runtime
        .register(b"/packets", provider, None)
        .map_err(|(error, _)| error)
        .unwrap();
    let caller = runtime.bind(&[grant(id, 7)]).unwrap().caller();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let opened = acquire(&caller, id, cx);
    flags(&caller, opened, cx);
    let request = receive(&caller, opened, 0, Oversize::Reject);
    assert_eq!(caller.submit(request.clone()), Err(Error::PRESSURE));
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(library.count(c"fixture_received"), 0);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
