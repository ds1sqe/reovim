use super::*;
use core::cell::Cell;
use reovim_kernel_interface::{
    Pending,
    call::{Cancellation, Completion, PendingOps},
    declaration::{Declaration, Operation as Declared},
    provider::{ProviderInterface, ResourceOps, ResourceProvider},
};
use std::rc::Rc;

struct Probe {
    caller: Caller,
    declaration: Declaration,
    loan: Cell<AccessId>,
    alias: Cell<AccessId>,
    began: Cell<bool>,
    finish: Cell<bool>,
    retain: bool,
    return_loan: bool,
    captured: Cell<AccessId>,
}

#[test]
fn provider_loans_keep_admitted_access_until_finish() {
    loan_lifecycle(false, false);
}

#[test]
fn explicitly_retained_input_survives_request_and_source_close() {
    loan_lifecycle(true, false);
}

#[test]
fn retention_permission_does_not_make_a_loan_owned_output() {
    loan_lifecycle(true, true);
}

fn loan_lifecycle(retain: bool, return_loan: bool) {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let declaration = provider.interfaces()[0].clone();
    let limits = Limits { handles: 4, ..Limits::default() };
    let mut runtime = Runtime::new([0x47; 16], limits).unwrap();
    let source = runtime
        .register(b"/source", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let imports = runtime.bind(&[]).unwrap();
    let provider_caller = native(imports.caller());
    let probe = Rc::new(Probe {
        caller: provider_caller,
        declaration,
        loan: Cell::new(AccessId(0)),
        alias: Cell::new(AccessId(0)),
        began: Cell::new(false),
        finish: Cell::new(false),
        retain,
        return_loan,
        captured: Cell::new(AccessId(0)),
    });
    let offered = declaration_with_ref(return_loan);
    let provider = ResourceProvider::new(
        probe.clone(),
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: offered.clone(),
            begin: Some(begin),
            create: None,
        }]),
        Rc::new(()),
    )
    .map_err(|(e, _)| e)
    .unwrap();
    let mut foreign = Runtime::new([0x47; 16], limits).unwrap();
    let foreign_binding = foreign.bind(&[]).unwrap();
    let Err((error, provider)) =
        runtime.register(b"/reader", provider, Some(&foreign_binding))
    else {
        panic!("foreign import context must reject")
    };
    assert_eq!(error, Error::DENIED);
    foreign.close().unwrap();
    let target = runtime
        .register(b"/reader", provider, Some(&imports))
        .map_err(|(e, _)| e)
        .unwrap();
    let source_grant = Grant { retain_loans: retain, ..grant(source, 7) };
    let caller =
        runtime.bind(&[source_grant, grant(target, 1)]).unwrap().caller();
    let caller = native(caller);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let input = acquire(&caller, source, cx);
    let target = acquire(&caller, target, cx);
    set(&caller, input, 1);
    set(&caller, target, 1);
    let target = success(
        &caller,
        call(&caller, Operation::Bind, target, bytes(offered.bytes())),
        cx,
    )
    .references[0];
    let mut request = call(
        &caller,
        Operation::Ioctl,
        target,
        Value::copy(&0_u32.to_le_bytes(), &[input]).unwrap(),
    );
    request.command = 1;
    if !return_loan {
        cancellation_releases_capacity(&caller, &mut runtime, &request, cx);
    }
    assert!(!probe.began.get());
    finish_loan(&caller, &mut runtime, &probe, input, &request, return_loan);
    if !return_loan {
        assert_eq!(library.count(c"fixture_destroyed"), 1);
    }
}

fn finish_loan(
    caller: &Caller,
    runtime: &mut Runtime,
    probe: &Probe,
    input: AccessId,
    request: &Call,
    return_loan: bool,
) {
    let cx = &mut Context::from_waker(Waker::noop());
    let mut invalid = request.clone();
    invalid.input.references[0] = AccessId(999);
    assert_eq!(caller.submit(invalid), Err(Error::DENIED));
    caller.submit(request.clone()).unwrap();
    caller.submit(request.clone()).unwrap();
    set(caller, input, 0);
    let closing = call(caller, Operation::Close, input, Value::unit());
    caller.submit(closing.clone()).unwrap();
    runtime.advance(cx).unwrap();
    assert!(probe.began.get());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(caller, request.request, cx);
    if return_loan {
        assert_eq!(
            result.as_ref(),
            &Outcome::Unknown { error: Error::PROVIDER, progress: None }
        );
        assert_eq!(runtime.close(), Err(Error::INCOMPLETE));
        return;
    }
    let Outcome::Ok(value) = result.as_ref() else { panic!("read failed") };
    assert_eq!(number(value), 41);
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(caller.snapshot(closing.request), Ok(Status::Pending));
    set(&probe.caller, probe.loan.get(), 1);
    probe.finish.set(true);
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(caller, closing.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    denied(&probe.caller, Operation::SetAccess, probe.loan.get(), 0);
    denied(&probe.caller, Operation::SetAccess, probe.alias.get(), 0);
    caller.release(closing.request).unwrap();
    if probe.retain {
        retained_read(probe, runtime, cx);
    }
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

fn declaration_with_ref(return_loan: bool) -> Declaration {
    let mut data = vec![0x72; 16];
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.push(2);
    data.extend_from_slice(&1_u32.to_le_bytes());
    data.extend_from_slice(&[0, 1, 0, 0, 0, 1]);
    data.extend_from_slice(&4_u16.to_le_bytes());
    data.extend_from_slice(b"read");
    data.push(12);
    data.extend_from_slice(&[0x71; 16]);
    data.extend_from_slice(&1_u32.to_le_bytes());
    if return_loan {
        data.push(12);
        data.extend_from_slice(&[0x71; 16]);
        data.extend_from_slice(&1_u32.to_le_bytes());
    } else {
        data.push(4);
    }
    data.push(0);
    Declaration::decode(&data).unwrap()
}

fn native(caller: Caller) -> Caller {
    let exported = Rc::new(ExportedCaller::new(caller, LIMITS, 8).unwrap());
    unsafe { reovim_sdk::import_caller(exported.raw(), exported, LIMITS, 64) }
        .unwrap()
}

fn begin(
    probe: &mut Rc<Probe>,
    _: usize,
    _: Declared,
    input: &Value,
) -> Result<Pending, Error> {
    assert!(!probe.began.replace(true));
    let loan = input.references[0];
    probe.loan.set(loan);
    denied(&probe.caller, Operation::SetAccess, loan, 7);
    if !probe.retain {
        denied(&probe.caller, Operation::Capture, loan, 0);
    }
    let cx = &mut Context::from_waker(Waker::noop());
    let alias = success(
        &probe.caller,
        call(
            &probe.caller,
            Operation::Bind,
            loan,
            bytes(probe.declaration.bytes()),
        ),
        cx,
    )
    .references[0];
    probe.alias.set(alias);
    capture(probe, alias, cx);
    let mut read = call(&probe.caller, Operation::Ioctl, alias, Value::unit());
    read.command = 1;
    probe.caller.submit(read.clone())?;
    Ok(Pending::new(
        (probe.clone(), read.request),
        PendingOps {
            poll,
            take,
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |(probe, _)| {
                if probe.finish.get() { Ok(()) } else { Err(Error::BUSY) }
            },
        },
    ))
}

fn poll(
    (probe, id): &mut (Rc<Probe>, u64),
    cx: &mut Context<'_>,
) -> Result<Poll<()>, Error> {
    match probe.caller.result(*id, cx)? {
        Status::Outcome(_) => Ok(Poll::Ready(())),
        _ => Ok(Poll::Pending),
    }
}

fn take((probe, id): &mut (Rc<Probe>, u64)) -> Result<Completion, Error> {
    let Status::Outcome(outcome) = probe.caller.snapshot(*id)? else {
        return Err(Error::NOT_READY);
    };
    probe.caller.release(*id)?;
    let outcome = if probe.return_loan {
        Outcome::Ok(Value::copy(&0_u32.to_le_bytes(), &[probe.alias.get()])?)
    } else {
        outcome.as_ref().clone()
    };
    Ok(Completion { outcome, activity: 1, owner: None })
}

fn set(caller: &Caller, handle: AccessId, rights: u32) {
    success(
        caller,
        call(
            caller,
            Operation::SetAccess,
            handle,
            Value::copy(&rights.to_le_bytes(), &[]).unwrap(),
        ),
        &mut Context::from_waker(Waker::noop()),
    );
}

fn denied(
    caller: &Caller,
    operation: Operation,
    handle: AccessId,
    rights: u32,
) {
    let request = call(
        caller,
        operation,
        handle,
        Value::copy(&rights.to_le_bytes(), &[]).unwrap(),
    );
    assert_eq!(caller.submit(request.clone()), Err(Error::DENIED));
    caller.release(request.request).unwrap();
}

fn cancellation_releases_capacity(
    caller: &Caller,
    runtime: &mut Runtime,
    template: &Call,
    cx: &mut Context<'_>,
) {
    let mut admitted = Vec::new();
    for _ in 0..4 {
        let mut call = template.clone();
        call.request = caller.reserve().unwrap();
        caller.submit(call.clone()).unwrap();
        admitted.push(call.request);
    }
    let mut rejected = template.clone();
    rejected.request = caller.reserve().unwrap();
    assert_eq!(caller.submit(rejected.clone()), Err(Error::PRESSURE));
    assert_eq!(caller.snapshot(rejected.request), Ok(Status::Reserved));
    for id in &admitted {
        assert_eq!(caller.cancel(*id), Ok(Cancellation::Accepted));
    }
    runtime.advance(cx).unwrap();
    for id in admitted {
        assert_eq!(
            outcome(caller, id, cx).as_ref(),
            &Outcome::Failed(Error::CANCELLED)
        );
        caller.release(id).unwrap();
    }
    caller.submit(rejected.clone()).unwrap();
    assert_eq!(caller.cancel(rejected.request), Ok(Cancellation::Accepted));
    caller.release(rejected.request).unwrap();
    runtime.advance(cx).unwrap();
}

fn capture(probe: &Probe, alias: AccessId, cx: &mut Context<'_>) {
    if !probe.retain || probe.return_loan {
        return;
    }
    let value = success(
        &probe.caller,
        call(
            &probe.caller,
            Operation::Capture,
            alias,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    probe.captured.set(value.references[0]);
    denied(&probe.caller, Operation::SetAccess, value.references[0], 7);
}

fn retained_read(probe: &Probe, runtime: &mut Runtime, cx: &mut Context<'_>) {
    let mut read = call(
        &probe.caller,
        Operation::Ioctl,
        probe.captured.get(),
        Value::unit(),
    );
    read.command = 1;
    probe.caller.submit(read.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(&probe.caller, read.request, cx);
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("retained read failed")
    };
    assert_eq!(number(value), 41);
    probe.caller.release(read.request).unwrap();
    runtime.advance(cx).unwrap();
}
