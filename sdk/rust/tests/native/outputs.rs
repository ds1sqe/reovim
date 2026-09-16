use super::*;
use core::cell::Cell;
use reovim_kernel_interface::{
    Pending,
    call::{Cancellation, Completion, PendingOps},
    declaration::{Declaration, Operation as Declared},
    provider::{Creation, ProviderInterface, ResourceOps, ResourceProvider},
};
use std::rc::Rc;

struct Provider {
    caller: Caller,
    source: AccessId,
    calls: Cell<u32>,
    released: Cell<bool>,
    moved: Cell<bool>,
    malformed: bool,
    missing_primary: bool,
}

struct Returned {
    provider: Rc<Provider>,
    handle: AccessId,
}

impl Drop for Returned {
    fn drop(&mut self) {
        let caller = &self.provider.caller;
        let request = call(caller, Operation::SetAccess, self.handle, flags(0));
        let result = caller.submit(request.clone());
        self.provider.moved.set(result == Err(Error::DENIED));
        caller.release(request.request).unwrap();
        self.provider.released.set(true);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Delivery {
    Adopt,
    Discard,
    ReleasePending,
    Malformed,
    FactoryAdopt,
    FactoryReject,
}

#[test]
fn rejected_factory_consumes_valid_auxiliary_before_owner_release() {
    returned_access(Delivery::FactoryReject);
}

#[test]
fn factory_primary_and_owned_auxiliary_share_result_custody() {
    returned_access(Delivery::FactoryAdopt);
}

#[test]
fn returned_access_is_moved_once_before_provider_owner_release() {
    returned_access(Delivery::Adopt);
}

#[test]
fn malformed_returned_access_retains_its_cleanup_owner() {
    returned_access(Delivery::Malformed);
}

#[test]
fn discarded_result_cleans_access_without_closing_shared_subject() {
    returned_access(Delivery::Discard);
}

#[test]
fn released_pending_request_still_consumes_provider_output_ownership() {
    returned_access(Delivery::ReleasePending);
}

fn returned_access(delivery: Delivery) {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let declaration = provider.interfaces()[0].clone();
    let factory =
        matches!(delivery, Delivery::FactoryAdopt | Delivery::FactoryReject);
    let limits =
        Limits { handles: if factory { 6 } else { 4 }, ..Limits::default() };
    let mut runtime = Runtime::new([0x48; 16], limits).unwrap();
    let source = runtime
        .register(b"/source", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let imports = runtime.bind(&[grant(source, 1)]).unwrap();
    let imported = native(imports.caller());
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let handle = acquire(&imported, source, cx);
    set(&imported, handle, 1, cx);
    let source = success(
        &imported,
        call(&imported, Operation::Bind, handle, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    let state = Rc::new(Provider {
        caller: imported,
        source,
        calls: Cell::new(0),
        released: Cell::new(false),
        moved: Cell::new(false),
        malformed: delivery == Delivery::Malformed,
        missing_primary: delivery == Delivery::FactoryReject,
    });
    let offered = declaration_with_output(factory);
    let provider = ResourceProvider::new(
        state.clone(),
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: offered.clone(),
            begin: Some(begin),
            create: Some(create_returned),
        }]),
        Rc::new(()),
    )
    .map_err(|(e, _)| e)
    .unwrap();
    let provider = runtime
        .register(b"/access", provider, Some(&imports))
        .map_err(|(e, _)| e)
        .unwrap();
    let access =
        Grant { create_access: u32::from(factory), ..grant(provider, 1) };
    let caller = native(runtime.bind(&[access]).unwrap().caller());
    let handle = acquire(&caller, provider, cx);
    set(&caller, handle, 1, cx);
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, handle, bytes(offered.bytes())),
        cx,
    )
    .references[0];
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    let cancelled = invoke(&caller, bound);
    caller.submit(cancelled.clone()).unwrap();
    let pressure = invoke(&caller, bound);
    assert_eq!(caller.submit(pressure.clone()), Err(Error::PRESSURE));
    assert_eq!(state.calls.get(), 0);
    caller.release(pressure.request).unwrap();
    assert_eq!(caller.cancel(cancelled.request), Ok(Cancellation::Accepted));
    caller.release(cancelled.request).unwrap();
    if delivery == Delivery::ReleasePending {
        caller.release(request.request).unwrap();
    }
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    if delivery == Delivery::Malformed {
        assert!(!state.released.get());
        assert_eq!(
            outcome(&caller, request.request, cx).as_ref(),
            &Outcome::Unknown { error: Error::PROVIDER, progress: None }
        );
        caller.submit(request).unwrap();
        assert_eq!(state.calls.get(), 1);
        assert_eq!(runtime.close(), Err(Error::INCOMPLETE));
        return;
    }
    assert!(state.released.get());
    assert!(state.moved.get());
    assert_eq!(state.calls.get(), 1);
    finish_delivery(&caller, &mut runtime, &request, delivery, cx);
    // The provider's independently owned source access remains usable.
    read(&state.caller, &mut runtime, source, cx);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn finish_delivery(
    caller: &Caller,
    runtime: &mut Runtime,
    request: &Call,
    delivery: Delivery,
    cx: &mut Context<'_>,
) {
    if delivery == Delivery::ReleasePending {
        assert_eq!(caller.snapshot(request.request), Err(Error::CLOSED));
        return;
    }
    let result = outcome(caller, request.request, cx);
    if delivery == Delivery::FactoryReject {
        assert_eq!(result.as_ref(), &Outcome::Failed(Error::PROVIDER));
        caller.release(request.request).unwrap();
        return;
    }
    let Outcome::Ok(value) = result.as_ref() else { panic!("expected access") };
    let returned = value.references[0];
    if delivery == Delivery::FactoryAdopt {
        assert_eq!(value.data.as_ref(), &[1, 0, 0, 0, 0, 0, 0, 0]);
        denied(caller, value.references[1], 0);
    }
    denied(caller, returned, 1);
    caller.submit(request.clone()).unwrap();
    assert_eq!(outcome(caller, request.request, cx), result);
    if matches!(delivery, Delivery::Adopt | Delivery::FactoryAdopt) {
        caller.adopt(request.request).unwrap();
        caller.adopt(request.request).unwrap();
    }
    caller.release(request.request).unwrap();
    if delivery == Delivery::FactoryAdopt {
        set(caller, value.references[1], 1, cx);
    }
    if matches!(delivery, Delivery::Adopt | Delivery::FactoryAdopt) {
        denied(caller, returned, 7);
        read(caller, runtime, returned, cx);
    } else {
        denied(caller, returned, 0);
    }
}

fn begin(
    state: &mut Rc<Provider>,
    _: usize,
    _: Declared,
    _: &Value,
) -> Result<Pending, Error> {
    let request =
        call(&state.caller, Operation::Capture, state.source, flags(1));
    state.caller.submit(request.clone())?;
    let result = outcome(
        &state.caller,
        request.request,
        &mut Context::from_waker(Waker::noop()),
    );
    let Outcome::Ok(value) = result.as_ref() else {
        return Err(Error::PROVIDER);
    };
    state.caller.adopt(request.request)?;
    state.caller.release(request.request)?;
    state.calls.set(state.calls.get() + 1);
    let owner =
        Returned { provider: state.clone(), handle: value.references[0] };
    let mut value = value.clone();
    if state.malformed {
        value.references[0] = AccessId(999);
    }
    let completion = Completion {
        outcome: Outcome::Ok(value),
        activity: 0,
        owner: Some(Box::new(owner)),
    };
    Ok(Pending::new(
        Some(completion),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value: &mut Option<Completion>| {
                value.take().ok_or(Error::CLOSED)
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}

fn create_returned(
    state: &mut Rc<Provider>,
    interface: usize,
    operation: Declared,
    input: &Value,
    creation: &Creation,
) -> Result<Pending, Error> {
    if !state.missing_primary {
        let primary = ResourceProvider::new(
            (),
            ResourceOps {
                open: None,
                namespace: None,
                io: None,
                stop: |()| Ok(()),
            },
            Box::new([]),
            Rc::new(()),
        )
        .map_err(|(error, ())| error)?;
        creation.resource(primary).map_err(|(error, _)| error)?;
    }
    begin(state, interface, operation, input)
}

fn declaration_with_output(factory: bool) -> Declaration {
    let mut data = vec![0x73; 16];
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.push(2);
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.extend_from_slice(&[if factory { 2 } else { 0 }, 1, 0, 0, 0, 0]);
    data.extend_from_slice(&3_u16.to_le_bytes());
    data.extend_from_slice(b"get");
    data.extend_from_slice(&[0, 12]);
    data.extend_from_slice(&[0x71; 16]);
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.push(0);
    if factory {
        data.push(12);
        data.extend_from_slice(&[0; 16]);
        data.extend_from_slice(&1_u32.to_le_bytes());
    }
    Declaration::decode(&data).unwrap()
}

fn native(caller: Caller) -> Caller {
    let owner = Rc::new(ExportedCaller::new(caller, LIMITS, 8).unwrap());
    unsafe { reovim_sdk::import_caller(owner.raw(), owner, LIMITS, 64) }
        .unwrap()
}

fn flags(rights: u32) -> Value {
    Value::copy(&rights.to_le_bytes(), &[]).unwrap()
}

fn set(caller: &Caller, handle: AccessId, rights: u32, cx: &mut Context<'_>) {
    success(
        caller,
        call(caller, Operation::SetAccess, handle, flags(rights)),
        cx,
    );
}

fn denied(caller: &Caller, handle: AccessId, rights: u32) {
    let request = call(caller, Operation::SetAccess, handle, flags(rights));
    assert_eq!(caller.submit(request.clone()), Err(Error::DENIED));
    caller.release(request.request).unwrap();
}

fn invoke(caller: &Caller, handle: AccessId) -> Call {
    let mut request = call(caller, Operation::Ioctl, handle, Value::unit());
    request.command = 1;
    request
}

fn read(
    caller: &Caller,
    runtime: &mut Runtime,
    handle: AccessId,
    cx: &mut Context<'_>,
) {
    let request = invoke(caller, handle);
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("read failed") };
    assert_eq!(number(value), 41);
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
}
