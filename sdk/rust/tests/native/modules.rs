use super::*;
use reovim_kernel_interface::{
    driver::{DriverOps, Lifecycle, Progress},
    module::{ModuleContext, ModuleImplementation, ModuleScope},
    provider::{ResourceOps, ResourceProvider},
};
use reovim_kernel_runtime::Activation;
use std::{cell::Cell, rc::Rc};

struct Probe {
    fail: Cell<bool>,
    close_ready: Cell<bool>,
    destroyed: Cell<bool>,
}

struct Imports {
    library: Rc<Library>,
    probe: Rc<Probe>,
    omit_second: bool,
}

struct ModuleState {
    context: ModuleContext<Imports>,
    lifecycle: Lifecycle,
    staged: bool,
}

impl Drop for ModuleState {
    fn drop(&mut self) {
        assert_eq!(self.lifecycle, Lifecycle::Closed);
        self.context.imports.probe.destroyed.set(true);
    }
}

#[allow(
    clippy::result_large_err,
    reason = "inert rejection returns context intact without allocating"
)]
fn implementation() -> ModuleImplementation<ModuleState, Imports> {
    ModuleImplementation {
        construct: |context| {
            assert_ne!(context.instance, context.namespace);
            assert_eq!(context.scope, ModuleScope::Global);
            let (error, mut rejected) =
                context.exports.stage(b"early", inert()).err().unwrap();
            assert_eq!(error, Error::NOT_READY);
            rejected.stop().unwrap();
            Ok(ModuleState {
                context,
                lifecycle: Lifecycle::Constructed,
                staged: false,
            })
        },
        driver: DriverOps {
            status: |state| state.lifecycle,
            advance,
            close: |state| {
                state.lifecycle = Lifecycle::Closing;
                if !state.context.imports.probe.close_ready.get() {
                    return Err(Error::BUSY);
                }
                state.lifecycle = Lifecycle::Closed;
                Ok(())
            },
        },
    }
}

fn advance(
    state: &mut ModuleState,
    _: &mut Context<'_>,
) -> Result<Progress, Error> {
    if state.lifecycle == Lifecycle::Closing {
        return Ok(Progress::Idle);
    }
    if state.context.imports.probe.fail.get() {
        return Err(Error::UNAVAILABLE);
    }
    if !state.staged {
        stage(state, b"first")?;
        state.staged = true;
        state.lifecycle = Lifecycle::Starting;
        return Ok(Progress::More);
    }
    if state.lifecycle == Lifecycle::Starting {
        if !state.context.imports.omit_second {
            stage(state, b"second")?;
        }
        state.lifecycle = Lifecycle::Ready;
    }
    Ok(Progress::Idle)
}

fn stage(state: &ModuleState, path: &[u8]) -> Result<(), Error> {
    let library = &state.context.imports.library;
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }?;
    state.context.exports.stage(path, provider).map_err(|(error, mut p)| {
        p.stop().unwrap();
        error
    })
}

fn inert() -> ResourceProvider {
    ResourceProvider::new(
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
    .ok()
    .unwrap()
}

fn setup(omit_second: bool) -> (Runtime, ResourceId, Rc<Library>, Rc<Probe>) {
    let mut runtime = Runtime::new([0x78; 16], Limits::default()).unwrap();
    let namespace = runtime.register(b"/module", inert(), None).ok().unwrap();
    let binding = runtime.bind(&[]).unwrap();
    runtime.start().unwrap();
    let library = Library::compile();
    let probe = Rc::new(Probe {
        fail: Cell::new(false),
        close_ready: Cell::new(true),
        destroyed: Cell::new(false),
    });
    let instance = runtime
        .activate(
            Activation {
                namespace,
                scope: ModuleScope::Global,
                required: &[b"first", b"second"],
                exports: 2,
            },
            implementation(),
            Imports {
                library: library.clone(),
                probe: probe.clone(),
                omit_second,
            },
            &binding,
            library.clone(),
        )
        .unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Constructed));
    (runtime, instance, library, probe)
}

#[test]
fn runtime_drives_module_then_discovers_and_calls_its_published_provider() {
    let (mut runtime, instance, library, probe) = setup(false);
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Starting));
    assert_eq!(runtime.module_exports(instance), Err(Error::NOT_READY));
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Ready));
    let exports = runtime.module_exports(instance).unwrap().to_vec();
    let caller = runtime.bind(&[grant(exports[0], 7)]).unwrap().caller();
    let handle = acquire(&caller, exports[0], cx);
    let declaration = discovery::first(&caller, handle, cx);
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, handle, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    success(
        &caller,
        call(
            &caller,
            Operation::SetAccess,
            handle,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    let mut read = call(&caller, Operation::Ioctl, bound, Value::unit());
    read.command = 1;
    caller.submit(read.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(&caller, read.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("missing result") };
    assert_eq!(number(value), 41);
    caller.adopt(read.request).unwrap();
    caller.release(read.request).unwrap();
    drop(result);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert!(probe.destroyed.get());
    assert_eq!(library.count(c"fixture_destroyed"), 2);
}

#[test]
fn ready_without_required_exports_fails_and_retains_busy_instance_cleanup() {
    let (mut runtime, instance, library, probe) = setup(true);
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    probe.close_ready.set(false);
    runtime.advance(cx).unwrap();
    assert_eq!(
        runtime.module_status(instance),
        Ok(Lifecycle::Failed(Error::UNAVAILABLE))
    );
    assert_eq!(runtime.module_exports(instance), Err(Error::UNAVAILABLE));
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    assert_eq!(runtime.close(), Err(Error::BUSY));
    assert!(!probe.destroyed.get());
    runtime.advance(cx).unwrap();
    probe.close_ready.set(true);
    runtime.close().unwrap();
    assert!(probe.destroyed.get());
}

#[test]
fn failed_ready_module_fences_new_calls_but_allows_access_cleanup() {
    let (mut runtime, instance, _, probe) = setup(false);
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let export = runtime.module_exports(instance).unwrap()[0];
    let caller = runtime.bind(&[grant(export, 7)]).unwrap().caller();
    let handle = acquire(&caller, export, cx);
    probe.fail.set(true);
    runtime.advance(cx).unwrap();
    let inspect = call(
        &caller,
        Operation::Interfaces,
        handle,
        discovery::input(0, 65536),
    );
    assert_eq!(caller.submit(inspect.clone()), Err(Error::CLOSED));
    caller.release(inspect.request).unwrap();
    let closing = call(&caller, Operation::Close, handle, Value::unit());
    caller.submit(closing.clone()).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&caller, closing.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    caller.release(closing.request).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert!(probe.destroyed.get());
}
