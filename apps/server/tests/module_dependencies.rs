use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Contract, Error, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Factory, Module},
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::RefCell,
    rc::Rc,
    task::{Context, Waker},
};

type Trace = Rc<RefCell<Vec<(&'static str, ResourceId)>>>;

const fn advance(state: &mut Lifecycle) -> Progress {
    if matches!(*state, Lifecycle::Closing) {
        *state = Lifecycle::Closed;
        Progress::Stopped
    } else {
        *state = Lifecycle::Ready;
        Progress::Idle
    }
}

fn factory(
    name: &'static str,
    trace: &Trace,
    dependencies: &[Contract],
) -> Factory {
    let mut factory = Factory::new(
        (name, Rc::clone(trace)),
        |(name, trace), id| {
            trace.borrow_mut().push((name, id));
            Ok(Module::new(
                Lifecycle::Constructed,
                DriverOps {
                    status: |s| *s,
                    advance: |s, _| Ok(advance(s)),
                    close: |s| {
                        *s = Lifecycle::Closing;
                        Ok(())
                    },
                },
                vec![Contribution::Resource {
                    path: format!("services/{name}").into(),
                    provider: reovim_resource_local::memory(name.as_bytes()),
                }],
                Rc::new(()),
            ))
        },
        Rc::new(()),
    );
    factory.dependencies = dependencies.into();
    factory
}

fn register(
    runtime: &mut Runtime,
    name: &'static str,
    trace: &Trace,
    dependencies: &[Contract],
) -> ResourceId {
    runtime
        .register_factory(
            name,
            Contract::new(name, 1),
            factory(name, trace, dependencies),
        )
        .map_err(|(error, _, _)| error)
        .unwrap()
}

#[test]
fn dependency_is_ready_before_dependant_and_retained_until_cleanup() {
    let mut runtime = Runtime::new(141, Limits::default()).unwrap();
    runtime.start().unwrap();
    let trace = Trace::default();
    let b = register(&mut runtime, "b", &trace, &[]);
    runtime
        .register("services/seed", reovim_resource_local::memory(b"seed"))
        .map_err(|(error, _)| error)
        .unwrap();
    let services = runtime.resource("services").unwrap();
    let imports = runtime.connect(&[(services, Permissions::READ)]).unwrap();
    let mut dependant = Factory::new(
        (imports, factory("a", &trace, &[])),
        |(imports, factory), id| {
            // Publication occurs only once the dependency is Ready.
            imports
                .resolve("services/b")
                .into_inner()
                .expect("dependency must publish before construction");
            factory.construct(id)
        },
        Rc::new(()),
    );
    dependant.dependencies = Box::new([Contract::new("b", 1)]);
    let a = runtime
        .register_factory("a", Contract::new("a", 1), dependant)
        .map_err(|(error, _, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[(a, Permissions::ALL), (b, Permissions::ALL)])
            .unwrap(),
    );
    let local = client.resolve("a").into_inner().unwrap();
    run_with(module::activate(&client, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    let duplicate = runtime
        .register_factory(
            "another-b",
            Contract::new("b", 1),
            factory("unused", &trace, &[]),
        )
        .map_err(|(error, _, _)| error)
        .unwrap();
    let later = runtime
        .connect(&[
            (a, Permissions::ALL),
            (b, Permissions::ALL),
            (duplicate, Permissions::ALL),
        ])
        .unwrap();
    let same = later.resolve("a").into_inner().unwrap();
    run_with(module::activate(&later, same), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    let dependency = client.resolve("b").into_inner().unwrap();
    run_with(
        module::activate(&client, dependency),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(module::activate(&client, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    let trace = trace.borrow();
    let [("b", b), ("a", a)] = trace.as_slice() else {
        panic!("dependency must construct first, exactly once");
    };
    let export = runtime.resource("services/b").unwrap();
    let reader = runtime.connect(&[(export, Permissions::READ)]).unwrap();
    let subject = reader.resolve("services/b").into_inner().unwrap();
    let reply = run_with(
        reader.read(
            subject,
            &reovim_kernel_interface::Message::new(
                Contract::new("resource.bytes", 1),
                &[],
            ),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"b");
    assert_eq!(runtime.close_module(*b), Err(Error::BUSY));
    assert_eq!(runtime.close_module(*a), Err(Error::BUSY));
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    assert_eq!(runtime.close_module(*b), Err(Error::BUSY));
    runtime.advance(&mut wake).unwrap();
    runtime.close_module(*b).unwrap();
    runtime.close().unwrap();
}

#[test]
fn invalid_graphs_execute_no_constructors() {
    for (required, grant, cycle, expected) in [
        (Contract::new("missing", 1), true, false, Error::UNAVAILABLE),
        (Contract::new("b", 2), true, false, Error::UNSUPPORTED),
        (Contract::new("b", 1), false, false, Error::DENIED),
        (Contract::new("b", 1), true, true, Error::INVALID),
    ] {
        rejected_graph(required, grant, cycle, expected);
    }
}

fn rejected_graph(
    required: Contract,
    grant: bool,
    cycle: bool,
    expected: Error,
) {
    let mut runtime = Runtime::new(142, Limits::default()).unwrap();
    runtime.start().unwrap();
    let trace = Trace::default();
    let dependencies = if cycle { vec![Contract::new("a", 1)] } else { vec![] };
    let b = register(&mut runtime, "b", &trace, &dependencies);
    let a = register(&mut runtime, "a", &trace, &[required]);
    let rights = if grant { Permissions::ALL } else { Permissions::READ };
    let client =
        runtime.connect(&[(a, Permissions::ALL), (b, rights)]).unwrap();
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(expected))
    );
    assert!(trace.borrow().is_empty());
    assert_eq!(runtime.resource("modules/instances"), Err(Error::UNAVAILABLE));
    runtime.close().unwrap();
}

#[test]
fn partial_selection_pressure_rolls_back_and_retry_succeeds() {
    let limits = Limits { resources: 16, ..Limits::default() };
    let mut runtime = Runtime::new(143, limits).unwrap();
    runtime.start().unwrap();
    let trace = Trace::default();
    let b = register(&mut runtime, "b", &trace, &[]);
    let a = register(&mut runtime, "a", &trace, &[Contract::new("b", 1)]);
    let mut blockers = Vec::new();
    for name in ["v", "w", "x", "y", "z"] {
        blockers.push(
            runtime
                .register(name, reovim_resource_local::memory(b"held"))
                .map_err(|(error, _)| error)
                .unwrap(),
        );
    }
    let client = runtime
        .connect(&[(a, Permissions::ALL), (b, Permissions::ALL)])
        .unwrap();
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PRESSURE))
    );
    assert!(trace.borrow().is_empty());
    assert_eq!(runtime.resource("modules"), Err(Error::UNAVAILABLE));
    runtime.unregister_contributions(&blockers).unwrap();
    run_with(module::activate(&client, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    let mut wake = Context::from_waker(Waker::noop());
    for _ in 0..4 {
        (Runtime::DRIVER.advance)(&mut runtime, &mut wake).unwrap();
    }
    runtime.close().unwrap();
}

#[test]
fn failed_dependency_never_constructs_dependant() {
    let mut runtime = Runtime::new(144, Limits::default()).unwrap();
    runtime.start().unwrap();
    let trace = Trace::default();
    let b = runtime
        .register_factory(
            "b",
            Contract::new("b", 1),
            Factory::new((), |(), _| Err(Error::PROVIDER), Rc::new(())),
        )
        .map_err(|(error, _, _)| error)
        .unwrap();
    let a = register(&mut runtime, "a", &trace, &[Contract::new("b", 1)]);
    let client = runtime
        .connect(&[(a, Permissions::ALL), (b, Permissions::ALL)])
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
    assert!(trace.borrow().is_empty());
    runtime.close().unwrap();
}

#[test]
fn ambiguous_authorized_dependency_rejects_before_construction() {
    let mut runtime = Runtime::new(145, Limits::default()).unwrap();
    runtime.start().unwrap();
    let trace = Trace::default();
    let first = register(&mut runtime, "b", &trace, &[]);
    let second = runtime
        .register_factory(
            "other",
            Contract::new("b", 1),
            factory("other", &trace, &[]),
        )
        .map_err(|(error, _, _)| error)
        .unwrap();
    let a = register(&mut runtime, "a", &trace, &[Contract::new("b", 1)]);
    let client = runtime
        .connect(&[
            (a, Permissions::ALL),
            (first, Permissions::ALL),
            (second, Permissions::ALL),
        ])
        .unwrap();
    let local = client.resolve("a").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::activate(&client, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::INVALID))
    );
    assert!(trace.borrow().is_empty());
    runtime.close().unwrap();
}
