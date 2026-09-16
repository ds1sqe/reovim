use super::*;
use reovim_kernel_interface::{call::Cancellation, module::ModuleContext};
use reovim_kernel_runtime::Limits;
use std::{cell::RefCell, task::Poll};

struct Probe {
    ids: RefCell<Vec<ResourceId>>,
    reject: Cell<bool>,
    ready: Cell<bool>,
    clean: Cell<bool>,
    stopped: Cell<bool>,
    destroyed: Cell<bool>,
}
impl Probe {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            ids: RefCell::new(Vec::new()),
            reject: Cell::new(false),
            ready: Cell::new(true),
            clean: Cell::new(true),
            stopped: Cell::new(false),
            destroyed: Cell::new(false),
        })
    }
    fn id(&self) -> ResourceId {
        *self.ids.borrow().last().unwrap()
    }
}
struct Driver {
    probe: Rc<Probe>,
    phase: Lifecycle,
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.probe.destroyed.set(true);
    }
}
#[allow(
    clippy::result_large_err,
    reason = "inert construction returns context"
)]
fn implementation(
    id: u8,
    name: &str,
    dependencies: Vec<Dependency>,
    probe: &Rc<Probe>,
) -> Rc<Definition> {
    let probe = probe.clone();
    Rc::new(
        Definition::new(
            Declaration::new(
                ModuleId([id; 16]),
                name,
                1,
                1,
                dependencies.into_boxed_slice(),
            )
            .unwrap(),
            Box::new([]),
            0,
            Rc::new(move |context| {
                if probe.reject.get() {
                    return Err(Error::PROVIDER);
                }
                Module::construct(
                    ModuleImplementation {
                        construct: |context: ModuleContext<Rc<Probe>>| {
                            context
                                .imports
                                .ids
                                .borrow_mut()
                                .push(context.instance);
                            Ok(Driver {
                                probe: context.imports,
                                phase: Lifecycle::Constructed,
                            })
                        },
                        driver: DriverOps {
                            status: |driver| driver.phase,
                            advance: drive_module,
                            close: close_module,
                        },
                    },
                    ModuleContext {
                        instance: context.instance,
                        namespace: context.namespace,
                        scope: context.scope,
                        imports: probe.clone(),
                        exports: context.exports,
                    },
                    Rc::new(()),
                )
            }),
        )
        .unwrap(),
    )
}
#[allow(
    clippy::unnecessary_wraps,
    reason = "DriverOps requires a fallible callback"
)]
fn drive_module(
    driver: &mut Driver,
    _: &mut Context<'_>,
) -> Result<Progress, Error> {
    if driver.phase == Lifecycle::Closed {
        return Ok(Progress::Stopped);
    }
    if driver.phase != Lifecycle::Closing {
        driver.phase = if driver.probe.ready.get() {
            Lifecycle::Ready
        } else {
            Lifecycle::Starting
        };
    }
    Ok(Progress::Idle)
}
fn close_module(driver: &mut Driver) -> Result<(), Error> {
    driver.phase = Lifecycle::Closing;
    if !driver.probe.clean.get() {
        return Err(Error::BUSY);
    }
    driver.phase = Lifecycle::Closed;
    driver.probe.stopped.set(true);
    Ok(())
}
fn drive(runtime: &mut Runtime, turns: usize) {
    for _ in 0..turns {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
}
fn select(
    runtime: &Runtime,
    root: ResourceId,
    name: &str,
) -> reovim_kernel_runtime::ModuleSelection {
    runtime.select_module(root, name, 1, ModuleScope::Global).unwrap()
}
fn required() -> Vec<Dependency> {
    vec![dependency(1, 1, DependencyScope::Same, false)]
}

#[test]
fn rollback_waits_for_provisional_cleanup_and_blocks_explicit_unload() {
    let (mut runtime, root, binding) = base();
    let dep = Probe::new();
    let bad = Probe::new();
    dep.clean.set(false);
    bad.reject.set(true);
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "dep", vec![], &dep),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(2, "bad", required(), &bad),
    );
    let work = runtime.probe_modules(select(&runtime, root, "bad")).unwrap();
    drive(&mut runtime, 1);
    assert_eq!(runtime.unload_module(dep.id()), Err(Error::BUSY));
    assert_eq!(
        runtime.advance(&mut Context::from_waker(Waker::noop())),
        Ok(Progress::More)
    );
    drive(&mut runtime, 6);
    assert_eq!(work.status(), Poll::Pending);
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Closing));
    assert!(!dep.stopped.get());
    dep.clean.set(true);
    drive(&mut runtime, 5);
    assert_eq!(work.status(), Poll::Ready(Err(Error::PROVIDER)));
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Closed));
    assert!(dep.stopped.get());
    runtime.close().unwrap();
}

#[test]
fn failed_graph_preserves_a_dependency_shared_by_another_pending_graph() {
    let (mut runtime, root, binding) = base();
    let dep = Probe::new();
    let bad = Probe::new();
    let good = Probe::new();
    bad.reject.set(true);
    good.ready.set(false);
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "dep", vec![], &dep),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(2, "bad", required(), &bad),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(3, "good", required(), &good),
    );
    let failing = runtime.probe_modules(select(&runtime, root, "bad")).unwrap();
    let waiting =
        runtime.probe_modules(select(&runtime, root, "good")).unwrap();
    drive(&mut runtime, 10);
    assert_eq!(failing.status(), Poll::Ready(Err(Error::PROVIDER)));
    assert_eq!(waiting.status(), Poll::Pending);
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Ready));
    assert!(!dep.stopped.get());
    good.ready.set(true);
    drive(&mut runtime, 4);
    assert_eq!(waiting.status(), Poll::Ready(Ok(good.id())));
    assert_eq!(waiting.cancel(), Cancellation::TooLate);
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Ready));
    runtime.close().unwrap();
}

#[test]
fn cancellation_preserves_active_dependencies_and_shutdown_settles() {
    let (mut runtime, root, binding) = base();
    let dep = Probe::new();
    let slow = Probe::new();
    slow.ready.set(false);
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "dep", vec![], &dep),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(2, "slow", required(), &slow),
    );
    let dep_selection = select(&runtime, root, "dep");
    runtime.activate_selected(&dep_selection.modules()[0]).unwrap();
    drive(&mut runtime, 1);
    let cancelled =
        runtime.probe_modules(select(&runtime, root, "slow")).unwrap();
    drive(&mut runtime, 4);
    assert_eq!(cancelled.cancel(), Cancellation::Accepted);
    drive(&mut runtime, 8);
    assert_eq!(cancelled.status(), Poll::Ready(Err(Error::CANCELLED)));
    assert!(!dep.stopped.get());
    assert!(slow.stopped.get());
    let shutdown =
        runtime.probe_modules(select(&runtime, root, "slow")).unwrap();
    drive(&mut runtime, 4);
    runtime.close().unwrap();
    assert_eq!(shutdown.status(), Poll::Ready(Err(Error::CLOSED)));
}

#[test]
fn dropping_observation_retains_and_drives_provisional_rollback() {
    let (mut runtime, root, binding) = base();
    let dep = Probe::new();
    let slow = Probe::new();
    slow.ready.set(false);
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "dep", vec![], &dep),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(2, "slow", required(), &slow),
    );
    let observer =
        runtime.probe_modules(select(&runtime, root, "slow")).unwrap();
    drive(&mut runtime, 4);
    assert_eq!(observer.status(), Poll::Pending);
    drop(observer);
    drive(&mut runtime, 10);
    assert!(slow.stopped.get() && dep.stopped.get());
    assert_eq!(runtime.module_status(slow.id()), Ok(Lifecycle::Closed));
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Closed));
    runtime.close().unwrap();
}

#[test]
fn foreign_graph_rejects_before_admitting_any_constructor() {
    let (mut runtime, root, binding) = base();
    let probe = Probe::new();
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "one", vec![], &probe),
    );
    let selection = select(&runtime, root, "one");
    let (mut foreign, _, _) = base();
    assert!(matches!(foreign.probe_modules(selection), Err(Error::DENIED)));
    assert!(probe.ids.borrow().is_empty());
    foreign.close().unwrap();
    runtime.close().unwrap();
}

#[test]
fn live_activation_imports_reject_retirement_before_fencing() {
    let (mut runtime, root, binding) = base();
    let probe = Probe::new();
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "one", vec![], &probe),
    );
    let activation =
        runtime.probe_modules(select(&runtime, root, "one")).unwrap();
    assert_eq!(runtime.retire_binding(&binding), Err(Error::BUSY));
    let caller = binding.caller();
    let request = caller.reserve().unwrap();
    caller.release(request).unwrap();
    drive(&mut runtime, 3);
    assert_eq!(activation.status(), Poll::Ready(Ok(probe.id())));
    assert_eq!(runtime.retire_binding(&binding), Err(Error::BUSY));
    let request = caller.reserve().unwrap();
    caller.release(request).unwrap();
    assert_eq!(runtime.unload_module(probe.id()), Ok(Lifecycle::Closing));
    drive(&mut runtime, 3);
    runtime.retire_binding(&binding).unwrap();
    assert_eq!(caller.reserve(), Err(Error::CLOSED));
    runtime.close().unwrap();
}

fn limited(
    resources: usize,
    requests: usize,
) -> (Runtime, ResourceId, Binding) {
    let limits = Limits { resources, requests, ..Limits::default() };
    let mut runtime = Runtime::new([0xbb; 16], limits).unwrap();
    let provider = ResourceProvider::new(
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
    .unwrap();
    let root = runtime.register(b"/catalog", provider, None).ok().unwrap();
    let binding = runtime.bind(&[]).unwrap();
    runtime.start().unwrap();
    (runtime, root, binding)
}

#[test]
fn resource_pressure_rolls_back_before_the_unadmitted_constructor() {
    let (mut runtime, root, binding) = limited(4, 4);
    let dep = Probe::new();
    let top = Probe::new();
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "dep", vec![], &dep),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(2, "top", required(), &top),
    );
    let activation =
        runtime.probe_modules(select(&runtime, root, "top")).unwrap();
    drive(&mut runtime, 10);
    assert_eq!(activation.status(), Poll::Ready(Err(Error::PRESSURE)));
    assert!(top.ids.borrow().is_empty());
    assert!(dep.stopped.get());
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Closed));
    runtime.close().unwrap();
}

#[test]
fn queued_activation_capacity_rejects_before_construction_and_recovers() {
    let (mut runtime, root, binding) = limited(8, 1);
    let probe = Probe::new();
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "one", vec![], &probe),
    );
    let first = runtime.probe_modules(select(&runtime, root, "one")).unwrap();
    assert!(matches!(
        runtime.probe_modules(select(&runtime, root, "one")),
        Err(Error::PRESSURE)
    ));
    assert!(probe.ids.borrow().is_empty());
    assert_eq!(first.cancel(), Cancellation::Accepted);
    drive(&mut runtime, 1);
    assert_eq!(first.status(), Poll::Ready(Err(Error::CANCELLED)));
    assert!(probe.ids.borrow().is_empty());
    let next = runtime.probe_modules(select(&runtime, root, "one")).unwrap();
    drive(&mut runtime, 3);
    assert_eq!(next.status(), Poll::Ready(Ok(probe.id())));
    runtime.close().unwrap();
}

#[test]
fn shutdown_settles_activation_after_destruction_before_receipts_drain() {
    shutdown_during_activation(false, Error::CLOSED);
}

#[test]
fn cancellation_before_shutdown_still_waits_for_destruction() {
    shutdown_during_activation(true, Error::CANCELLED);
}

fn shutdown_during_activation(cancel: bool, expected: Error) {
    let (mut runtime, root, binding) = base();
    let slow = Probe::new();
    slow.ready.set(false);
    slow.clean.set(false);
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "slow", vec![], &slow),
    );
    let work = runtime.probe_modules(select(&runtime, root, "slow")).unwrap();
    drive(&mut runtime, 3);
    let caller = binding.caller();
    let receipt = caller.reserve().unwrap();
    if cancel {
        assert_eq!(work.cancel(), Cancellation::Accepted);
    }
    assert_eq!(runtime.close(), Err(Error::BUSY));
    drive(&mut runtime, 4);
    assert_eq!(work.status(), Poll::Pending);
    assert!(!slow.stopped.get());
    assert!(!slow.destroyed.get());
    slow.clean.set(true);
    drive(&mut runtime, 4);
    assert!(slow.stopped.get());
    assert!(slow.destroyed.get());
    assert_eq!(work.status(), Poll::Ready(Err(expected)));
    assert_eq!(runtime.close(), Err(Error::BUSY));
    caller.release(receipt).unwrap();
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

#[test]
fn shutdown_preserves_the_original_failure_during_busy_rollback() {
    let (mut runtime, root, binding) = base();
    let dep = Probe::new();
    dep.clean.set(false);
    let bad = Probe::new();
    bad.reject.set(true);
    register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "dep", vec![], &dep),
    );
    register(
        &mut runtime,
        root,
        &binding,
        implementation(2, "bad", required(), &bad),
    );
    let work = runtime.probe_modules(select(&runtime, root, "bad")).unwrap();
    drive(&mut runtime, 8);
    assert_eq!(runtime.module_status(dep.id()), Ok(Lifecycle::Closing));
    assert_eq!(work.status(), Poll::Pending);
    let caller = binding.caller();
    let receipt = caller.reserve().unwrap();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    drive(&mut runtime, 3);
    assert_eq!(work.status(), Poll::Pending);
    dep.clean.set(true);
    drive(&mut runtime, 4);
    assert!(dep.destroyed.get());
    assert_eq!(work.status(), Poll::Ready(Err(Error::PROVIDER)));
    caller.release(receipt).unwrap();
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

#[test]
fn caller_cancellation_retains_busy_activation_rollback() {
    caller_cleanup(false);
}

#[test]
fn caller_activation_settles_during_shutdown_before_its_receipt_releases() {
    caller_cleanup(true);
}

fn caller_cleanup(shutdown: bool) {
    use super::control::{bind_decl, call, invoke, outcome};
    use reovim_kernel_interface::{
        Outcome,
        caller::{Operation, Status},
        module::activation,
    };
    let (mut runtime, root, binding) = base();
    let slow = Probe::new();
    slow.ready.set(false);
    slow.clean.set(false);
    let package = register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "slow", vec![], &slow),
    );
    let (caller, bound) =
        bind_decl(&mut runtime, package, &activation::declaration().unwrap());
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &4_u32.to_le_bytes(),
    );
    let request = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(request.clone()).unwrap();
    drive(&mut runtime, 4);
    assert_eq!(runtime.module_status(slow.id()), Ok(Lifecycle::Starting));
    let error = if shutdown {
        assert_eq!(runtime.close(), Err(Error::BUSY));
        Error::CLOSED
    } else {
        assert_eq!(caller.cancel(request.request), Ok(Cancellation::Accepted));
        Error::CANCELLED
    };
    drive(&mut runtime, 5);
    assert!(matches!(caller.snapshot(request.request), Ok(Status::Pending)));
    assert!(!slow.destroyed.get());
    slow.clean.set(true);
    drive(&mut runtime, 6);
    assert!(slow.destroyed.get());
    assert_eq!(
        outcome(&caller, request.request).as_ref(),
        &Outcome::Failed(error)
    );
    caller.submit(request.clone()).unwrap();
    assert_eq!(
        outcome(&caller, request.request).as_ref(),
        &Outcome::Failed(error)
    );
    caller.release(request.request).unwrap();
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

#[test]
fn caller_activation_queue_pressure_releases_private_output_capacity() {
    use super::control::{bind_decl, call, invoke, outcome};
    use reovim_kernel_interface::{
        Outcome, caller::Operation, module::activation,
    };
    let (mut runtime, root, binding) =
        base_limits(Limits { requests: 2, ..Limits::default() });
    let slow = Probe::new();
    slow.ready.set(false);
    let package = register(
        &mut runtime,
        root,
        &binding,
        implementation(1, "slow", vec![], &slow),
    );
    let mut callers = Vec::new();
    for _ in 0..3 {
        let (caller, bound) = bind_decl(
            &mut runtime,
            package,
            &activation::declaration().unwrap(),
        );
        invoke(
            &mut runtime,
            &caller,
            Operation::SetAccess,
            bound,
            0,
            &4_u32.to_le_bytes(),
        );
        callers.push((caller, bound));
    }
    drive(&mut runtime, 1);
    let mut requests = Vec::new();
    for (caller, bound) in &callers {
        let request = call(caller, Operation::Exec, *bound, 0, &[]);
        caller.submit(request.clone()).unwrap();
        requests.push(request);
    }
    drive(&mut runtime, 4);
    assert_eq!(
        outcome(&callers[2].0, requests[2].request).as_ref(),
        &Outcome::Failed(Error::PRESSURE)
    );
    let original = slow.id();
    assert_eq!(runtime.module_status(original), Ok(Lifecycle::Starting));
    for (caller, request) in callers.iter().zip(&requests).take(2) {
        assert_eq!(
            caller.0.cancel(request.request),
            Ok(Cancellation::Accepted)
        );
    }
    drive(&mut runtime, 12);
    assert!(slow.destroyed.get());
    for (caller, request) in callers.iter().zip(&requests) {
        caller.0.release(request.request).unwrap();
    }
    drive(&mut runtime, 1);
    slow.ready.set(true);
    let (caller, bound) = &callers[2];
    let retry = call(caller, Operation::Exec, *bound, 0, &[]);
    caller.submit(retry.clone()).unwrap();
    drive(&mut runtime, 8);
    assert!(matches!(outcome(caller, retry.request).as_ref(), Outcome::Ok(_)));
    assert_ne!(slow.id(), original);
    assert_eq!(runtime.module_status(slow.id()), Ok(Lifecycle::Ready));
    caller.release(retry.request).unwrap();
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}
