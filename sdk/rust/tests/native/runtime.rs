use core::{future::Future, task::Poll};
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Call, Caller, Operation, Status},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, Limits, Runtime};
use reovim_sdk::{ExportedCaller, Request};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Waker},
};

use super::{LIMITS, Library, Signal, grant, number};

#[path = "info.rs"]
pub mod info;

#[path = "discovery.rs"]
pub mod discovery;

#[path = "native_module.rs"]
mod native_module;

#[path = "modules.rs"]
mod modules;

#[path = "io.rs"]
mod io;

#[path = "capture.rs"]
mod capture;

#[path = "paths.rs"]
mod paths;

#[path = "loans.rs"]
mod loans;

#[path = "native_factory.rs"]
mod native_factory;

#[path = "factory.rs"]
mod factory;

#[path = "outputs.rs"]
mod outputs;

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

fn outcome(
    caller: &Caller,
    id: u64,
    cx: &mut Context<'_>,
) -> std::rc::Rc<Outcome> {
    let Status::Outcome(outcome) = caller.result(id, cx).unwrap() else {
        panic!("expected retained outcome")
    };
    outcome
}

fn success(caller: &Caller, request: Call, cx: &mut Context<'_>) -> Value {
    let id = request.request;
    caller.submit(request).unwrap();
    let result = outcome(caller, id, cx);
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("expected successful common operation")
    };
    caller.adopt(id).unwrap();
    caller.adopt(id).unwrap();
    caller.release(id).unwrap();
    value.clone()
}

fn bytes(data: &[u8]) -> Value {
    let mut encoded = u32::try_from(data.len()).unwrap().to_le_bytes().to_vec();
    encoded.extend_from_slice(data);
    Value::copy(&encoded, &[]).unwrap()
}

#[test]
fn runtime_recovers_native_effect_without_executing_a_retry() {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let mut runtime = Runtime::new([0x38; 16], Limits::default()).unwrap();
    let identity = runtime
        .register(b"/counter", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    runtime.start().unwrap();
    let waker = Waker::from(Arc::new(Signal(AtomicBool::new(false))));
    let cx = &mut Context::from_waker(&waker);
    let handle = acquire(&caller, identity, cx);
    let declaration = discovery::first(&caller, handle, cx);
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, handle, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    let mut request = call(
        &caller,
        Operation::Exec,
        bound,
        Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
    );
    assert_eq!(caller.submit(request.clone()), Err(Error::DENIED));
    assert_eq!(caller.result(request.request, cx), Ok(Status::Reserved));
    success(
        &caller,
        call(
            &caller,
            Operation::SetAccess,
            handle,
            Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    caller.submit(request.clone()).unwrap();
    assert_eq!(caller.result(request.request, cx), Ok(Status::Pending));
    let waiters = register_waiters(&caller, request.request);
    let read: unsafe extern "C" fn(
        *const reovim_kernel_interface::abi::Provider,
    ) -> u64 = unsafe { library.symbol(c"fixture_value") };
    assert_eq!(unsafe { read(&raw const raw) }, 41);
    runtime.advance(cx).unwrap();
    assert_eq!(unsafe { read(&raw const raw) }, 46);
    runtime.advance(cx).unwrap();
    let uncertain = outcome(&caller, request.request, cx);
    assert!(waiters.iter().all(|signal| signal.0.load(Ordering::Relaxed)));
    assert!(matches!(uncertain.as_ref(), Outcome::Unknown { .. }));
    assert_eq!(caller.adopt(request.request), Err(Error::CONFLICT));
    caller.submit(request.clone()).unwrap();
    request.input.data[0] = 99;
    assert_eq!(caller.submit(request.clone()), Err(Error::CONFLICT));
    read_control(&mut runtime, &caller, bound, cx);
    let closing = call(&caller, Operation::Close, handle, Value::unit());
    caller.submit(closing.clone()).unwrap();
    let rejected = call(
        &caller,
        Operation::Exec,
        bound,
        Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
    );
    assert_eq!(caller.submit(rejected.clone()), Err(Error::CLOSED));
    caller.release(rejected.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(caller.result(closing.request, cx), Ok(Status::Pending));
    caller.reconcile(request.request).unwrap();
    assert_eq!(caller.result(request.request, cx), Ok(Status::Pending));
    runtime.advance(cx).unwrap();
    let settled = outcome(&caller, request.request, cx);
    let Outcome::Ok(value) = settled.as_ref() else { panic!("expected OK") };
    assert_eq!(number(value), 46);
    assert!(matches!(uncertain.as_ref(), Outcome::Unknown { .. }));
    caller.adopt(request.request).unwrap();
    caller.release(request.request).unwrap();
    assert_eq!(caller.submit(request), Err(Error::CLOSED));
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&caller, closing.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    caller.release(closing.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_released"), 3);
    assert_eq!(library.count(c"fixture_finished"), 2);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn read_control(
    runtime: &mut Runtime,
    caller: &Caller,
    bound: AccessId,
    cx: &mut Context<'_>,
) {
    let mut read = call(caller, Operation::Ioctl, bound, Value::unit());
    read.command = 1;
    caller.submit(read.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(caller, read.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("expected read") };
    assert_eq!(number(value), 46);
    caller.release(read.request).unwrap();
    runtime.advance(cx).unwrap();
}

fn acquire(
    caller: &Caller,
    identity: ResourceId,
    cx: &mut Context<'_>,
) -> AccessId {
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&identity.epoch);
    locator.extend_from_slice(&identity.serial.to_le_bytes());
    let opened = success(
        caller,
        call(
            caller,
            Operation::Acquire,
            AccessId(0),
            Value::copy(&locator, &[]).unwrap(),
        ),
        cx,
    );
    assert_eq!(&opened.data[24..28], &0_u32.to_le_bytes());
    opened.references[0]
}

fn register_waiters(caller: &Caller, request: u64) -> [Arc<Signal>; 2] {
    core::array::from_fn(|_| {
        let signal = Arc::new(Signal(AtomicBool::new(false)));
        let wake = Waker::from(signal.clone());
        assert_eq!(
            caller.result(request, &mut Context::from_waker(&wake)),
            Ok(Status::Pending)
        );
        signal
    })
}

#[test]
fn sdk_dropped_wait_recovers_the_original_native_request() {
    sdk_request_recovery(false);
}

#[test]
fn sdk_imported_caller_recovers_through_the_native_boundary() {
    sdk_request_recovery(true);
}

fn sdk_request_recovery(native: bool) {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let declaration = provider.interfaces()[0].clone();
    let limits = Limits { waiters: 1, ..Limits::default() };
    let mut runtime = Runtime::new([0x39; 16], limits).unwrap();
    let identity = runtime
        .register(b"/counter", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    let (caller, owner) = bridge(caller, native);
    runtime.start().unwrap();
    let waker = Waker::from(Arc::new(Signal(AtomicBool::new(false))));
    let cx = &mut Context::from_waker(&waker);
    let handle = acquire(&caller, identity, cx);
    success(
        &caller,
        call(
            &caller,
            Operation::SetAccess,
            handle,
            Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, handle, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    let request = Request::reserve(
        &caller,
        Operation::Exec,
        bound,
        0,
        None,
        Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
    )
    .unwrap();
    cancel_before_begin(&caller, &mut runtime, bound, cx);
    let mut reserved = Box::pin(request.outcome());
    assert_eq!(reserved.as_mut().poll(cx), Poll::Ready(Err(Error::NOT_READY)));
    drop(reserved);
    request.submit().unwrap();
    request.submit().unwrap();
    let mut waiting = Box::pin(request.outcome());
    assert_eq!(waiting.as_mut().poll(cx), Poll::Pending);
    assert_eq!(caller.snapshot(request.id()), Ok(Status::Pending));
    assert_eq!(waiting.as_mut().poll(cx), Poll::Pending);
    assert_eq!(waiting.as_mut().poll(cx), Poll::Pending);
    drop(waiting);
    let id = request.id();
    drop(request);
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let recovered = Request::recover(&caller, id).unwrap();
    assert_eq!(recovered.submit(), Err(Error::INVALID));
    let uncertain = ready_outcome(&recovered, cx);
    assert!(matches!(uncertain.as_ref(), Outcome::Unknown { .. }));
    recovered.reconcile().unwrap();
    runtime.advance(cx).unwrap();
    let result = ready_outcome(&recovered, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("expected OK") };
    assert_eq!(number(value), 46);
    assert!(matches!(uncertain.as_ref(), Outcome::Unknown { .. }));
    recovered.adopt().unwrap();
    recovered.release().unwrap();
    assert_eq!(recovered.status(cx), Err(Error::CLOSED));
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_finished"), 1);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    drop(recovered);
    drop(caller);
    if let Some(owner) = owner {
        assert!(owner.upgrade().is_none());
    }
}

fn bridge(
    caller: Caller,
    native: bool,
) -> (Caller, Option<std::rc::Weak<ExportedCaller>>) {
    if !native {
        return (caller, None);
    }
    let exported =
        std::rc::Rc::new(ExportedCaller::new(caller, LIMITS, 1).unwrap());
    let weak = std::rc::Rc::downgrade(&exported);
    let imported = unsafe {
        reovim_sdk::import_caller(exported.raw(), exported, LIMITS, 64)
    }
    .unwrap();
    (imported, Some(weak))
}

fn ready_outcome(
    request: &Request,
    cx: &mut Context<'_>,
) -> std::rc::Rc<Outcome> {
    let mut waiting = Box::pin(request.outcome());
    let Poll::Ready(Ok(outcome)) = waiting.as_mut().poll(cx) else {
        panic!("expected ready outcome")
    };
    outcome
}

fn cancel_before_begin(
    caller: &Caller,
    runtime: &mut Runtime,
    bound: AccessId,
    cx: &mut Context<'_>,
) {
    let cancelled = Request::reserve(
        caller,
        Operation::Exec,
        bound,
        0,
        None,
        Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
    )
    .unwrap();
    cancelled.submit().unwrap();
    assert_eq!(
        cancelled.cancel(),
        Ok(reovim_kernel_interface::call::Cancellation::Accepted)
    );
    runtime.advance(cx).unwrap();
    assert_eq!(
        ready_outcome(&cancelled, cx).as_ref(),
        &Outcome::Failed(Error::CANCELLED)
    );
    cancelled.release().unwrap();
}

type Invoke = unsafe extern "C" fn(
    *const reovim_kernel_interface::abi::Caller,
    u64,
    u64,
    u32,
    *mut reovim_kernel_interface::abi::Pending,
    *mut reovim_kernel_interface::abi::Error,
) -> u32;

#[test]
fn independent_c_caller_retains_execution_after_delivery_finish() {
    use reovim_kernel_interface::abi;
    let library = Library::compile();
    let provider = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(
            &raw const provider,
            library.clone(),
            LIMITS,
        )
    }
    .unwrap();
    let declaration = provider.interfaces()[0].clone();
    let limits = Limits { waiters: 1, ..Limits::default() };
    let mut runtime = Runtime::new([0x41; 16], limits).unwrap();
    let identity = runtime
        .register(b"/counter", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let handle = acquire(&caller, identity, cx);
    success(
        &caller,
        call(
            &caller,
            Operation::SetAccess,
            handle,
            Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, handle, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    let exported = ExportedCaller::new(caller.clone(), LIMITS, 1).unwrap();
    let raw = exported.raw();
    let ops = unsafe { *raw.ops };
    let mut id = 0;
    assert_eq!(unsafe { ops.reserve.unwrap()(raw.context, &raw mut id) }, 0);
    let invoke: Invoke = unsafe { library.symbol(c"fixture_invoke") };
    let mut view =
        abi::Pending { context: core::ptr::null_mut(), ops: core::ptr::null() };
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            invoke(
                &raw const raw,
                id,
                bound.0,
                1,
                &raw mut view,
                &raw mut error,
            )
        },
        Error::INVALID.code.0
    );
    assert!(view.context.is_null());
    assert_eq!(caller.snapshot(id), Ok(Status::Reserved));
    assert_eq!(
        unsafe {
            invoke(
                &raw const raw,
                id,
                bound.0,
                0,
                &raw mut view,
                &raw mut error,
            )
        },
        0
    );
    reject_view_pressure(&caller, &raw, invoke, bound);
    let first = unsafe { *view.ops };
    assert_eq!(unsafe { first.finish.unwrap()(view.context) }, 0);
    assert_eq!(caller.snapshot(id), Ok(Status::Pending));
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        unsafe {
            ops.result.unwrap()(raw.context, id, &raw mut view, &raw mut error)
        },
        0
    );
    refine_native_view(&library, &mut runtime, &view, cx);
    assert_eq!(unsafe { ops.adopt.unwrap()(raw.context, id) }, 0);
    assert_eq!(unsafe { ops.release.unwrap()(raw.context, id) }, 0);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_wake_refs"), 0);
    assert!(library.count(c"fixture_wakes") > 0);
    assert_eq!(library.count(c"fixture_finished"), 1);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn reject_view_pressure(
    caller: &Caller,
    raw: &reovim_kernel_interface::abi::Caller,
    invoke: Invoke,
    bound: AccessId,
) {
    use reovim_kernel_interface::abi;
    let second = caller.reserve().unwrap();
    let mut view =
        abi::Pending { context: core::ptr::null_mut(), ops: core::ptr::null() };
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            invoke(raw, second, bound.0, 0, &raw mut view, &raw mut error)
        },
        Error::PRESSURE.code.0
    );
    assert!(view.context.is_null());
    assert_eq!(caller.snapshot(second), Ok(Status::Reserved));
    caller.release(second).unwrap();
}

fn refine_native_view(
    library: &Library,
    runtime: &mut Runtime,
    view: &reovim_kernel_interface::abi::Pending,
    cx: &mut Context<'_>,
) {
    use reovim_kernel_interface::abi;
    let ops = unsafe { *view.ops };
    let signal: unsafe extern "C" fn() -> abi::Wake =
        unsafe { library.symbol(c"fixture_signal") };
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal()) }, 1);
    let mut result = core::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { ops.take.unwrap()(view.context, result.as_mut_ptr()) },
        0
    );
    let unknown = unsafe { result.assume_init() };
    assert_eq!(unknown.status, 2);
    assert_eq!(unknown.error.code, 90001);
    assert_eq!(
        unsafe { ops.finish.unwrap()(view.context) },
        Error::BUSY.code.0
    );
    unsafe { unknown.release.unwrap()(unknown.owner) };
    assert_eq!(unsafe { ops.reconcile.unwrap()(view.context) }, 0);
    let passive = abi::Wake {
        context: core::ptr::null_mut(),
        wake: None,
        retain: None,
        release: None,
    };
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, passive) }, 0);
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal()) }, 0);
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal()) }, 0);
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal()) }, 0);
    runtime.advance(cx).unwrap();
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal()) }, 1);
    let mut result = core::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe { ops.take.unwrap()(view.context, result.as_mut_ptr()) },
        0
    );
    let final_value = unsafe { result.assume_init() };
    assert_eq!(final_value.status, 0);
    let bytes = unsafe {
        core::slice::from_raw_parts(
            final_value.value.data.ptr,
            final_value.value.data.len,
        )
    };
    assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), 46);
    unsafe { final_value.release.unwrap()(final_value.owner) };
    assert_eq!(unsafe { ops.finish.unwrap()(view.context) }, 0);
}

#[test]
fn invalid_safe_caller_error_never_reports_native_success() {
    use reovim_kernel_interface::caller::CallerOps;
    let caller = Caller::new(
        (),
        CallerOps {
            reserve: |()| Err(Error::new(0)),
            submit: |(), _| Err(Error::new(0)),
            result: |(), _, _| Err(Error::new(0)),
            cancel: |(), _| Err(Error::new(0)),
            reconcile: |(), _| Err(Error::new(0)),
            adopt: |(), _| Err(Error::new(0)),
            release: |(), _| Err(Error::new(0)),
        },
    );
    let exported = ExportedCaller::new(caller, LIMITS, 1).unwrap();
    let raw = exported.raw();
    let ops = unsafe { *raw.ops };
    let mut id = 91;
    assert_eq!(
        unsafe { ops.reserve.unwrap()(raw.context, &raw mut id) },
        Error::PROVIDER.code.0
    );
    assert_eq!(id, 91);
}

#[test]
fn imported_lookup_errors_do_not_consume_reservation_capacity() {
    let mut runtime = Runtime::new([0x42; 16], Limits::default()).unwrap();
    let origin = runtime.bind(&[]).unwrap().caller();
    runtime.start().unwrap();
    let exported = std::rc::Rc::new(
        ExportedCaller::new(origin.clone(), LIMITS, 1).unwrap(),
    );
    let imported = unsafe {
        reovim_sdk::import_caller(exported.raw(), exported, LIMITS, 1)
    }
    .unwrap();
    assert_eq!(imported.snapshot(0), Err(Error::INVALID));
    assert_eq!(imported.snapshot(99), Err(Error::CLOSED));
    let first = imported.reserve().unwrap();
    assert_eq!(first, 1);
    assert_eq!(imported.reserve(), Err(Error::PRESSURE));
    let second = origin.reserve().unwrap();
    assert_eq!(second, 2);
    imported.release(first).unwrap();
    origin.release(second).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
