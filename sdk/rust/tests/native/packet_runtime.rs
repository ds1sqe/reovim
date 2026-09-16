use super::*;

fn native(source: &str) -> (Rc<Library>, ResourceProvider) {
    let library = Library::compile_source(source);
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    (library, provider)
}

fn input(data: &[u8]) -> Value {
    let mut bytes = 77_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(data);
    Value::copy(&bytes, &[]).unwrap()
}

fn send(caller: &Caller, opened: AccessId, input: Value) -> Call {
    call(caller, Operation::Send, opened, input)
}

#[test]
fn caller_sends_declared_packets_with_write_admission_and_exact_retry() {
    let (library, provider) = native("packet.c");
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    let denied = send(&caller, opened, input(&[0xff, 0]));
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    assert_eq!(library.count(c"fixture_sent"), 0);
    flags(&caller, opened, cx);
    let invalid = send(&caller, opened, input(&[0; 9]));
    assert_eq!(caller.submit(invalid.clone()), Err(Error::INVALID));
    caller.release(invalid.request).unwrap();
    assert_eq!(library.count(c"fixture_sent"), 0);
    for data in [&[][..], &[0xff, 0][..]] {
        let request = send(&caller, opened, input(data));
        caller.submit(request.clone()).unwrap();
        runtime.advance(cx).unwrap();
        runtime.advance(cx).unwrap();
        let first = outcome(&caller, request.request, cx);
        let Outcome::Ok(value) = first.as_ref() else { panic!("{first:?}") };
        assert_eq!(number(value), data.len() as u64);
        caller.submit(request.clone()).unwrap();
        assert_eq!(outcome(&caller, request.request, cx), first);
        caller.release(request.request).unwrap();
        runtime.advance(cx).unwrap();
    }
    assert_eq!(library.count(c"fixture_sent"), 2);
    assert_eq!(library.count(c"fixture_sender"), 77);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn uncertain_send_holds_close_and_recovers_original_acceptance() {
    let (library, provider) = native("packet.c");
    let set: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_uncertain") };
    unsafe { set() };
    let (mut runtime, caller, opened) = setup(provider);
    let cx = &mut Context::from_waker(Waker::noop());
    flags(&caller, opened, cx);
    let cancelled = send(&caller, opened, input(b"skip"));
    caller.submit(cancelled.clone()).unwrap();
    assert_eq!(caller.cancel(cancelled.request), Ok(Cancellation::Accepted));
    caller.release(cancelled.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(library.count(c"fixture_sent"), 0);
    let request = send(&caller, opened, input(b"sent"));
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let unknown = outcome(&caller, request.request, cx);
    assert!(matches!(unknown.as_ref(), Outcome::Unknown { .. }));
    let close = call(&caller, Operation::Close, opened, Value::unit());
    caller.submit(close.clone()).unwrap();
    runtime.advance(cx).unwrap();
    assert!(matches!(
        caller.result(close.request, cx).unwrap(),
        Status::Pending
    ));
    caller.submit(request.clone()).unwrap();
    caller.reconcile(request.request).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(&caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("{result:?}") };
    assert_eq!(number(value), 4);
    assert_eq!(library.count(c"fixture_sent"), 1);
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&caller, close.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    caller.release(close.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn malformed_send_evidence_fences_the_original_request() {
    for mode in [1, 2, 3] {
        let (library, provider) = native("packet.c");
        let set: unsafe extern "C" fn(u32) =
            unsafe { library.symbol(c"fixture_fault") };
        unsafe { set(mode) };
        let (mut runtime, caller, opened) = setup(provider);
        let cx = &mut Context::from_waker(Waker::noop());
        flags(&caller, opened, cx);
        let request = send(&caller, opened, input(b"sent"));
        caller.submit(request.clone()).unwrap();
        runtime.advance(cx).unwrap();
        runtime.advance(cx).unwrap();
        assert_eq!(
            outcome(&caller, request.request, cx).as_ref(),
            &Outcome::Unknown { error: Error::PROVIDER, progress: None }
        );
        caller.submit(request.clone()).unwrap();
        assert_eq!(caller.reconcile(request.request), Err(Error::CLOSED));
        assert_eq!(library.count(c"fixture_sent"), 1);
        caller.release(request.request).unwrap();
        assert_eq!(runtime.close(), Err(Error::INCOMPLETE));
        assert_eq!(library.count(c"fixture_destroyed"), 0);
    }
}

#[path = "packet_loans.rs"]
mod references;

#[path = "packet_receive.rs"]
mod receives;
