use super::*;

#[test]
fn native_capture_has_independent_flags_fence_and_custody() {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let declaration = provider.interfaces()[0].clone();
    let mut runtime = Runtime::new([0x43; 16], Limits::default()).unwrap();
    let identity = runtime
        .register(b"/counter", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    let (caller, _) = bridge(caller, true);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let opened = acquire(&caller, identity, cx);
    set(&caller, opened, 7, cx);
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, opened, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    let capture = call(&caller, Operation::Capture, bound, flags(1));
    caller.submit(capture.clone()).unwrap();
    let result = outcome(&caller, capture.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("capture failed") };
    assert_eq!(value.data.as_ref(), &0_u32.to_le_bytes());
    let captured = value.references[0];
    rejected(&caller, Operation::SetAccess, captured, 1, Error::DENIED);
    caller.submit(capture.clone()).unwrap();
    assert_eq!(outcome(&caller, capture.request, cx), result);
    caller.adopt(capture.request).unwrap();
    caller.adopt(capture.request).unwrap();
    caller.release(capture.request).unwrap();
    rejected(&caller, Operation::SetAccess, captured, 7, Error::DENIED);
    rejected(&caller, Operation::Capture, captured, 4, Error::DENIED);
    rejected(&caller, Operation::Capture, bound, 8, Error::INVALID);
    set(&caller, captured, 0, cx);
    rejected(&caller, Operation::Capture, captured, 1, Error::DENIED);
    set(&caller, captured, 1, cx);
    set(&caller, opened, 0, cx);
    abandoned(&caller, captured, cx);
    let zero = success(
        &caller,
        call(&caller, Operation::Capture, captured, flags(0)),
        cx,
    )
    .references[0];
    rejected(&caller, Operation::SetAccess, zero, 1, Error::DENIED);
    close(&caller, &mut runtime, opened, cx);
    read(&caller, &mut runtime, captured, cx);
    close(&caller, &mut runtime, captured, cx);
    set(&caller, zero, 0, cx);
    close(&caller, &mut runtime, zero, cx);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn flags(rights: u32) -> Value {
    Value::copy(&rights.to_le_bytes(), &[]).unwrap()
}

#[test]
fn capture_pressure_rejects_without_consuming_access_or_receipt() {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let limits = Limits { handles: 2, ..Limits::default() };
    let mut runtime = Runtime::new([0x44; 16], limits).unwrap();
    let identity = runtime
        .register(b"/counter", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    let (caller, _) = bridge(caller, true);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let source = acquire(&caller, identity, cx);
    set(&caller, source, 1, cx);
    let first = call(&caller, Operation::Capture, source, flags(1));
    caller.submit(first.clone()).unwrap();
    let second = call(&caller, Operation::Capture, source, flags(1));
    assert_eq!(caller.submit(second.clone()), Err(Error::PRESSURE));
    assert_eq!(caller.snapshot(second.request), Ok(Status::Reserved));
    caller.release(first.request).unwrap();
    caller.submit(second.clone()).unwrap();
    let result = outcome(&caller, second.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("capture failed") };
    let captured = value.references[0];
    caller.adopt(second.request).unwrap();
    caller.release(second.request).unwrap();
    set(&caller, source, 7, cx);
    rejected(&caller, Operation::SetAccess, captured, 7, Error::DENIED);
    close(&caller, &mut runtime, source, cx);
    set(&caller, captured, 1, cx);
    close(&caller, &mut runtime, captured, cx);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn set(caller: &Caller, handle: AccessId, rights: u32, cx: &mut Context<'_>) {
    success(
        caller,
        call(caller, Operation::SetAccess, handle, flags(rights)),
        cx,
    );
}

fn rejected(
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    rights: u32,
    error: Error,
) {
    let request = call(caller, operation, subject, flags(rights));
    assert_eq!(caller.submit(request.clone()), Err(error));
    assert_eq!(caller.snapshot(request.request), Ok(Status::Reserved));
    caller.release(request.request).unwrap();
}

fn abandoned(caller: &Caller, source: AccessId, cx: &mut Context<'_>) {
    let request = call(caller, Operation::Capture, source, flags(1));
    caller.submit(request.clone()).unwrap();
    let result = outcome(caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("capture failed") };
    let token = value.references[0];
    caller.release(request.request).unwrap();
    rejected(caller, Operation::SetAccess, token, 0, Error::DENIED);
    set(caller, source, 1, cx);
}

fn close(
    caller: &Caller,
    runtime: &mut Runtime,
    handle: AccessId,
    cx: &mut Context<'_>,
) {
    let request = call(caller, Operation::Close, handle, Value::unit());
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(caller, request.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    caller.release(request.request).unwrap();
}

fn read(
    caller: &Caller,
    runtime: &mut Runtime,
    handle: AccessId,
    cx: &mut Context<'_>,
) {
    let mut request = call(caller, Operation::Ioctl, handle, Value::unit());
    request.command = 1;
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("read failed") };
    assert_eq!(number(value), 41);
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
}
