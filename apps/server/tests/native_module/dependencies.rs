use super::*;
use reovim_kernel_interface::module::control;
use std::cell::Cell;

#[path = "service.rs"]
mod service;

fn namespace(runtime: &mut Runtime, path: &[u8]) -> ResourceId {
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
    runtime.register(path, provider, None).ok().unwrap()
}
struct Loaded {
    package: ResourceId,
    producer: Rc<Cell<Option<ResourceId>>>,
    consumer: Rc<Cell<Option<ResourceId>>>,
}
fn observed_definition(
    module: &reovim_sdk::NativeModule,
    observed: &Rc<Cell<Option<ResourceId>>>,
) -> Rc<reovim_kernel_interface::module::Definition> {
    let limits = ValueLimits { bytes: 8192, elements: 256, references: 8 };
    let definition = module.definition(limits, 16).unwrap();
    let observed = observed.clone();
    Rc::new(
        reovim_kernel_interface::module::Definition::new(
            definition.declaration().clone(),
            definition.required().into(),
            definition.exports(),
            Rc::new(move |context| {
                observed.set(Some(context.instance));
                definition.construct(context)
            }),
        )
        .unwrap(),
    )
}
fn packages(runtime: &mut Runtime, approved: bool) -> Loaded {
    let registry = namespace(runtime, b"/catalog");
    let producer_mount = namespace(runtime, b"/producer");
    let consumer_mount = namespace(runtime, b"/consumer");
    let imports = runtime.bind(&[]).unwrap();
    let producer_artifact = Artifact::compile("module.c");
    let consumer_artifact = Artifact::compile("dependency_consumer.c");
    let producer = unsafe { native::load(&producer_artifact.library) }.unwrap();
    let consumer = unsafe { native::load(&consumer_artifact.library) }.unwrap();
    let producer_instance = Rc::new(Cell::new(None));
    let consumer_instance = Rc::new(Cell::new(None));
    runtime
        .register_module(
            registry,
            producer_mount,
            observed_definition(&producer, &producer_instance),
            &imports,
            &[],
        )
        .unwrap();
    let package = runtime
        .register_module(
            registry,
            consumer_mount,
            observed_definition(&consumer, &consumer_instance),
            &imports,
            if approved { &["input"] } else { &[] },
        )
        .unwrap();
    drop(producer);
    drop(consumer);
    drop(producer_artifact);
    drop(consumer_artifact);
    Loaded { package, producer: producer_instance, consumer: consumer_instance }
}
fn result(runtime: &mut Runtime, request: &Request) -> Rc<Outcome> {
    reovim_host_reference::run(request.outcome(), |cx| {
        runtime.advance(cx).map(|_| ())
    })
    .unwrap()
    .unwrap()
}
fn identity(
    runtime: &mut Runtime,
    caller: &Caller,
    access: AccessId,
) -> ResourceId {
    let info = invoke(runtime, caller, Operation::Info, access, 0, &[]);
    ResourceId {
        epoch: info.data[..16].try_into().unwrap(),
        serial: u64::from_le_bytes(info.data[16..24].try_into().unwrap()),
    }
}
#[test]
fn native_consumer_uses_granted_dependency_after_both_artifacts_are_removed() {
    let mut runtime =
        reovim_server::assemble([0x95; 16], Limits::default()).unwrap();
    let package = packages(&mut runtime, true);
    let (caller, activation, _) = discover(&mut runtime, package.package, 0);
    let request = Request::reserve(
        &caller,
        Operation::Exec,
        activation,
        0,
        None,
        Value::unit(),
    )
    .unwrap();
    request.submit().unwrap();
    let completed = result(&mut runtime, &request);
    let Outcome::Ok(value) = completed.as_ref() else {
        panic!("native consumer activation failed");
    };
    let access = value.references[0];
    // Resubmission recovers the same result without running either constructor.
    request.submit().unwrap();
    assert_eq!(result(&mut runtime, &request), completed);
    request.adopt().unwrap();
    request.release().unwrap();
    let instance = identity(&mut runtime, &caller, access);
    let export = runtime.module_exports(instance).unwrap()[0];
    let binding = runtime
        .bind(&[Grant {
            resource: export,
            access: 5,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap();
    let (reader, bound, declaration) =
        discover_using(&mut runtime, export, binding.caller());
    let command = declaration
        .methods()
        .iter()
        .find_map(|method| match method.operation() {
            Declared::Ioctl(command) => Some(command),
            Declared::Exec => None,
        })
        .unwrap();
    let value =
        invoke(&mut runtime, &reader, Operation::Ioctl, bound, command, &[]);
    assert_eq!(u64::from_le_bytes(value.data.as_ref().try_into().unwrap()), 81);
    let unload = Request::reserve(
        &caller,
        Operation::Ioctl,
        access,
        control::UNLOAD,
        None,
        Value::unit(),
    )
    .unwrap();
    unload.submit().unwrap();
    assert_eq!(
        result(&mut runtime, &unload).as_ref(),
        &Outcome::Failed(Error::BUSY)
    );
    unload.release().unwrap();
    reconcile_add(&mut runtime, &binding, &reader, bound, instance);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&binding).unwrap();
    invoke(
        &mut runtime,
        &caller,
        Operation::Ioctl,
        access,
        control::UNLOAD,
        &[],
    );
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Closed));
    let producer = package.producer.get().unwrap();
    assert_eq!(runtime.module_status(producer), Ok(Lifecycle::Ready));
    let original = runtime.module_exports(producer).unwrap()[0];
    assert_eq!(read_discovered(&mut runtime, original), 81);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
fn reconcile_add(
    runtime: &mut Runtime,
    binding: &reovim_kernel_runtime::Binding,
    caller: &Caller,
    access: AccessId,
    instance: ResourceId,
) {
    let request = Request::reserve(
        caller,
        Operation::Exec,
        access,
        0,
        None,
        Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
    )
    .unwrap();
    request.submit().unwrap();
    let uncertain = result(runtime, &request);
    let Outcome::Unknown { progress: Some(value), .. } = uncertain.as_ref()
    else {
        panic!("missing original mutation evidence");
    };
    assert_eq!(u64::from_le_bytes(value.data.as_ref().try_into().unwrap()), 86);
    assert_eq!(runtime.retire_binding(binding), Err(Error::INCOMPLETE));
    assert_eq!(runtime.unload_module(instance), Err(Error::BUSY));
    request.reconcile().unwrap();
    let completed = result(runtime, &request);
    let Outcome::Ok(value) = completed.as_ref() else {
        panic!("reconciliation failed");
    };
    assert_eq!(u64::from_le_bytes(value.data.as_ref().try_into().unwrap()), 86);
    request.submit().unwrap();
    assert_eq!(result(runtime, &request), completed);
    request.adopt().unwrap();
    request.release().unwrap();
}

#[test]
fn native_dependency_activation_requires_explicit_import_approval() {
    let mut runtime =
        reovim_server::assemble([0x96; 16], Limits::default()).unwrap();
    let package = packages(&mut runtime, false);
    let (caller, bound, _) = discover(&mut runtime, package.package, 0);
    let request = Request::reserve(
        &caller,
        Operation::Exec,
        bound,
        0,
        None,
        Value::unit(),
    )
    .unwrap();
    request.submit().unwrap();
    assert_eq!(
        result(&mut runtime, &request).as_ref(),
        &Outcome::Failed(Error::DENIED)
    );
    assert_eq!(package.producer.get(), None);
    assert_eq!(package.consumer.get(), None);
    request.release().unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

#[test]
fn cancelling_native_startup_drains_import_views_before_dependency_teardown() {
    use reovim_kernel_interface::call::Cancellation;
    let mut runtime =
        reovim_server::assemble([0x97; 16], Limits::default()).unwrap();
    let loaded = packages(&mut runtime, true);
    let (caller, bound, _) = discover(&mut runtime, loaded.package, 0);
    let request = Request::reserve(
        &caller,
        Operation::Exec,
        bound,
        0,
        None,
        Value::unit(),
    )
    .unwrap();
    request.submit().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..16 {
        runtime.advance(cx).unwrap();
        if loaded.consumer.get().is_some() {
            break;
        }
    }
    let consumer = loaded.consumer.get().expect("consumer was not constructed");
    assert_eq!(runtime.module_status(consumer), Ok(Lifecycle::Starting));
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_status(consumer), Ok(Lifecycle::Starting));
    assert_eq!(request.cancel(), Ok(Cancellation::Accepted));
    assert_eq!(
        result(&mut runtime, &request).as_ref(),
        &Outcome::Failed(Error::CANCELLED)
    );
    assert_eq!(runtime.module_status(consumer), Ok(Lifecycle::Closed));
    assert_eq!(
        runtime.module_status(loaded.producer.get().unwrap()),
        Ok(Lifecycle::Closed)
    );
    request.release().unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

#[path = "loading.rs"]
mod loading;
