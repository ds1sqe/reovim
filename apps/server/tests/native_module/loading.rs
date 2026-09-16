use super::*;
use reovim_server::loading::{Loader, Target};
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

fn setup()
-> (Runtime, Rc<reovim_kernel_runtime::Binding>, ResourceId, ResourceId) {
    let mut runtime =
        reovim_server::assemble([0xb2; 16], Limits::default()).unwrap();
    let recipient = Rc::new(runtime.bind(&[]).unwrap());
    let registry = namespace(&mut runtime, b"/catalog");
    let mount = namespace(&mut runtime, b"/device");
    (runtime, recipient, registry, mount)
}
fn request(caller: &Caller, access: AccessId, path: &[u8]) -> Request {
    let mut input = u32::try_from(path.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(path);
    let request = Request::reserve(
        caller,
        Operation::Exec,
        access,
        0,
        None,
        Value::copy(&input, &[]).unwrap(),
    )
    .unwrap();
    request.submit().unwrap();
    request
}
fn finish(
    runtime: &mut Runtime,
    loader: &mut Loader,
    request: &Request,
) -> Rc<Outcome> {
    reovim_host_reference::run(request.outcome(), |cx| {
        loader.advance(runtime, cx);
        runtime.advance(cx).map(|_| ())
    })
    .unwrap()
    .unwrap()
}
fn package(outcome: &Outcome) -> ResourceId {
    let Outcome::Ok(value) = outcome else {
        panic!("load failed: {outcome:?}")
    };
    ResourceId {
        epoch: value.data[..16].try_into().unwrap(),
        serial: u64::from_le_bytes(value.data[16..24].try_into().unwrap()),
    }
}
fn loader(
    runtime: &mut Runtime,
    recipient: &Rc<reovim_kernel_runtime::Binding>,
    registry: ResourceId,
    mount: ResourceId,
    artifact: &Artifact,
    calls: &Rc<Cell<u32>>,
    capacity: usize,
) -> (Loader, AccessId) {
    let allowed = artifact.library.as_os_str().as_bytes().to_vec();
    let observed = calls.clone();
    let loader = Loader::new(
        Target {
            registry,
            namespace: mount,
            imports: Rc::new(runtime.bind(&[]).unwrap()),
            recipient: recipient.clone(),
            approved: Box::new([]),
        },
        Rc::new(move |path| {
            if path != allowed {
                return Err(Error::DENIED);
            }
            observed.set(observed.get() + 1);
            unsafe { native::load(Path::new(OsStr::from_bytes(path))) }
        }),
        capacity,
        ValueLimits { bytes: 8192, elements: 256, references: 8 },
    )
    .unwrap();
    let resource = runtime
        .register(b"/load", loader.provider().unwrap(), None)
        .ok()
        .unwrap();
    runtime
        .grant(
            recipient,
            Grant {
                resource,
                access: 5,
                create_access: 0,
                retain_loans: false,
            },
        )
        .unwrap();
    let (_, access, declaration) =
        discover_using(runtime, resource, recipient.caller());
    assert_eq!(declaration.api(), &[0xd3; 16]);
    (loader, access)
}

#[test]
fn declared_host_admission_retains_package_and_drives_activation_separately() {
    let (mut runtime, recipient, registry, mount) = setup();
    let caller = recipient.caller();
    let reserved = caller.reserve().unwrap();
    let mut artifact = Artifact::compile("module.c");
    let renamed = artifact.directory.join(OsStr::from_bytes(b"module-\xff.so"));
    std::fs::rename(&artifact.library, &renamed).unwrap();
    artifact.library = renamed;
    let path = artifact.library.as_os_str().as_bytes().to_vec();
    let calls = Rc::new(Cell::new(0));
    let (mut loader, access) = loader(
        &mut runtime,
        &recipient,
        registry,
        mount,
        &artifact,
        &calls,
        16,
    );
    let load = request(&caller, access, &path);
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(calls.get(), 0);
    let original = finish(&mut runtime, &mut loader, &load);
    let package = package(original.as_ref());
    assert_eq!(runtime.module_status(package), Err(Error::UNAVAILABLE));
    drop(artifact);
    load.submit().unwrap();
    assert_eq!(finish(&mut runtime, &mut loader, &load), original);
    load.adopt().unwrap();
    load.release().unwrap();
    let repeated = request(&caller, access, &path);
    assert_eq!(finish(&mut runtime, &mut loader, &repeated), original);
    assert_eq!(calls.get(), 1);
    repeated.adopt().unwrap();
    repeated.release().unwrap();
    assert_eq!(
        caller.snapshot(reserved),
        Ok(reovim_kernel_interface::caller::Status::Reserved)
    );
    caller.release(reserved).unwrap();
    let (_, activation, _) =
        discover_using(&mut runtime, package, caller.clone());
    let ready =
        invoke(&mut runtime, &caller, Operation::Exec, activation, 0, &[]);
    let instance = identity(&mut runtime, &caller, ready.references[0]);
    let first = runtime.module_exports(instance).unwrap()[0];
    assert_eq!(read_discovered(&mut runtime, first), 81);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

#[test]
fn failed_recipient_grant_retains_the_admitted_package_as_error_context() {
    let (mut runtime, recipient, registry, mount) = setup();
    let artifact = Artifact::compile("module.c");
    let calls = Rc::new(Cell::new(0));
    let (mut loader, access) = loader(
        &mut runtime,
        &recipient,
        registry,
        mount,
        &artifact,
        &calls,
        16,
    );
    let request = request(
        &recipient.caller(),
        access,
        artifact.library.as_os_str().as_bytes(),
    );
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.retire_binding(&recipient), Err(Error::BUSY));
    let completed = finish(&mut runtime, &mut loader, &request);
    let Outcome::Failed(error) = completed.as_ref() else {
        panic!("expected grant failure")
    };
    assert_eq!(error.code, Error::CLOSED.code);
    let package = error.id.expect("admitted package identity lost");
    let selected = runtime
        .select_module(registry, "fixture", 1, ModuleScope::Global)
        .unwrap();
    assert_eq!(selected.modules()[0].resource(), package);
    assert_eq!(calls.get(), 1);
    request.submit().unwrap();
    assert_eq!(finish(&mut runtime, &mut loader, &request), completed);
    request.release().unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&recipient).unwrap();
    drop(loader);
    runtime.close().unwrap();
}

#[test]
fn full_loader_reuses_captured_package_without_reentering_artifact_policy() {
    let (mut runtime, recipient, registry, mount) = setup();
    let artifact = Artifact::compile("module.c");
    let path = artifact.library.as_os_str().as_bytes().to_vec();
    let calls = Rc::new(Cell::new(0));
    let (mut loader, access) =
        loader(&mut runtime, &recipient, registry, mount, &artifact, &calls, 1);
    let first = request(&recipient.caller(), access, &path);
    let original = finish(&mut runtime, &mut loader, &first);
    package(original.as_ref());
    first.adopt().unwrap();
    first.release().unwrap();
    drop(artifact);
    let rejected = request(&recipient.caller(), access, b"/another-artifact");
    assert_eq!(
        finish(&mut runtime, &mut loader, &rejected).as_ref(),
        &Outcome::Failed(Error::PRESSURE)
    );
    rejected.release().unwrap();
    let repeated = request(&recipient.caller(), access, &path);
    assert_eq!(finish(&mut runtime, &mut loader, &repeated), original);
    assert_eq!(calls.get(), 1);
    repeated.adopt().unwrap();
    repeated.release().unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
