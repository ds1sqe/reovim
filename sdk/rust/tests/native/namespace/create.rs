use super::*;

#[path = "created_open.rs"]
mod opened;

fn creator(runtime: &mut Runtime, directory: ResourceId) -> (Caller, AccessId) {
    creator_with(runtime, directory, 3)
}
fn creator_with(
    runtime: &mut Runtime,
    directory: ResourceId,
    ceiling: u32,
) -> (Caller, AccessId) {
    let mut permission = grant(directory, 3);
    permission.create_access = ceiling;
    let caller = runtime.bind(&[permission]).unwrap().caller();
    let parent = open(runtime, &caller, directory);
    query(runtime, &caller, Operation::SetAccess, parent, &3_u32.to_le_bytes())
        .unwrap();
    (caller, parent)
}
fn create(caller: &Caller, parent: AccessId, name: &[u8]) -> Call {
    let mut data = u32::try_from(name.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(name);
    Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Create,
        subject: parent,
        command: 0,
        expected: None,
        input: Value::copy(&data, &[]).unwrap(),
    }
}
fn id(value: &Value) -> ResourceId {
    ResourceId {
        epoch: value.data[..16].try_into().unwrap(),
        serial: u64::from_le_bytes(value.data[16..24].try_into().unwrap()),
    }
}
fn close(mut runtime: Runtime) {
    for _ in 0..16 {
        match runtime.close() {
            Ok(()) => return,
            Err(error) => assert_eq!(error, Error::BUSY),
        }
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    panic!("creation cleanup did not drain")
}
#[test]
fn create_installs_one_identity_for_returned_access_listing_and_reopen() {
    let (mut runtime, library, observer, directory) = installed();
    let (caller, parent) = creator(&mut runtime, directory);
    let call = create(&caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    let created = completed(&mut runtime, &caller, call.request).unwrap();
    let info = query(
        &mut runtime,
        &caller,
        Operation::Info,
        created.references[0],
        &[],
    )
    .unwrap();
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 3);
    let rows = ids(&entries(&mut runtime, &caller, parent, 192).unwrap());
    assert_eq!(rows.last(), Some(&(b"new".to_vec(), id(&created))));
    let reopened = open(&mut runtime, &observer, id(&created));
    assert_eq!(read(&mut runtime, &observer, reopened), 0);
    assert_eq!(read(&mut runtime, &caller, created.references[0]), 0);
    assert_eq!(library.count(c"fixture_materializations"), 0);
    assert_eq!(library.count(c"fixture_creations"), 1);
    close(runtime);
}
#[test]
fn unknown_create_blocks_competing_materialization_and_recovers_same_identity()
{
    let (mut runtime, library, observer, directory) = installed();
    let (caller, parent) = creator(&mut runtime, directory);
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_create_unknown") };
    unsafe { uncertain() };
    let call = create(&caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_unknown(caller.snapshot(call.request).unwrap());
    let rows = ids(&entries(&mut runtime, &caller, parent, 192).unwrap());
    let identity = rows.last().unwrap().1;
    let competing = acquisition(&observer, identity);
    observer.submit(competing.clone()).unwrap();
    assert_eq!(
        completed(&mut runtime, &observer, competing.request),
        Err(Error::BUSY)
    );
    assert_eq!(library.count(c"fixture_materializations"), 0);
    caller.submit(call.clone()).unwrap();
    caller.reconcile(call.request).unwrap();
    let created = completed(&mut runtime, &caller, call.request).unwrap();
    assert_eq!(id(&created), identity);
    let reopened = open(&mut runtime, &observer, identity);
    assert_eq!(read(&mut runtime, &observer, reopened), 0);
    assert_eq!(library.count(c"fixture_creations"), 1);
    close(runtime);
}
#[test]
fn create_checks_grants_names_and_capacity_before_provider_effects() {
    let (mut runtime, library, observer, directory) = installed();
    let parent = open(&mut runtime, &observer, directory);
    let call = create(&observer, parent, b"new");
    assert_eq!(observer.submit(call.clone()), Err(Error::DENIED));
    observer.release(call.request).unwrap();
    let (caller, parent) = creator(&mut runtime, directory);
    let invalid = create(&caller, parent, b"../new");
    assert_eq!(caller.submit(invalid.clone()), Err(Error::INVALID));
    caller.release(invalid.request).unwrap();
    let _a = open(&mut runtime, &caller, directory);
    let _b = open(&mut runtime, &caller, directory);
    let full = create(&caller, parent, b"new");
    assert_eq!(caller.submit(full.clone()), Err(Error::PRESSURE));
    caller.release(full.request).unwrap();
    assert_eq!(library.count(c"fixture_creations"), 0);
    assert!(
        !ids(&entries(&mut runtime, &caller, parent, 192).unwrap())
            .iter()
            .any(|(name, _)| name == b"new")
    );
    close(runtime);
}
#[test]
fn cancellation_releases_creation_reservations_without_publishing() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = creator(&mut runtime, directory);
    for _ in 0..4 {
        let call = create(&caller, parent, b"new");
        caller.submit(call.clone()).unwrap();
        assert_eq!(
            caller.cancel(call.request).unwrap(),
            reovim_kernel_interface::call::Cancellation::Accepted
        );
        assert_eq!(
            completed(&mut runtime, &caller, call.request),
            Err(Error::CANCELLED)
        );
    }
    assert_eq!(library.count(c"fixture_creations"), 0);
    let call = create(&caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    completed(&mut runtime, &caller, call.request).unwrap();
    close(runtime);
}
#[test]
fn conflicting_key_staging_cannot_publish_or_replace_existing_identity() {
    let (mut runtime, library, observer, directory) = installed();
    let existing = subjects(&mut runtime, &observer, directory);
    let (caller, parent) = creator(&mut runtime, directory);
    let conflict: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_create_conflict") };
    unsafe { conflict() };
    let call = create(&caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    assert_eq!(
        completed(&mut runtime, &caller, call.request),
        Err(Error::CONFLICT)
    );
    let rows = ids(&entries(&mut runtime, &caller, parent, 192).unwrap());
    assert_eq!(
        rows,
        vec![(b"disk".to_vec(), existing), (b"alias".to_vec(), existing)]
    );
    assert_eq!(library.count(c"fixture_creations"), 0);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    close(runtime);
}

#[test]
fn resource_capacity_rejects_before_namespace_creation() {
    let library = Library::compile_source("namespace.c");
    let mut runtime =
        Runtime::new([0x86; 16], Limits { resources: 1, ..Limits::default() })
            .unwrap();
    let directory = mounted(&mut runtime, &library, b"/device");
    runtime.start().unwrap();
    let (caller, parent) = creator(&mut runtime, directory);
    let call = create(&caller, parent, b"new");
    assert_eq!(caller.submit(call.clone()), Err(Error::PRESSURE));
    caller.release(call.request).unwrap();
    assert_eq!(library.count(c"fixture_creations"), 0);
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    close(runtime);
}
#[test]
fn admitted_creation_settles_after_runtime_stops_new_admission() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = creator(&mut runtime, directory);
    let call = create(&caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(library.count(c"fixture_creations"), 1);
    assert_eq!(runtime.close(), Err(Error::BUSY));
    completed(&mut runtime, &caller, call.request).unwrap();
    close(runtime);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
#[test]
fn staged_creation_reserves_its_canonical_declarations_until_installation() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = creator(&mut runtime, directory);
    let call = create(&caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let mut raw = library.create();
    let mut interface = unsafe { *raw.interfaces };
    let mut definition = unsafe {
        core::slice::from_raw_parts(
            interface.declaration.ptr,
            interface.declaration.len,
        )
    }
    .to_vec();
    let name = definition.windows(4).position(|name| name == b"read").unwrap();
    definition[name..name + 4].copy_from_slice(b"peek");
    interface.declaration =
        abi::Bytes { ptr: definition.as_ptr(), len: definition.len() };
    raw.interfaces = &raw const interface;
    let provider =
        unsafe { reovim_sdk::import_provider(&raw const raw, library, LIMITS) }
            .unwrap();
    let Err((error, mut rejected)) =
        runtime.register(b"/conflict", provider, None)
    else {
        panic!("conflicting declaration was installed")
    };
    assert_eq!(error, Error::CONFLICT);
    rejected.stop().unwrap();
    drop(rejected);
    let created = completed(&mut runtime, &caller, call.request).unwrap();
    assert_eq!(read(&mut runtime, &caller, created.references[0]), 0);
    close(runtime);
}

#[path = "created_lifetime.rs"]
mod lifetime;
