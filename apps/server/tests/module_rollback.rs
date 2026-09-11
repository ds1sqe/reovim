use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Factory, Module},
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

struct Control {
    ready: Cell<bool>,
    finish: Cell<bool>,
    fail_construct: Cell<bool>,
    fail_driver: Cell<bool>,
    instance: Cell<Option<ResourceId>>,
    closing: Cell<bool>,
    destroyed: Cell<bool>,
    checked_import: Cell<bool>,
    can_unload: Cell<bool>,
    unload_checked: Cell<bool>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            ready: Cell::new(true),
            finish: Cell::new(true),
            fail_construct: Cell::new(false),
            fail_driver: Cell::new(false),
            instance: Cell::new(None),
            closing: Cell::new(false),
            destroyed: Cell::new(false),
            checked_import: Cell::new(false),
            can_unload: Cell::new(true),
            unload_checked: Cell::new(false),
        }
    }
}

struct State {
    control: Rc<Control>,
    status: Lifecycle,
    imports: Option<Connection>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.control.destroyed.set(true);
    }
}

fn advance(state: &mut State) -> Result<Progress, Error> {
    if state.status == Lifecycle::Closing {
        return Ok(cleanup(state));
    }
    if state.control.fail_driver.get() {
        return Err(Error::PROVIDER);
    }
    state.status = if state.control.ready.get() {
        Lifecycle::Ready
    } else {
        Lifecycle::Starting
    };
    Ok(Progress::Idle)
}

fn cleanup(state: &mut State) -> Progress {
    if let Some(imports) = &state.imports {
        imports
            .resolve("services/b")
            .into_inner()
            .expect("dependency must remain published during cleanup");
        state.control.checked_import.set(true);
    }
    if state.control.finish.get() {
        state.status = Lifecycle::Closed;
        Progress::Stopped
    } else {
        Progress::Idle
    }
}

fn factory(
    name: &'static str,
    control: &Rc<Control>,
    imports: Option<Connection>,
) -> Factory {
    Factory::new(
        (name, Rc::clone(control), imports),
        |(name, control, imports), id| {
            assert_eq!(control.instance.replace(Some(id)), None);
            if control.fail_construct.get() {
                return Err(Error::PROVIDER);
            }
            Ok(Module::new(
                State {
                    control: Rc::clone(control),
                    status: Lifecycle::Constructed,
                    imports: imports.clone(),
                },
                DriverOps {
                    status: |s| s.status,
                    advance: |s, _| advance(s),
                    close: |s| {
                        s.control.closing.set(true);
                        s.status = Lifecycle::Closing;
                        Ok(())
                    },
                },
                vec![Contribution::Resource {
                    path: format!("services/{name}").into(),
                    provider: resource(name, control),
                }],
                Rc::new(()),
            ))
        },
        Rc::new(()),
    )
}

fn resource(name: &'static str, control: &Rc<Control>) -> ResourceProvider {
    ResourceProvider::new(
        (name, Rc::clone(control)),
        ResourceOps {
            read: |(name, _), _, input| {
                Box::pin(std::future::ready(Ok(Message::new(
                    input.contract,
                    name.as_bytes(),
                ))))
            },
            write: |_, _, _| {
                Box::pin(std::future::ready(Err(Error::UNSUPPORTED)))
            },
            unload: Some(|(_, control)| {
                control.unload_checked.set(true);
                if control.can_unload.get() { Ok(()) } else { Err(Error::BUSY) }
            }),
        },
        Box::new([Surface {
            contract: Contract::new("resource.bytes", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn register(
    runtime: &mut Runtime,
    name: &'static str,
    mut factory: Factory,
    dependencies: &[Contract],
) -> ResourceId {
    factory.dependencies = dependencies.into();
    runtime
        .register_factory(name, Contract::new(name, 1), factory)
        .map_err(|(error, _, _)| error)
        .unwrap()
}

fn runtime() -> Runtime {
    let mut runtime = Runtime::new(151, Limits::default()).unwrap();
    runtime.start().unwrap();
    runtime
}

fn drive_until(runtime: &mut Runtime, mut done: impl FnMut(&Runtime) -> bool) {
    let mut wake = Context::from_waker(Waker::noop());
    for _ in 0..64 {
        if done(runtime) {
            return;
        }
        (Runtime::DRIVER.advance)(runtime, &mut wake).unwrap();
    }
    assert!(done(runtime), "expected lifecycle transition within 64 turns");
}

fn shutdown(runtime: &mut Runtime) {
    let result = runtime.close();
    assert!(result.is_ok() || result == Err(Error::BUSY));
    drive_until(runtime, |r| (Runtime::DRIVER.status)(r) == Lifecycle::Closed);
}

fn read_dependency(runtime: &mut Runtime) {
    let owner = runtime.resource("services/b").unwrap();
    let client = runtime.connect(&[(owner, Permissions::READ)]).unwrap();
    let local = client.resolve("services/b").into_inner().unwrap();
    let result = run_with(
        client.read(
            local,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.data.as_ref(), b"b");
}

#[test]
fn constructor_failure_rolls_back_new_dependency_and_retains_cleanup() {
    let mut runtime = runtime();
    let d = Rc::new(Control::default());
    let leaf = register(&mut runtime, "d", factory("d", &d, None), &[]);
    let b = Rc::new(Control::default());
    b.finish.set(false);
    let dependency = register(
        &mut runtime,
        "b",
        factory("b", &b, None),
        &[Contract::new("d", 1)],
    );
    let a = Rc::new(Control::default());
    a.fail_construct.set(true);
    let root = register(
        &mut runtime,
        "a",
        factory("a", &a, None),
        &[Contract::new("b", 1)],
    );
    let client = reovim_sdk::local(
        runtime
            .connect(&[
                (root, Permissions::ALL),
                (dependency, Permissions::ALL),
                (leaf, Permissions::ALL),
            ])
            .unwrap(),
    );
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    drive_until(&mut runtime, |_| b.closing.get());
    assert!(!b.destroyed.get());
    assert!(!d.closing.get());
    assert_eq!(runtime.resource("services/b"), Err(Error::UNAVAILABLE));
    b.finish.set(true);
    drive_until(&mut runtime, |_| d.destroyed.get());
    assert!(b.destroyed.get());
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    shutdown(&mut runtime);
}

#[test]
fn failed_driver_keeps_dependency_usable_until_its_state_is_destroyed() {
    let mut runtime = runtime();
    runtime
        .register("services/seed", reovim_resource_local::memory(b"seed"))
        .map_err(|(error, _)| error)
        .unwrap();
    let services = runtime.resource("services").unwrap();
    let imports = runtime.connect(&[(services, Permissions::READ)]).unwrap();
    let b = Rc::new(Control::default());
    let dependency = register(&mut runtime, "b", factory("b", &b, None), &[]);
    let a = Rc::new(Control::default());
    a.fail_driver.set(true);
    a.finish.set(false);
    let root = register(
        &mut runtime,
        "a",
        factory("a", &a, Some(imports)),
        &[Contract::new("b", 1)],
    );
    let client = runtime
        .connect(&[(root, Permissions::ALL), (dependency, Permissions::ALL)])
        .unwrap();
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    drive_until(&mut runtime, |_| a.checked_import.get());
    assert!(!a.destroyed.get());
    assert!(!b.closing.get());
    read_dependency(&mut runtime);
    a.finish.set(true);
    drive_until(&mut runtime, |_| b.destroyed.get());
    assert!(a.destroyed.get());
    shutdown(&mut runtime);
}

#[test]
fn explicitly_active_dependency_survives_failed_dependant() {
    let mut runtime = runtime();
    let b = Rc::new(Control::default());
    let dependency = register(&mut runtime, "b", factory("b", &b, None), &[]);
    let a = Rc::new(Control::default());
    a.fail_construct.set(true);
    let root = register(
        &mut runtime,
        "a",
        factory("a", &a, None),
        &[Contract::new("b", 1)],
    );
    let client = runtime
        .connect(&[(root, Permissions::ALL), (dependency, Permissions::ALL)])
        .unwrap();
    let local = client.resolve("b").into_inner().unwrap();
    run_with(module::activate(&client, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    read_dependency(&mut runtime);
    assert!(!b.closing.get());
    shutdown(&mut runtime);
}

#[test]
fn concurrent_success_preserves_shared_dependency_after_failure() {
    let mut runtime = runtime();
    let b = Rc::new(Control::default());
    b.ready.set(false);
    let dependency = register(&mut runtime, "b", factory("b", &b, None), &[]);
    let a = Rc::new(Control::default());
    a.fail_driver.set(true);
    let failed = register(
        &mut runtime,
        "a",
        factory("a", &a, None),
        &[Contract::new("b", 1)],
    );
    let c = Rc::new(Control::default());
    let success = register(
        &mut runtime,
        "c",
        factory("c", &c, None),
        &[Contract::new("b", 1)],
    );
    let client = runtime
        .connect(&[
            (failed, Permissions::ALL),
            (success, Permissions::ALL),
            (dependency, Permissions::ALL),
        ])
        .unwrap();
    let a_local = client.resolve("a").into_inner().unwrap();
    let c_local = client.resolve("c").into_inner().unwrap();
    let input = Message::new(Contract::new("kernel.module.activate", 1), &[]);
    let failure = client.write(a_local, &input);
    let success_call = client.write(c_local, &input);
    let path = format!("modules/instances/{}", success.0);
    drive_until(&mut runtime, |r| r.resource(&path).is_ok());
    assert_eq!(a.instance.get(), None);
    assert_eq!(c.instance.get(), None);
    b.ready.set(true);
    assert_eq!(
        run_with(failure, &mut runtime, &Runtime::DRIVER).unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    run_with(success_call, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    drive_until(&mut runtime, |_| a.destroyed.get());
    read_dependency(&mut runtime);
    assert!(!b.closing.get());
    assert_eq!(
        runtime.close_module(c.instance.get().unwrap()),
        Err(Error::BUSY)
    );
    drive_until(&mut runtime, |_| c.destroyed.get());
    read_dependency(&mut runtime);
    assert!(!b.closing.get());
    shutdown(&mut runtime);
}

#[test]
fn busy_provisional_dependency_remains_usable_until_retry_can_close_it() {
    let mut runtime = runtime();
    let b = Rc::new(Control::default());
    b.can_unload.set(false);
    let dependency = register(&mut runtime, "b", factory("b", &b, None), &[]);
    let a = Rc::new(Control::default());
    a.fail_construct.set(true);
    let root = register(
        &mut runtime,
        "a",
        factory("a", &a, None),
        &[Contract::new("b", 1)],
    );
    let client = runtime
        .connect(&[(root, Permissions::ALL), (dependency, Permissions::ALL)])
        .unwrap();
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    drive_until(&mut runtime, |_| b.unload_checked.get());
    assert!(!b.closing.get());
    assert!(!b.destroyed.get());
    read_dependency(&mut runtime);
    b.can_unload.set(true);
    drive_until(&mut runtime, |_| b.destroyed.get());
    assert_eq!(runtime.resource("services/b"), Err(Error::UNAVAILABLE));
    shutdown(&mut runtime);
}
