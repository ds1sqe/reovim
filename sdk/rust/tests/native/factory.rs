use super::*;
#[path = "creation_export.rs"]
mod creation_export;
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{
    Pending,
    call::{Cancellation, Completion, PendingOps},
    declaration::Declaration,
    provider::{Creation, ProviderInterface, ResourceOps, ResourceProvider},
};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Create,
    Missing,
    Fail,
    Unknown,
}

struct Probe {
    mode: Mode,
    calls: Cell<u32>,
    stopped: Cell<bool>,
    destroyed: Cell<bool>,
    stop_ready: Cell<bool>,
    creation: RefCell<Option<Creation>>,
}

struct Child {
    probe: Rc<Probe>,
    value: u64,
}

impl Drop for Child {
    fn drop(&mut self) {
        assert!(self.probe.stopped.get());
        self.probe.destroyed.set(true);
    }
}

fn declaration(factory: bool) -> Declaration {
    let mut bytes = vec![if factory { 0x73 } else { 0x72 }; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(2); // Ioctl(1).
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.push(if factory { 2 } else { 0 });
    bytes.extend_from_slice(&1_u32.to_le_bytes()); // READ admission.
    bytes.extend_from_slice(&[0, 4, 0]);
    bytes.extend_from_slice(b"read");
    bytes.extend_from_slice(&[0, 4, 0]); // Unit -> U64, Unit progress.
    if factory {
        bytes.push(12);
        bytes.extend_from_slice(&[0x72; 16]);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
    }
    Declaration::decode(&bytes).unwrap()
}

fn completed(outcome: Outcome) -> Pending {
    Pending::new(
        Some(outcome),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |state| {
                Ok(Completion {
                    outcome: state.take().ok_or(Error::CLOSED)?,
                    activity: 0,
                    owner: None,
                })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    )
}

fn child(probe: &Rc<Probe>, value: u64) -> ResourceProvider {
    ResourceProvider::new(
        Child { probe: probe.clone(), value },
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |state| {
                if !state.probe.stop_ready.get() {
                    return Err(Error::BUSY);
                }
                state.probe.stopped.set(true);
                Ok(())
            },
        },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration(false),
            begin: Some(|state, _, _, _| {
                Ok(completed(Outcome::Ok(Value::copy(
                    &state.value.to_le_bytes(),
                    &[],
                )?)))
            }),
            create: None,
        }]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
    .unwrap()
}

fn create(probe: &Rc<Probe>, staging: &Creation) -> Pending {
    probe.calls.set(probe.calls.get() + 1);
    *probe.creation.borrow_mut() = Some(staging.clone());
    if probe.mode != Mode::Missing {
        assert!(staging.resource(child(probe, 91)).is_ok());
    }
    Pending::new(
        (probe.clone(), false),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |(probe, reconciled)| {
                let outcome = match probe.mode {
                    Mode::Fail => Outcome::Failed(Error::UNAVAILABLE),
                    Mode::Unknown if !*reconciled => Outcome::Unknown {
                        error: Error::UNAVAILABLE,
                        progress: None,
                    },
                    _ => Outcome::Ok(Value::copy(&17_u64.to_le_bytes(), &[])?),
                };
                Ok(Completion { outcome, activity: 0, owner: None })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |(_, reconciled)| {
                *reconciled = true;
                Ok(())
            },
            finish: |_| Ok(()),
        },
    )
}

fn factory_provider(probe: &Rc<Probe>) -> ResourceProvider {
    let provider = ResourceProvider::new(
        probe.clone(),
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: declaration(true),
            begin: None,
            create: Some(|probe, _, _, _, staging| Ok(create(probe, staging))),
        }]),
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
    .unwrap();
    let raw = reovim_sdk::ExportedProvider::new(provider, LIMITS)
        .ok()
        .unwrap()
        .into_raw();
    unsafe { reovim_sdk::import_provider(&raw const raw, Rc::new(()), LIMITS) }
        .unwrap()
}

fn setup(
    mode: Mode,
    capacity: usize,
) -> (Runtime, Caller, Rc<Probe>, AccessId) {
    let probe = Rc::new(Probe {
        mode,
        calls: Cell::new(0),
        stopped: Cell::new(false),
        destroyed: Cell::new(false),
        stop_ready: Cell::new(true),
        creation: RefCell::new(None),
    });
    let provider = factory_provider(&probe);
    let mut runtime = Runtime::new(
        [0x59; 16],
        Limits { resources: capacity, ..Limits::default() },
    )
    .unwrap();
    let id = runtime
        .register(b"/factory", provider, None)
        .map_err(|(error, _)| error)
        .unwrap();
    let grants = Grant { create_access: 1, ..grant(id, 1) };
    let caller = runtime.bind(&[grants]).unwrap().caller();
    let exported = Rc::new(ExportedCaller::new(caller, LIMITS, 8).unwrap());
    let caller = unsafe {
        reovim_sdk::import_caller(exported.raw(), exported, LIMITS, 64)
    }
    .unwrap();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    // EXEC/READ rights on the factory cannot replace a creation grant.
    let denied = runtime.bind(&[grant(id, 7)]).unwrap().caller();
    let bound = bind_factory(&denied, id, cx);
    let request = invoke(&denied, bound);
    assert_eq!(denied.submit(request.clone()), Err(Error::DENIED));
    assert_eq!(probe.calls.get(), 0);
    denied.release(request.request).unwrap();
    let bound = bind_factory(&caller, id, cx);
    (runtime, caller, probe, bound)
}

fn bind_factory(
    caller: &Caller,
    id: ResourceId,
    cx: &mut Context<'_>,
) -> AccessId {
    let opened = acquire(caller, id, cx);
    success(
        caller,
        call(
            caller,
            Operation::SetAccess,
            opened,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    success(
        caller,
        call(caller, Operation::Bind, opened, bytes(declaration(true).bytes())),
        cx,
    )
    .references[0]
}

fn invoke(caller: &Caller, subject: AccessId) -> Call {
    let mut request = call(caller, Operation::Ioctl, subject, Value::unit());
    request.command = 1;
    request
}

#[test]
fn factory_publishes_immutable_access_and_reclaims_released_subject() {
    let (mut runtime, caller, probe, bound) = setup(Mode::Create, 2);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    let pressure = invoke(&caller, bound);
    assert_eq!(caller.submit(pressure.clone()), Err(Error::PRESSURE));
    assert_eq!(probe.calls.get(), 0);
    caller.release(pressure.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(&caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("creation failed") };
    assert_eq!(&value.data[..4], &0_u32.to_le_bytes());
    assert_eq!(&value.data[4..], &17_u64.to_le_bytes());
    let created = value.references[0];
    caller.submit(request.clone()).unwrap();
    assert_eq!(probe.calls.get(), 1);
    let read = invoke(&caller, created);
    assert_eq!(caller.submit(read.clone()), Err(Error::DENIED));
    caller.release(read.request).unwrap();
    caller.adopt(request.request).unwrap();
    caller.release(request.request).unwrap();
    read_child(&mut runtime, &caller, created, cx);
    runtime.advance(cx).unwrap();
    assert!(!probe.destroyed.get());
    let close = call(&caller, Operation::Close, created, Value::unit());
    caller.submit(close.clone()).unwrap();
    probe.stop_ready.set(false);
    runtime.advance(cx).unwrap();
    caller.release(close.request).unwrap();
    assert!(!probe.destroyed.get());
    probe.stop_ready.set(true);
    runtime.advance(cx).unwrap();
    assert!(probe.destroyed.get());
    let recycled = invoke(&caller, bound);
    caller.submit(recycled.clone()).unwrap();
    assert_eq!(caller.cancel(recycled.request), Ok(Cancellation::Accepted));
    caller.release(recycled.request).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(probe.calls.get(), 1);
    runtime.close().unwrap();
}

fn read_child(
    runtime: &mut Runtime,
    caller: &Caller,
    created: AccessId,
    cx: &mut Context<'_>,
) {
    let bound = success(
        caller,
        call(
            caller,
            Operation::Bind,
            created,
            bytes(declaration(false).bytes()),
        ),
        cx,
    )
    .references[0];
    let read = invoke(caller, bound);
    caller.submit(read.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(caller, read.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("read failed") };
    assert_eq!(number(value), 91);
    caller.release(read.request).unwrap();
}

#[test]
fn failed_factory_drains_staged_provider_before_reusing_capacity() {
    let (mut runtime, caller, probe, bound) = setup(Mode::Fail, 2);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&caller, request.request, cx).as_ref(),
        &Outcome::Failed(Error::UNAVAILABLE)
    );
    probe.stop_ready.set(false);
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    assert!(!probe.destroyed.get());
    let next = invoke(&caller, bound);
    assert_eq!(caller.submit(next.clone()), Err(Error::PRESSURE));
    probe.stop_ready.set(true);
    runtime.advance(cx).unwrap();
    assert!(probe.destroyed.get());
    caller.submit(next.clone()).unwrap();
    caller.cancel(next.request).unwrap();
    caller.release(next.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

#[test]
fn successful_factory_without_primary_fails_without_publishing() {
    let (mut runtime, caller, _, bound) = setup(Mode::Missing, 2);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&caller, request.request, cx).as_ref(),
        &Outcome::Failed(Error::PROVIDER)
    );
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

#[test]
fn unknown_factory_reconciles_original_staging_after_delivery_release() {
    let (mut runtime, caller, probe, bound) = setup(Mode::Unknown, 2);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let unknown = outcome(&caller, request.request, cx);
    assert!(matches!(unknown.as_ref(), Outcome::Unknown { .. }));
    caller.reconcile(request.request).unwrap();
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(probe.calls.get(), 1);
    assert!(probe.destroyed.get());
    assert!(matches!(unknown.as_ref(), Outcome::Unknown { .. }));
    runtime.close().unwrap();
}

fn inert(interfaces: Box<[Declaration]>) -> ResourceProvider {
    ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        interfaces
            .into_iter()
            .map(|declaration| ProviderInterface {
                prepare: None,
                declaration,
                begin: Some(|(), _, _, _| Err(Error::UNSUPPORTED)),
                create: None,
            })
            .collect(),
        Rc::new(()),
    )
    .map_err(|(error, ())| error)
    .unwrap()
}

#[test]
fn factory_revalidates_declarations_before_installation() {
    let (mut runtime, caller, probe, bound) = setup(Mode::Create, 3);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    let mut changed = declaration(false).bytes().to_vec();
    let name = changed.windows(4).position(|bytes| bytes == b"read").unwrap();
    changed[name..name + 4].copy_from_slice(b"peek");
    runtime
        .register(
            b"/conflicting",
            inert(Box::new([Declaration::decode(&changed).unwrap()])),
            None,
        )
        .map_err(|(error, _)| error)
        .unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&caller, request.request, cx).as_ref(),
        &Outcome::Failed(Error::CONFLICT)
    );
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    assert!(probe.destroyed.get());
    let capability = probe.creation.borrow();
    let (error, mut rejected) = capability
        .as_ref()
        .unwrap()
        .resource(inert(Box::new([])))
        .err()
        .unwrap();
    assert_eq!(error, Error::CLOSED);
    rejected.stop().unwrap();
    runtime.close().unwrap();
}

#[test]
fn staging_rejects_duplicate_and_incompatible_primary_without_consumption() {
    let (mut runtime, caller, probe, bound) = setup(Mode::Missing, 2);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = invoke(&caller, bound);
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    let capability = probe.creation.borrow().as_ref().unwrap().clone();
    let (error, mut rejected) =
        capability.resource(inert(Box::new([]))).err().unwrap();
    assert_eq!(error, Error::UNSUPPORTED);
    rejected.stop().unwrap();
    assert!(capability.resource(child(&probe, 91)).is_ok());
    let (error, mut rejected) =
        capability.resource(inert(Box::new([]))).err().unwrap();
    assert_eq!(error, Error::CONFLICT);
    rejected.stop().unwrap();
    runtime.advance(cx).unwrap();
    assert!(matches!(
        outcome(&caller, request.request, cx).as_ref(),
        Outcome::Ok(_)
    ));
    let (error, mut rejected) =
        capability.resource(inert(Box::new([]))).err().unwrap();
    assert_eq!(error, Error::CLOSED);
    rejected.stop().unwrap();
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    assert!(probe.destroyed.get());
    runtime.close().unwrap();
}

#[test]
fn busy_created_resource_does_not_delay_other_resource_cleanup() {
    let (mut runtime, caller, first, bound) = setup(Mode::Create, 4);
    let second = Rc::new(Probe {
        mode: Mode::Create,
        calls: Cell::new(0),
        stopped: Cell::new(false),
        destroyed: Cell::new(false),
        stop_ready: Cell::new(false),
        creation: RefCell::new(None),
    });
    let id = runtime
        .register(b"/second", factory_provider(&second), None)
        .map_err(|(error, _)| error)
        .unwrap();
    let other = runtime
        .bind(&[Grant { create_access: 1, ..grant(id, 1) }])
        .unwrap()
        .caller();
    let cx = &mut Context::from_waker(Waker::noop());
    let other_bound = bind_factory(&other, id, cx);
    let first_call = invoke(&caller, bound);
    let second_call = invoke(&other, other_bound);
    caller.submit(first_call.clone()).unwrap();
    other.submit(second_call.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    caller.release(first_call.request).unwrap();
    other.release(second_call.request).unwrap();
    runtime.advance(cx).unwrap();
    assert!(first.destroyed.get());
    assert!(!second.destroyed.get());
    second.stop_ready.set(true);
    runtime.advance(cx).unwrap();
    assert!(second.destroyed.get());
    runtime.close().unwrap();
}
