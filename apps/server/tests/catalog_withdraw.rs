use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Factory, Module},
    provider::{Contribution, Permissions},
    requests,
};
use reovim_kernel_runtime::{Limits, Runtime, diagnostics::Stage};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

struct Control {
    instance: Cell<Option<ResourceId>>,
    status: Cell<Lifecycle>,
    can_stop: Cell<bool>,
    state_dropped: Cell<bool>,
    code_dropped: Cell<bool>,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            instance: Cell::new(None),
            status: Cell::new(Lifecycle::Constructed),
            can_stop: Cell::new(true),
            state_dropped: Cell::new(false),
            code_dropped: Cell::new(false),
        }
    }
}
struct State(Rc<Control>);
impl Drop for State {
    fn drop(&mut self) {
        self.0.state_dropped.set(true);
    }
}
struct Code {
    control: Rc<Control>,
    kernel: Connection,
}
impl Drop for Code {
    fn drop(&mut self) {
        // Native code release can re-enter imports; no registry guard is held.
        self.kernel.root().into_inner().unwrap();
        assert!(
            self.control.instance.get().is_none()
                || self.control.state_dropped.get()
        );
        self.control.code_dropped.set(true);
    }
}
fn advance(state: &State) -> Progress {
    if state.0.status.get() == Lifecycle::Closing {
        if !state.0.can_stop.get() {
            return Progress::Idle;
        }
        state.0.status.set(Lifecycle::Closed);
        return Progress::Stopped;
    }
    state.0.status.set(Lifecycle::Ready);
    Progress::Idle
}
fn register(
    runtime: &mut Runtime,
    version: u32,
    value: &'static [u8],
    control: &Rc<Control>,
) -> ResourceId {
    let kernel = runtime.connect(&[]).unwrap();
    let factory = Factory::new(
        (Rc::clone(control), value),
        |(control, value), id| {
            assert_eq!(control.instance.replace(Some(id)), None);
            Ok(Module::new(
                State(Rc::clone(control)),
                DriverOps {
                    status: |s| s.0.status.get(),
                    advance: |s, _| Ok(advance(s)),
                    close: |s| {
                        s.0.status.set(Lifecycle::Closing);
                        Ok(())
                    },
                },
                vec![Contribution::Resource {
                    path: "service".into(),
                    provider: reovim_resource_local::memory(value),
                }],
                Rc::new(()),
            ))
        },
        Rc::new(Code { control: Rc::clone(control), kernel }),
    );
    runtime
        .register_factory("catalog", Contract::new("example", version), factory)
        .map_err(|(error, _, _)| error)
        .unwrap()
}
fn setup() -> (Runtime, Connection, ResourceId, Rc<Control>) {
    let mut runtime = Runtime::new(171, Limits::default()).unwrap();
    runtime.start().unwrap();
    let control = Rc::new(Control::default());
    let catalog = register(&mut runtime, 1, b"old", &control);
    let root = runtime.resource("").unwrap();
    let admin = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    (runtime, admin, catalog, control)
}
fn drive_until(runtime: &mut Runtime, done: impl Fn(&Runtime) -> bool) {
    let mut wake = Context::from_waker(Waker::noop());
    for _ in 0..64 {
        if done(runtime) {
            return;
        }
        runtime.advance(&mut wake).unwrap();
    }
    assert!(done(runtime), "expected transition within 64 turns");
}
fn empty(name: &str) -> Message {
    Message::new(Contract::new(name, 1), &[])
}

#[test]
fn withdrawal_releases_code_and_queued_requests_keep_the_old_identity() {
    let (mut runtime, admin, _, control) = setup();
    let local = admin.resolve("catalog").into_inner().unwrap();
    let withdrawal = admin.write(local, &empty("kernel.module.withdraw"));
    let info = admin.read(local, &empty("kernel.module.info"));
    let activation = admin.write(local, &empty("kernel.module.activate"));
    let joined = admin.write(local, &empty("kernel.module.withdraw"));
    drive_until(&mut runtime, |_| control.code_dropped.get());
    assert_eq!(control.instance.get(), None);
    let replacement = Rc::new(Control::default());
    let new_id = register(&mut runtime, 2, b"new", &replacement);
    let metadata =
        run_with(info, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert_eq!(u32::from_le_bytes(metadata.data[..4].try_into().unwrap()), 1);
    assert_eq!(&metadata.data[4..], b"example");
    assert_eq!(
        run_with(activation, &mut runtime, &Runtime::DRIVER).unwrap(),
        Err(RequestError::Failed(Error::UNAVAILABLE))
    );
    assert_eq!(
        run_with(joined, &mut runtime, &Runtime::DRIVER).unwrap().unwrap(),
        empty("kernel.module.withdraw")
    );
    run_with(withdrawal, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert_eq!(runtime.resource("catalog"), Ok(new_id));
    assert_eq!(replacement.instance.get(), None);
    assert_eq!(
        run_with(
            module::activate(&admin, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    let new_local = admin.resolve("catalog").into_inner().unwrap();
    assert_ne!(new_local, local);
    run_with(
        module::withdraw(&admin, new_local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert!(replacement.code_dropped.get());
    runtime.close().unwrap();
}

#[test]
fn withdrawal_is_busy_until_selected_state_finishes_cleanup() {
    let (mut runtime, admin, _, control) = setup();
    let catalog = admin.resolve("catalog").into_inner().unwrap();
    let instance = run_with(
        module::activate(&admin, catalog),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        run_with(
            module::withdraw(&admin, catalog),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::BUSY))
    );
    assert_eq!(control.status.get(), Lifecycle::Ready);
    control.can_stop.set(false);
    let closing = admin.write(instance, &empty("kernel.module.unload"));
    drive_until(&mut runtime, |_| control.status.get() == Lifecycle::Closing);
    assert_eq!(
        run_with(
            module::withdraw(&admin, catalog),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::BUSY))
    );
    assert!(!control.code_dropped.get());
    control.can_stop.set(true);
    run_with(closing, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert!(control.state_dropped.get());
    assert!(!control.code_dropped.get());
    run_with(module::withdraw(&admin, catalog), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(control.code_dropped.get());
    runtime.close().unwrap();
}

#[test]
fn denied_and_malformed_withdrawal_preserve_the_factory() {
    let (mut runtime, admin, catalog, control) = setup();
    let observer = runtime.connect(&[(catalog, Permissions::READ)]).unwrap();
    let local = observer.resolve("catalog").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::withdraw(&observer, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let local = admin.resolve("catalog").into_inner().unwrap();
    assert_eq!(
        run_with(
            admin.write(
                local,
                &Message::new(
                    Contract::new("kernel.module.withdraw", 1),
                    b"bad"
                )
            ),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::INVALID))
    );
    assert_eq!(runtime.resource("catalog"), Ok(catalog));
    assert_eq!(control.instance.get(), None);
    assert!(!control.code_dropped.get());
    run_with(module::withdraw(&admin, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    runtime.close().unwrap();
}

#[test]
fn replacement_cannot_retarget_recovery_old_instances_or_exact_grants() {
    let (mut runtime, admin, catalog, control) = setup();
    let local = admin.resolve("catalog").into_inner().unwrap();
    let old_instance = run_with(
        module::activate(&admin, local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    let scoped = runtime.connect(&[(catalog, Permissions::ALL)]).unwrap();
    let old_local = scoped.resolve("catalog").into_inner().unwrap();
    let after = runtime.events(0).last().unwrap().sequence;
    let pending = scoped.write(old_local, &empty("kernel.module.activate"));
    drive_until(&mut runtime, |r| {
        r.events(after).iter().any(|event| {
            event.subject == catalog.0 && event.stage == Stage::Finished
        })
    });
    run_with(
        module::unload(&admin, old_instance),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(module::withdraw(&admin, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(control.code_dropped.get());
    let replacement = Rc::new(Control::default());
    let new_catalog = register(&mut runtime, 2, b"new", &replacement);
    assert_ne!(new_catalog, catalog);
    let new_local = admin.resolve("catalog").into_inner().unwrap();
    let new_instance = run_with(
        module::activate(&admin, new_local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_ne!(new_instance, old_instance);
    let next = scoped.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        run_with(pending, &mut runtime, &Runtime::DRIVER).unwrap()
    else {
        panic!("original activation must remain recoverable");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let recovered =
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap()
            .unwrap();
    next.close(work).into_inner().unwrap();
    assert_eq!(
        run_with(
            module::ready(&next, recovered.resources[0]),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::CLOSED))
    );
    assert_eq!(
        next.resolve("catalog").into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    run_with(
        module::unload(&admin, old_instance),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    let service = admin.resolve("service").into_inner().unwrap();
    let value = run_with(
        admin.read(service, &empty("resource.bytes")),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(value.data.as_ref(), b"new");
    assert!(!replacement.state_dropped.get());
    run_with(
        module::unload(&admin, new_instance),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(
        module::withdraw(&admin, new_local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn withdrawal_recovery_cannot_remove_a_new_registration() {
    let (mut runtime, admin, _, control) = setup();
    let local = admin.resolve("catalog").into_inner().unwrap();
    let pending = admin.write(local, &empty("kernel.module.withdraw"));
    drive_until(&mut runtime, |_| control.code_dropped.get());
    let replacement = Rc::new(Control::default());
    let new_id = register(&mut runtime, 2, b"new", &replacement);
    let next = admin.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        run_with(pending, &mut runtime, &Runtime::DRIVER).unwrap()
    else {
        panic!("withdrawal result must remain recoverable");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let recovered =
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap()
            .unwrap();
    next.close(work).into_inner().unwrap();
    assert_eq!(recovered, empty("kernel.module.withdraw"));
    assert_eq!(runtime.resource("catalog"), Ok(new_id));
    assert!(!replacement.code_dropped.get());
    let local = next.resolve("catalog").into_inner().unwrap();
    run_with(module::withdraw(&next, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    runtime.close().unwrap();
}

#[test]
fn cancelled_withdrawal_leaves_the_factory_available_for_activation() {
    let (mut runtime, admin, catalog, control) = setup();
    let local = admin.resolve("catalog").into_inner().unwrap();
    let pending = admin.write(local, &empty("kernel.module.withdraw"));
    let next = admin.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        run_with(pending, &mut runtime, &Runtime::DRIVER).unwrap()
    else {
        panic!("queued withdrawal must have a retained request");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    run_with(
        next.write(work, &empty("kernel.request.cancel")),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap(),
        Err(RequestError::Failed(Error::CANCELLED))
    );
    next.close(work).into_inner().unwrap();
    assert_eq!(runtime.resource("catalog"), Ok(catalog));
    assert!(!control.code_dropped.get());
    let local = next.resolve("catalog").into_inner().unwrap();
    let instance = run_with(
        module::activate(&next, local),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(module::unload(&next, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    run_with(module::withdraw(&next, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(control.state_dropped.get());
    assert!(control.code_dropped.get());
    runtime.close().unwrap();
}
