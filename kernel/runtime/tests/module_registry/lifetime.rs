use super::*;
use reovim_kernel_interface::{
    Value,
    caller::{Call, Operation},
    resource::AccessId,
};
use reovim_kernel_interface::{module::ModuleContext, provider::Registrar};
use reovim_kernel_runtime::Grant;
use std::cell::RefCell;

#[derive(Default)]
struct Probe {
    failure: Cell<Option<Error>>,
    release: Cell<bool>,
    closing: Cell<bool>,
    destroyed: Cell<bool>,
    dependency: RefCell<Option<Rc<Self>>>,
    needs_drive: Cell<bool>,
    stop_error: Cell<Option<Error>>,
    draining: Cell<bool>,
    drained: Cell<bool>,
    provider_destroyed: Cell<bool>,
}
struct State {
    probe: Rc<Probe>,
    registrar: Registrar,
    path: &'static [u8],
    phase: Lifecycle,
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(dependency) = self.probe.dependency.borrow().as_ref() {
            assert!(!dependency.closing.get());
        }
        self.probe.destroyed.set(true);
    }
}
struct Export(Rc<Probe>);
impl Drop for Export {
    fn drop(&mut self) {
        assert!(!self.0.closing.get());
        self.0.provider_destroyed.set(true);
    }
}
fn stop_export(export: &mut Export) -> Result<(), Error> {
    if let Some(error) = export.0.stop_error.get() {
        return Err(error);
    }
    if export.0.needs_drive.get() && !export.0.drained.get() {
        export.0.draining.set(true);
        return Err(Error::BUSY);
    }
    Ok(())
}
fn drive(state: &mut State, _: &mut Context<'_>) -> Result<Progress, Error> {
    if state.probe.draining.get() {
        state.probe.drained.set(true);
    }
    if state.probe.closing.get() {
        return Ok(Progress::Idle);
    }
    if let Some(error) = state.probe.failure.get() {
        return Err(error);
    }
    if state.phase == Lifecycle::Constructed {
        let provider = ResourceProvider::new(
            Export(state.probe.clone()),
            ResourceOps {
                open: None,
                namespace: None,
                io: None,
                stop: stop_export,
            },
            Box::new([]),
            Rc::new(()),
        )
        .ok()
        .unwrap();
        assert!(state.registrar.stage(state.path, provider).is_ok());
        state.phase = Lifecycle::Ready;
    }
    Ok(Progress::Idle)
}
#[allow(clippy::result_large_err, reason = "inert constructor returns context")]
fn live_definition(
    id: u8,
    name: &'static str,
    dependencies: Vec<Dependency>,
    probe: &Rc<Probe>,
) -> Rc<Definition> {
    let declaration = Declaration::new(
        ModuleId([id; 16]),
        name,
        1,
        1,
        dependencies.into_boxed_slice(),
    )
    .unwrap();
    let probe = probe.clone();
    Rc::new(
        Definition::new(
            declaration,
            vec![Box::from(name.as_bytes())].into_boxed_slice(),
            1,
            Rc::new(move |context| {
                Module::construct(
                    ModuleImplementation {
                        construct: |context: ModuleContext<(
                            Rc<Probe>,
                            &'static [u8],
                        )>| {
                            Ok(State {
                                probe: context.imports.0,
                                registrar: context.exports,
                                path: context.imports.1,
                                phase: Lifecycle::Constructed,
                            })
                        },
                        driver: DriverOps {
                            status: |state| state.phase,
                            advance: drive,
                            close: close_state,
                        },
                    },
                    ModuleContext {
                        instance: context.instance,
                        namespace: context.namespace,
                        scope: context.scope,
                        imports: (probe.clone(), name.as_bytes()),
                        exports: context.exports,
                    },
                    Rc::new(()),
                )
            }),
        )
        .unwrap(),
    )
}

const fn grant(resource: ResourceId) -> Grant {
    Grant { resource, access: 0, create_access: 0, retain_loans: false }
}

#[test]
fn dependency_failure_fences_consumers_and_retains_busy_cleanup() {
    let (mut runtime, root, binding) = base();
    let probes = [
        Rc::new(Probe::default()),
        Rc::new(Probe::default()),
        Rc::new(Probe::default()),
    ];
    for (index, name) in ["dep", "middle", "top"].into_iter().enumerate() {
        let id = u8::try_from(index + 1).unwrap();
        if index != 0 {
            *probes[index].dependency.borrow_mut() =
                Some(probes[index - 1].clone());
        }
        let dependencies = if index == 0 {
            vec![]
        } else {
            vec![dependency(id - 1, 1, DependencyScope::Same, false)]
        };
        register(
            &mut runtime,
            root,
            &binding,
            live_definition(id, name, dependencies, &probes[index]),
        );
    }
    let independent = Rc::new(Probe::default());
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(4, "independent", vec![], &independent),
    );
    let selection =
        runtime.select_module(root, "top", 1, ModuleScope::Global).unwrap();
    let mut instances = Vec::new();
    let mut exports = Vec::new();
    for selected in selection.modules() {
        let id = runtime.activate_selected(selected).unwrap();
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
        assert_eq!(runtime.module_status(id), Ok(Lifecycle::Ready));
        exports.push(runtime.module_exports(id).unwrap()[0]);
        instances.push(id);
    }
    let separate = runtime
        .select_module(root, "independent", 1, ModuleScope::Global)
        .unwrap();
    let other = runtime.activate_selected(&separate.modules()[0]).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let other_export = runtime.module_exports(other).unwrap()[0];
    let mut retained = Vec::new();
    for &export in &exports {
        retained.push(runtime.bind(&[grant(export)]).unwrap());
    }
    probes[0].failure.set(Some(Error::UNAVAILABLE));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    for ((&id, &export), held) in instances.iter().zip(&exports).zip(&retained)
    {
        assert_eq!(
            runtime.module_status(id),
            Ok(Lifecycle::Failed(Error::UNAVAILABLE))
        );
        assert_eq!(runtime.module_exports(id), Err(Error::UNAVAILABLE));
        assert_closed(&mut runtime, held, export);
    }
    // Composition grants still retain the fenced entries. Their provider
    // owners, and therefore every dependency, must survive until release.
    assert!(probes.iter().all(|probe| !probe.closing.get()));
    assert_eq!(runtime.module_status(other), Ok(Lifecycle::Ready));
    assert!(runtime.bind(&[grant(other_export)]).is_ok());
    assert!(!independent.closing.get());
    for binding in &retained {
        runtime.retire_binding(binding).unwrap();
    }
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(probes[2].closing.get());
    assert!(!probes[1].closing.get() && !probes[0].closing.get());
    probes[2].release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(probes[2].destroyed.get() && probes[1].closing.get());
    assert!(!probes[0].closing.get());
    probes[1].release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(probes[1].destroyed.get() && probes[0].closing.get());
    probes[0].release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(probes.iter().all(|probe| probe.destroyed.get()));
    assert_eq!(runtime.module_status(other), Ok(Lifecycle::Ready));
    assert!(!independent.closing.get());
    independent.release.set(true);
    runtime.close().unwrap();
    assert!(independent.destroyed.get());
}

fn assert_closed(
    runtime: &mut Runtime,
    binding: &Binding,
    resource: ResourceId,
) {
    let caller = binding.caller();
    let request = caller.reserve().unwrap();
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&resource.epoch);
    locator.extend_from_slice(&resource.serial.to_le_bytes());
    assert_eq!(
        caller.submit(Call {
            request,
            operation: Operation::Acquire,
            subject: AccessId(0),
            command: 0,
            expected: None,
            input: Value::copy(&locator, &[]).unwrap(),
        }),
        Err(Error::CLOSED)
    );
    caller.release(request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
}

fn close_state(state: &mut State) -> Result<(), Error> {
    state.probe.closing.set(true);
    if !state.probe.release.get() {
        return Err(Error::BUSY);
    }
    state.phase = Lifecycle::Closed;
    Ok(())
}

#[test]
fn consumer_destructor_runs_before_dependency_close_during_failure_driving() {
    let (mut runtime, root, binding) = base();
    let dependency = Rc::new(Probe::default());
    let consumer = Rc::new(Probe::default());
    *consumer.dependency.borrow_mut() = Some(dependency.clone());
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(1, "dep", vec![], &dependency),
    );
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(
            2,
            "consumer",
            vec![super::dependency(1, 1, DependencyScope::Same, false)],
            &consumer,
        ),
    );
    let selected = runtime
        .select_module(root, "consumer", 1, ModuleScope::Global)
        .unwrap();
    for module in selected.modules() {
        runtime.activate_selected(module).unwrap();
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    consumer.needs_drive.set(true);
    dependency.failure.set(Some(Error::UNAVAILABLE));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(consumer.draining.get());
    assert!(!consumer.closing.get() && !dependency.closing.get());
    assert!(!consumer.provider_destroyed.get());
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(consumer.drained.get() && consumer.provider_destroyed.get());
    assert!(consumer.closing.get());
    assert!(!dependency.closing.get());
    consumer.release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(consumer.destroyed.get());
    assert!(dependency.closing.get());
    dependency.release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(dependency.destroyed.get());
    runtime.close().unwrap();
}

#[test]
fn unload_rejects_live_uses_then_drains_without_cascading_to_dependencies() {
    let (mut runtime, root, binding) = base();
    let dependency = Rc::new(Probe::default());
    let consumer = Rc::new(Probe::default());
    *consumer.dependency.borrow_mut() = Some(dependency.clone());
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(1, "dep", vec![], &dependency),
    );
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(
            2,
            "consumer",
            vec![super::dependency(1, 1, DependencyScope::Same, false)],
            &consumer,
        ),
    );
    let selection = runtime
        .select_module(root, "consumer", 1, ModuleScope::Global)
        .unwrap();
    let dep = runtime.activate_selected(&selection.modules()[0]).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let subject = runtime.activate_selected(&selection.modules()[1]).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.unload_module(dep), Err(Error::BUSY));
    let export = runtime.module_exports(subject).unwrap()[0];
    let use_binding = runtime.bind(&[grant(export)]).unwrap();
    assert_eq!(runtime.unload_module(subject), Err(Error::BUSY));
    assert_eq!(runtime.module_status(subject), Ok(Lifecycle::Ready));
    assert!(!consumer.closing.get() && !dependency.closing.get());
    assert_eq!(runtime.module_exports(subject).unwrap(), &[export]);
    assert_available(&mut runtime, &use_binding, export);
    runtime.retire_binding(&use_binding).unwrap();
    consumer.needs_drive.set(true);
    assert_eq!(runtime.unload_module(subject), Ok(Lifecycle::Closing));
    assert!(matches!(runtime.bind(&[grant(export)]), Err(Error::CLOSED)));
    assert_eq!(runtime.module_exports(subject), Err(Error::CLOSED));
    assert!(!consumer.closing.get());
    assert_eq!(runtime.unload_module(subject), Ok(Lifecycle::Closing));
    assert_eq!(
        runtime.activate_selected(&selection.modules()[1]),
        Err(Error::CLOSED)
    );
    assert_eq!(
        runtime.advance(&mut Context::from_waker(Waker::noop())),
        Ok(Progress::More)
    );
    assert!(consumer.draining.get() && !consumer.closing.get());
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(consumer.provider_destroyed.get() && consumer.closing.get());
    assert_eq!(runtime.module_status(subject), Ok(Lifecycle::Closing));
    consumer.release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.module_status(subject), Ok(Lifecycle::Closed));
    assert!(consumer.destroyed.get());
    assert_eq!(runtime.module_status(dep), Ok(Lifecycle::Ready));
    assert!(!dependency.closing.get());
    assert_eq!(runtime.unload_module(dep), Ok(Lifecycle::Closing));
    dependency.release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.module_status(dep), Ok(Lifecycle::Closed));
    runtime.close().unwrap();
}

fn assert_available(
    runtime: &mut Runtime,
    binding: &Binding,
    resource: ResourceId,
) {
    let caller = binding.caller();
    let request = caller.reserve().unwrap();
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&resource.epoch);
    locator.extend_from_slice(&resource.serial.to_le_bytes());
    caller
        .submit(Call {
            request,
            operation: Operation::Acquire,
            subject: AccessId(0),
            command: 0,
            expected: None,
            input: Value::copy(&locator, &[]).unwrap(),
        })
        .unwrap();
    let reovim_kernel_interface::caller::Status::Outcome(result) =
        caller.snapshot(request).unwrap()
    else {
        panic!("missing result")
    };
    assert!(matches!(result.as_ref(), reovim_kernel_interface::Outcome::Ok(_)));
    caller.release(request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
}

#[test]
fn unload_before_start_never_publishes_required_exports() {
    let (mut runtime, root, binding) = base();
    let probe = Rc::new(Probe::default());
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(1, "inert", vec![], &probe),
    );
    let selection =
        runtime.select_module(root, "inert", 1, ModuleScope::Global).unwrap();
    let instance = runtime.activate_selected(&selection.modules()[0]).unwrap();
    assert_eq!(runtime.unload_module(instance), Ok(Lifecycle::Closing));
    probe.release.set(true);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Closed));
    assert!(probe.destroyed.get());
    assert!(!probe.provider_destroyed.get());
    runtime.close().unwrap();
}

#[test]
fn caller_status_exposes_cleanup_error_while_original_unload_stays_owned() {
    use reovim_kernel_interface::{
        Outcome, caller::Status, module::control as api,
    };
    let (mut runtime, root, binding) = base();
    let probe = Rc::new(Probe::default());
    register(
        &mut runtime,
        root,
        &binding,
        live_definition(1, "service", vec![], &probe),
    );
    let selected =
        runtime.select_module(root, "service", 1, ModuleScope::Global).unwrap();
    let instance = runtime.activate_selected(&selected.modules()[0]).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let (caller, bound) = super::control::bind(&mut runtime, instance);
    super::control::invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &5_u32.to_le_bytes(),
    );
    let failure = Error::with_id(91, instance);
    probe.stop_error.set(Some(failure));
    let unload = super::control::call(
        &caller,
        Operation::Ioctl,
        bound,
        api::UNLOAD,
        &[],
    );
    caller.submit(unload.clone()).unwrap();
    for _ in 0..3 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    assert_eq!(caller.snapshot(unload.request), Ok(Status::Pending));
    assert!(!probe.destroyed.get());
    let status = super::control::call(
        &caller,
        Operation::Ioctl,
        bound,
        api::STATUS,
        &[],
    );
    caller.submit(status.clone()).unwrap();
    for _ in 0..3 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    assert_eq!(
        super::control::outcome(&caller, status.request).as_ref(),
        &Outcome::Failed(failure)
    );
    caller.release(status.request).unwrap();
    probe.stop_error.set(None);
    probe.release.set(true);
    for _ in 0..4 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    assert_eq!(
        super::control::outcome(&caller, unload.request).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    caller.adopt(unload.request).unwrap();
    caller.release(unload.request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(probe.destroyed.get());
    runtime.close().unwrap();
}
