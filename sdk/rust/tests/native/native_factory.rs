use super::*;
use reovim_kernel_interface::abi;
use std::rc::Rc;

struct Harness {
    runtime: Runtime,
    caller: Caller,
    library: Rc<Library>,
    bound: AccessId,
    imports: Option<Caller>,
}

impl Harness {
    fn new() -> Self {
        Self::capacity(2, false)
    }

    fn capacity(resources: usize, auxiliary: bool) -> Self {
        Self::source(resources, auxiliary, "factory.c")
    }

    fn source(resources: usize, auxiliary: bool, source: &str) -> Self {
        let library = Library::compile_source(source);
        let raw = library.create();
        let provider = unsafe {
            reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
        }
        .unwrap();
        let declaration = provider.interfaces()[0].clone();
        let mut runtime =
            Runtime::new([0x60; 16], Limits { resources, ..Limits::default() })
                .unwrap();
        let imports = auxiliary.then(|| runtime.bind(&[]).unwrap());
        let id = runtime
            .register(b"/factory", provider, imports.as_ref())
            .map_err(|(error, _)| error)
            .unwrap();
        let caller = runtime
            .bind(&[Grant { create_access: 1, ..grant(id, 7) }])
            .unwrap()
            .caller();
        let export = Rc::new(ExportedCaller::new(caller, LIMITS, 8).unwrap());
        let caller = unsafe {
            reovim_sdk::import_caller(export.raw(), export, LIMITS, 64)
        }
        .unwrap();
        runtime.start().unwrap();
        let cx = &mut Context::from_waker(Waker::noop());
        let opened = acquire(&caller, id, cx);
        success(
            &caller,
            call(
                &caller,
                Operation::SetAccess,
                opened,
                Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
            ),
            cx,
        );
        let bound = success(
            &caller,
            call(&caller, Operation::Bind, opened, bytes(declaration.bytes())),
            cx,
        )
        .references[0];
        Self {
            runtime,
            caller,
            library,
            bound,
            imports: imports.map(|imports| imports.caller()),
        }
    }

    fn make(&self, mode: u32, exec: bool) -> Call {
        let mut request = call(
            &self.caller,
            if exec { Operation::Exec } else { Operation::Ioctl },
            self.bound,
            Value::copy(&mode.to_le_bytes(), &[]).unwrap(),
        );
        request.command = if exec { 0 } else { 7 };
        request
    }

    fn cleanup_ready(&self, ready: bool) {
        let set: unsafe extern "C" fn(u32) =
            unsafe { self.library.symbol(c"fixture_cleanup_ready") };
        unsafe { set(u32::from(ready)) };
    }
}

#[test]
fn native_exec_factory_retains_staging_through_recovery_and_cleanup() {
    factory_recovery(true);
}

#[test]
fn native_ioctl_factory_uses_its_declared_nonzero_command() {
    factory_recovery(false);
}

fn factory_recovery(exec: bool) {
    let mut host = Harness::new();
    let cx = &mut Context::from_waker(Waker::noop());
    let cancelled = host.make(0, exec);
    host.caller.submit(cancelled.clone()).unwrap();
    host.caller.cancel(cancelled.request).unwrap();
    host.caller.release(cancelled.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 0);
    let request = host.make(1, exec);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let unknown = outcome(&host.caller, request.request, cx);
    let Outcome::Unknown { progress: Some(progress), .. } = unknown.as_ref()
    else {
        panic!("expected factory progress")
    };
    assert_eq!(number(progress), 0xcafe_5678);
    let close = call(&host.caller, Operation::Close, host.bound, Value::unit());
    host.caller.submit(close.clone()).unwrap();
    host.caller.submit(request.clone()).unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 1);
    host.caller.reconcile(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    let result = outcome(&host.caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("creation failed") };
    assert_eq!(&value.data[..4], &0_u32.to_le_bytes());
    assert_eq!(&value.data[4..], &0xcafe_5678_u64.to_le_bytes());
    let created = value.references[0];
    host.caller.adopt(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&host.caller, close.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    host.caller.release(close.request).unwrap();
    assert_eq!(host.library.count(c"fixture_late_rejected"), 1);
    assert_eq!(host.library.count(c"fixture_rejected_destroyed"), 3);
    read_child(&mut host, created, cx);
    host.cleanup_ready(false);
    let close = call(&host.caller, Operation::Close, created, Value::unit());
    host.caller.submit(close.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.caller.release(close.request).unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    host.cleanup_ready(true);
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 1);
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_factory_destroyed"), 1);
    assert!(matches!(unknown.as_ref(), Outcome::Unknown { .. }));
    let code = Rc::downgrade(&host.library);
    drop(host);
    assert!(code.upgrade().is_none());
}

fn read_child(host: &mut Harness, child: AccessId, cx: &mut Context<'_>) {
    assert_eq!(read_value(host, child, cx), 0xfeed_1234);
}

fn read_value(
    host: &mut Harness,
    child: AccessId,
    cx: &mut Context<'_>,
) -> u64 {
    let denied = call(
        &host.caller,
        Operation::SetAccess,
        child,
        Value::copy(&2_u32.to_le_bytes(), &[]).unwrap(),
    );
    assert_eq!(host.caller.submit(denied.clone()), Err(Error::DENIED));
    host.caller.release(denied.request).unwrap();
    let bound = success(
        &host.caller,
        call(
            &host.caller,
            Operation::Bind,
            child,
            bytes(crate::factory_child_declaration().bytes()),
        ),
        cx,
    )
    .references[0];
    let mut read = call(&host.caller, Operation::Ioctl, bound, Value::unit());
    read.command = 1;
    host.caller.submit(read.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let result = outcome(&host.caller, read.request, cx);
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("child read failed")
    };
    let result = number(value);
    host.caller.release(read.request).unwrap();
    host.runtime.advance(cx).unwrap();
    result
}

#[test]
fn native_failed_factory_keeps_cleanup_and_export_alive_while_busy() {
    let mut host = Harness::new();
    let cx = &mut Context::from_waker(Waker::noop());
    let request = host.make(3, true);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&host.caller, request.request, cx).as_ref(),
        &Outcome::Failed(Error::UNAVAILABLE)
    );
    host.cleanup_ready(false);
    host.caller.release(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.runtime.close(), Err(Error::BUSY));
    assert_eq!(host.library.count(c"fixture_finished"), 0);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    host.cleanup_ready(true);
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_late_rejected"), 1);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 1);
    assert_eq!(host.library.count(c"fixture_factory_destroyed"), 1);
}

#[test]
fn native_factory_without_primary_returns_provider_failure() {
    let mut host = Harness::new();
    let cx = &mut Context::from_waker(Waker::noop());
    let request = host.make(2, true);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&host.caller, request.request, cx).as_ref(),
        &Outcome::Failed(Error::PROVIDER)
    );
    host.caller.release(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    assert_eq!(host.library.count(c"fixture_late_rejected"), 1);
    assert_eq!(host.library.count(c"fixture_factory_destroyed"), 1);
}

#[test]
fn malformed_native_factory_result_quarantines_its_original_owner() {
    malformed_result(4);
}

#[test]
fn failed_native_take_keeps_unexpected_output_owner_quarantined() {
    malformed_result(5);
}

fn malformed_result(mode: u32) {
    let mut host = Harness::new();
    let cx = &mut Context::from_waker(Waker::noop());
    let request = host.make(mode, true);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&host.caller, request.request, cx).as_ref(),
        &Outcome::Unknown { error: Error::PROVIDER, progress: None }
    );
    assert_eq!(host.library.count(c"fixture_released"), 0);
    host.caller.release(request.request).unwrap();
    assert_eq!(host.runtime.close(), Err(Error::INCOMPLETE));
    assert_eq!(host.library.count(c"fixture_finished"), 0);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    let code = Rc::downgrade(&host.library);
    drop(host);
    assert!(code.upgrade().is_some());
}

#[test]
fn native_factory_table_rejection_leaves_context_with_supplier() {
    let library = Library::compile_source("factory.c");
    let raw = library.create();
    let interface = unsafe { *raw.interfaces };
    let mut table = unsafe { *interface.factory };
    table.create = None;
    let broken = abi::Interface { factory: &raw const table, ..interface };
    let provider = abi::Provider { interfaces: &raw const broken, ..raw };
    assert_eq!(
        unsafe {
            reovim_sdk::import_provider(
                &raw const provider,
                library.clone(),
                LIMITS,
            )
        }
        .err(),
        Some(Error::INVALID)
    );
    assert_eq!(library.count(c"fixture_factory_destroyed"), 0);
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    assert_eq!(library.count(c"fixture_created"), 0);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_factory_destroyed"), 1);
}

#[path = "auxiliary.rs"]
mod auxiliary;

#[path = "factory_open.rs"]
mod opened;
