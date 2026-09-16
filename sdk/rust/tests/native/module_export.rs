use super::*;
use reovim_kernel_interface::{
    call::Cancellation,
    caller::{Caller, CallerOps, Status},
    driver::{DriverOps, Lifecycle, Progress},
    module::{ModuleContext, ModuleImplementation},
    provider::{Registrar, RegistrarOps},
};
use reovim_sdk::{ExportedCaller, ExportedRegistrar, export_module_instance};

static DROPPED: AtomicBool = AtomicBool::new(false);
struct State {
    phase: Lifecycle,
    caller: Caller,
}
impl Drop for State {
    fn drop(&mut self) {
        assert_eq!(self.caller.reserve(), Err(Error::CLOSED));
        DROPPED.store(true, Ordering::Relaxed);
    }
}

#[allow(clippy::result_large_err, reason = "inert rejection returns context")]
fn implementation(reject: bool) -> ModuleImplementation<State, Caller> {
    ModuleImplementation {
        construct: if reject {
            |context: ModuleContext<Caller>| {
                let error = Error::with_id(5, context.instance);
                Err((context, error))
            }
        } else {
            |context: ModuleContext<Caller>| {
                Ok(State {
                    phase: Lifecycle::Constructed,
                    caller: context.imports,
                })
            }
        },
        driver: DriverOps {
            status: |state| state.phase,
            advance: |state, _| {
                if state.phase == Lifecycle::Closing {
                    state.phase = Lifecycle::Closed;
                    return Ok(Progress::Stopped);
                }
                assert_eq!(state.caller.snapshot(1), Ok(Status::Pending));
                state.phase = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                state.phase = Lifecycle::Closing;
                Err(Error::BUSY)
            },
        },
    }
}

fn environment() -> (ExportedCaller, ExportedRegistrar) {
    let caller = Caller::new(
        (),
        CallerOps {
            reserve: |()| Ok(1),
            submit: |(), _| Ok(()),
            result: |(), _, _| Ok(Status::Pending),
            cancel: |(), _| Ok(Cancellation::TooLate),
            reconcile: |(), _| Err(Error::UNSUPPORTED),
            adopt: |(), _| Ok(()),
            release: |(), _| Ok(()),
        },
    );
    let registrar = Registrar::new(
        (),
        RegistrarOps {
            stage: |(), _, provider| Err((Error::DENIED, provider)),
        },
    );
    (
        ExportedCaller::new(caller, LIMITS, 1).unwrap(),
        ExportedRegistrar::new(registrar, Rc::new(()), LIMITS),
    )
}

fn context(
    caller: &ExportedCaller,
    exports: &ExportedRegistrar,
) -> abi::ModuleContext {
    abi::ModuleContext {
        bytes: size_of::<abi::ModuleContext>(),
        instance: abi::Identity { epoch: [0x73; 16], serial: 1 },
        scope: 0,
        session: abi::Identity::default(),
        namespace_resource: abi::Identity { epoch: [0x73; 16], serial: 2 },
        imports: caller.raw(),
        exports: exports.raw(),
    }
}
fn empty() -> abi::ModuleInstance {
    abi::ModuleInstance {
        bytes: 0,
        context: core::ptr::null_mut(),
        driver: core::ptr::null(),
        destroy: None,
    }
}
fn phase(instance: abi::ModuleInstance) -> u32 {
    let mut status = abi::ModuleStatus::default();
    assert_eq!(
        unsafe {
            (*instance.driver).status.unwrap()(
                instance.context,
                &raw mut status,
            )
        },
        0
    );
    status.phase
}
fn advance(instance: abi::ModuleInstance) -> u32 {
    let mut progress = u32::MAX;
    let mut error = abi::Error::default();
    let wake = abi::Wake {
        context: core::ptr::null_mut(),
        retain: None,
        release: None,
        wake: None,
    };
    assert_eq!(
        unsafe {
            (*instance.driver).advance.unwrap()(
                instance.context,
                wake,
                &raw mut progress,
                &raw mut error,
            )
        },
        0
    );
    progress
}

#[test]
fn exported_module_drives_busy_cleanup_and_drains_imports_before_closed() {
    let (caller, exports) = environment();
    let context = context(&caller, &exports);
    let mut instance = empty();
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            export_module_instance(
                &raw const context,
                &raw mut instance,
                &raw mut error,
                implementation(false),
                LIMITS,
                1,
            )
        },
        0
    );
    assert_eq!(phase(instance), 0);
    assert_eq!(advance(instance), 1);
    assert_eq!(phase(instance), 2);
    assert_eq!(
        unsafe {
            (*instance.driver).close.unwrap()(instance.context, &raw mut error)
        },
        Error::BUSY.code.0
    );
    assert_eq!(phase(instance), 3);
    assert!(!DROPPED.load(Ordering::Relaxed));
    assert_eq!(advance(instance), 2);
    assert_eq!(phase(instance), 4);
    // The module's old view must release the host's sole delivery slot.
    let raw = caller.raw();
    let mut view = core::mem::MaybeUninit::uninit();
    assert_eq!(
        unsafe {
            (*raw.ops).result.unwrap()(
                raw.context,
                1,
                view.as_mut_ptr(),
                &raw mut error,
            )
        },
        0
    );
    let view = unsafe { view.assume_init() };
    assert_eq!(unsafe { (*view.ops).finish.unwrap()(view.context) }, 0);
    unsafe { instance.destroy.unwrap()(instance.context) };
    assert!(DROPPED.load(Ordering::Relaxed));
}

#[test]
fn exported_module_rejects_inertly_with_exact_diagnostic_identity() {
    let (caller, exports) = environment();
    let mut context = context(&caller, &exports);
    let mut instance = empty();
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            export_module_instance(
                &raw const context,
                &raw mut instance,
                &raw mut error,
                implementation(true),
                LIMITS,
                1,
            )
        },
        5
    );
    assert_eq!(error.detail, context.instance);
    assert!(instance.context.is_null());
    assert!(instance.driver.is_null());
    assert!(instance.destroy.is_none());
    context.scope = 2;
    assert_eq!(
        unsafe {
            export_module_instance(
                &raw const context,
                &raw mut instance,
                &raw mut error,
                implementation(false),
                LIMITS,
                1,
            )
        },
        Error::INVALID.code.0
    );
    assert!(instance.context.is_null());
}
