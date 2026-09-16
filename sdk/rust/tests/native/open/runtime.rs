use super::*;
use reovim_kernel_interface::caller::{Call, Caller, Operation, Status};
use reovim_kernel_interface::{call::Cancellation, resource::AccessId};
use reovim_kernel_runtime::{Grant, Limits, Runtime};

#[path = "recovery.rs"]
mod recovery;

fn call(
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    input: Value,
) -> Call {
    Call {
        request: caller.reserve().unwrap(),
        operation,
        subject,
        command: 0,
        expected: None,
        input,
    }
}

fn installed(limits: Limits) -> (Runtime, Rc<Library>, Caller, ResourceId) {
    installed_with(limits, c"fixture_open_offered")
}

fn installed_with(
    limits: Limits,
    symbol: &core::ffi::CStr,
) -> (Runtime, Rc<Library>, Caller, ResourceId) {
    let library = Library::compile_source("open.c");
    let create: unsafe extern "C" fn(*mut abi::Provider) -> u32 =
        unsafe { library.symbol(symbol) };
    let mut raw = core::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(raw.as_mut_ptr()) }, 0);
    let raw = unsafe { raw.assume_init() };
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let mut runtime = Runtime::new([0x72; 16], limits).unwrap();
    let identity = runtime.register(b"/cursor", provider, None).ok().unwrap();
    let caller = runtime
        .bind(&[Grant {
            resource: identity,
            access: 1,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap()
        .caller();
    runtime.start().unwrap();
    (runtime, library, caller, identity)
}

fn acquire(caller: &Caller, identity: ResourceId) -> Call {
    let mut data = 1_u32.to_le_bytes().to_vec();
    data.extend_from_slice(&identity.epoch);
    data.extend_from_slice(&identity.serial.to_le_bytes());
    call(
        caller,
        Operation::Acquire,
        AccessId(0),
        Value::copy(&data, &[]).unwrap(),
    )
}

fn drive(runtime: &mut Runtime, caller: &Caller, request: u64) -> Rc<Outcome> {
    for _ in 0..32 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
        if let Status::Outcome(outcome) = caller.snapshot(request).unwrap() {
            return outcome;
        }
    }
    panic!("request did not settle");
}

fn completed(runtime: &mut Runtime, caller: &Caller, request: u64) -> Value {
    let outcome = drive(runtime, caller, request);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    let value = value.clone();
    caller.adopt(request).unwrap();
    caller.release(request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    value
}

fn query(runtime: &mut Runtime, caller: &Caller, request: Call) -> Value {
    let id = request.request;
    caller.submit(request).unwrap();
    completed(runtime, caller, id)
}

fn access(
    runtime: &mut Runtime,
    caller: &Caller,
    token: AccessId,
    rights: u32,
) {
    query(
        runtime,
        caller,
        call(
            caller,
            Operation::SetAccess,
            token,
            Value::copy(&rights.to_le_bytes(), &[]).unwrap(),
        ),
    );
}

fn byte(
    runtime: &mut Runtime,
    caller: &Caller,
    token: AccessId,
    offset: Option<u64>,
) -> Value {
    let mut data =
        offset.map_or_else(Vec::new, |offset| offset.to_le_bytes().to_vec());
    data.extend_from_slice(&1_u64.to_le_bytes());
    query(
        runtime,
        caller,
        call(
            caller,
            if offset.is_some() { Operation::ReadAt } else { Operation::Read },
            token,
            Value::copy(&data, &[]).unwrap(),
        ),
    )
}

fn toggle(library: &Library, name: &core::ffi::CStr) {
    let toggle: unsafe extern "C" fn() = unsafe { library.symbol(name) };
    unsafe { toggle() };
}

#[test]
fn runtime_opens_cursors_without_new_identity_or_initial_rights() {
    let (mut runtime, library, caller, identity) = installed(Limits::default());
    let a = query(&mut runtime, &caller, acquire(&caller, identity));
    let b = query(&mut runtime, &caller, acquire(&caller, identity));
    assert_eq!(&a.data[..24], &b.data[..24]);
    let a = a.references[0];
    let b = b.references[0];
    let denied = call(
        &caller,
        Operation::Read,
        a,
        Value::copy(&1_u64.to_le_bytes(), &[]).unwrap(),
    );
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    access(&mut runtime, &caller, a, 1);
    access(&mut runtime, &caller, b, 1);
    assert_byte(&byte(&mut runtime, &caller, a, None), b'a');
    assert_byte(&byte(&mut runtime, &caller, a, Some(0)), b'a');
    assert_byte(&byte(&mut runtime, &caller, b, None), b'a');
    let mut input = 0_u64.to_le_bytes().to_vec();
    input.extend_from_slice(&4096_u64.to_le_bytes());
    let offered = query(
        &mut runtime,
        &caller,
        call(
            &caller,
            Operation::Interfaces,
            a,
            Value::copy(&input, &[]).unwrap(),
        ),
    );
    let length =
        u32::from_le_bytes(offered.data[4..8].try_into().unwrap()) as usize;
    let bound = query(
        &mut runtime,
        &caller,
        call(
            &caller,
            Operation::Bind,
            a,
            Value::copy(&offered.data[4..8 + length], &[]).unwrap(),
        ),
    )
    .references[0];
    let mut position = call(&caller, Operation::Ioctl, bound, Value::unit());
    position.command = 1;
    let position = query(&mut runtime, &caller, position);
    assert_eq!(
        u64::from_le_bytes(position.data.as_ref().try_into().unwrap()),
        1
    );
    let captured = query(
        &mut runtime,
        &caller,
        call(
            &caller,
            Operation::Capture,
            a,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
    )
    .references[0];
    assert_byte(&byte(&mut runtime, &caller, captured, None), b'b');
    access(&mut runtime, &caller, a, 0);
    access(&mut runtime, &caller, a, 1);
    query(
        &mut runtime,
        &caller,
        call(&caller, Operation::Close, a, Value::unit()),
    );
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 0);
    assert_byte(&byte(&mut runtime, &caller, captured, None), b'c');
    query(
        &mut runtime,
        &caller,
        call(&caller, Operation::Close, captured, Value::unit()),
    );
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    assert_byte(&byte(&mut runtime, &caller, b, None), b'b');
    assert_eq!(library.count(c"fixture_opens"), 2);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 2);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn runtime_recovers_the_original_open_and_joins_busy_close() {
    let (mut runtime, library, caller, identity) = installed(Limits::default());
    toggle(&library, c"fixture_open_unknown");
    let acquisition = acquire(&caller, identity);
    caller.submit(acquisition.clone()).unwrap();
    assert!(matches!(
        drive(&mut runtime, &caller, acquisition.request).as_ref(),
        Outcome::Unknown { .. }
    ));
    assert_eq!(caller.adopt(acquisition.request), Err(Error::CONFLICT));
    caller.submit(acquisition.clone()).unwrap();
    caller.reconcile(acquisition.request).unwrap();
    let token =
        completed(&mut runtime, &caller, acquisition.request).references[0];
    access(&mut runtime, &caller, token, 1);
    assert_byte(&byte(&mut runtime, &caller, token, None), b'a');
    toggle(&library, c"fixture_stop_busy");
    let close = call(&caller, Operation::Close, token, Value::unit());
    caller.submit(close.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(caller.snapshot(close.request), Ok(Status::Pending));
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 0);
    toggle(&library, c"fixture_stop_ready");
    completed(&mut runtime, &caller, close.request);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    assert_eq!(library.count(c"fixture_opens"), 1);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn runtime_reserves_before_open_and_cancelled_admission_reuses_capacity() {
    let limits = Limits { callers: 1, handles: 1, ..Limits::default() };
    let (mut runtime, library, caller, identity) = installed(limits);
    let first = acquire(&caller, identity);
    caller.submit(first.clone()).unwrap();
    let second = acquire(&caller, identity);
    assert_eq!(caller.submit(second.clone()), Err(Error::PRESSURE));
    caller.release(second.request).unwrap();
    assert_eq!(caller.cancel(first.request), Ok(Cancellation::Accepted));
    assert_eq!(
        drive(&mut runtime, &caller, first.request).as_ref(),
        &Outcome::Failed(Error::CANCELLED)
    );
    caller.release(first.request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(library.count(c"fixture_opens"), 0);
    let token =
        query(&mut runtime, &caller, acquire(&caller, identity)).references[0];
    access(&mut runtime, &caller, token, 1);
    assert_byte(&byte(&mut runtime, &caller, token, None), b'a');
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
}
