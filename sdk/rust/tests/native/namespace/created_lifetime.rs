use super::*;

fn invoke_toggle(library: &Library, name: &std::ffi::CStr) {
    let toggle: unsafe extern "C" fn() = unsafe { library.symbol(name) };
    unsafe { toggle() };
}
fn created(runtime: &mut Runtime, caller: &Caller, parent: AccessId) -> Value {
    let call = create(caller, parent, b"new");
    caller.submit(call.clone()).unwrap();
    completed(runtime, caller, call.request).unwrap()
}
fn remove(runtime: &mut Runtime, caller: &Caller, parent: AccessId) -> Call {
    let mut name = 3_u32.to_le_bytes().to_vec();
    name.extend_from_slice(b"new");
    let call = Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Delete,
        subject: parent,
        command: 0,
        expected: None,
        input: Value::copy(&name, &[]).unwrap(),
    };
    caller.submit(call.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    call
}
fn named(
    runtime: &mut Runtime,
    caller: &Caller,
    parent: AccessId,
) -> ResourceId {
    ids(&entries(runtime, caller, parent, 192).unwrap())
        .into_iter()
        .find(|(name, _)| name == b"new")
        .unwrap()
        .1
}
fn write_and_close(
    runtime: &mut Runtime,
    caller: &Caller,
    access: AccessId,
    add: u32,
) -> u64 {
    let mut page = 0_u64.to_le_bytes().to_vec();
    page.extend_from_slice(&4096_u64.to_le_bytes());
    let offered =
        query(runtime, caller, Operation::Interfaces, access, &page).unwrap();
    let length =
        u32::from_le_bytes(offered.data[4..8].try_into().unwrap()) as usize;
    let bound = query(
        runtime,
        caller,
        Operation::Bind,
        access,
        &offered.data[4..8 + length],
    )
    .unwrap()
    .references[0];
    query(runtime, caller, Operation::SetAccess, bound, &7_u32.to_le_bytes())
        .unwrap();
    let call = Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Exec,
        subject: bound,
        command: 0,
        expected: None,
        input: Value::copy(&add.to_le_bytes(), &[]).unwrap(),
    };
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_unknown(caller.snapshot(call.request).unwrap());
    caller.reconcile(call.request).unwrap();
    let value = number(&completed(runtime, caller, call.request).unwrap());
    query(runtime, caller, Operation::Close, bound, &[]).unwrap();
    value
}
#[test]
fn named_object_contents_survive_last_provider_close_and_rematerialization() {
    let (mut runtime, library, observer, directory) = installed();
    let (caller, parent) = creator_with(&mut runtime, directory, 7);
    let initial = created(&mut runtime, &caller, parent);
    assert_eq!(
        write_and_close(&mut runtime, &caller, initial.references[0], 23),
        23
    );
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    assert_eq!(library.count(c"fixture_created_destroyed"), 0);
    let fresh = named(&mut runtime, &caller, parent);
    assert_ne!(fresh, id(&initial));
    let reopened = open(&mut runtime, &observer, fresh);
    assert_eq!(read(&mut runtime, &observer, reopened), 23);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    close(runtime);
    assert_eq!(library.count(c"fixture_created_destroyed"), 1);
}
#[test]
fn unknown_creation_keeps_original_unnamed_object_after_name_replacement() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = creator_with(&mut runtime, directory, 7);
    invoke_toggle(&library, c"fixture_create_unknown");
    let original = create(&caller, parent, b"new");
    caller.submit(original.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_unknown(caller.snapshot(original.request).unwrap());
    let original_id = named(&mut runtime, &caller, parent);
    let removal = remove(&mut runtime, &caller, parent);
    completed(&mut runtime, &caller, removal.request).unwrap();
    invoke_toggle(&library, c"fixture_create_known");
    let replacement = created(&mut runtime, &caller, parent);
    assert_ne!(id(&replacement), original_id);
    caller.submit(original.clone()).unwrap();
    caller.reconcile(original.request).unwrap();
    let recovered = completed(&mut runtime, &caller, original.request).unwrap();
    assert_eq!(id(&recovered), original_id);
    query(&mut runtime, &caller, Operation::Close, parent, &[]).unwrap();
    assert_eq!(
        write_and_close(&mut runtime, &caller, replacement.references[0], 9),
        9
    );
    assert_eq!(read(&mut runtime, &caller, recovered.references[0]), 0);
    assert_eq!(library.count(c"fixture_creations"), 2);
    assert_eq!(library.count(c"fixture_created_destroyed"), 0);
    query(
        &mut runtime,
        &caller,
        Operation::Close,
        recovered.references[0],
        &[],
    )
    .unwrap();
    assert_eq!(library.count(c"fixture_created_destroyed"), 1);
    close(runtime);
    assert_eq!(library.count(c"fixture_created_destroyed"), 2);
}
#[test]
fn unknown_unlink_never_deletes_the_new_object_at_the_same_name() {
    let (mut runtime, library, observer, directory) = installed();
    let (caller, parent) = creator_with(&mut runtime, directory, 7);
    let original = created(&mut runtime, &caller, parent);
    let retained = open(&mut runtime, &observer, id(&original));
    assert_eq!(
        write_and_close(&mut runtime, &caller, original.references[0], 17),
        17
    );
    invoke_toggle(&library, c"fixture_remove_unknown");
    let removal = remove(&mut runtime, &caller, parent);
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..4 {
        runtime.advance(cx).unwrap();
    }
    assert_unknown(caller.snapshot(removal.request).unwrap());
    let replacement = created(&mut runtime, &caller, parent);
    assert_ne!(id(&replacement), id(&original));
    caller.reconcile(removal.request).unwrap();
    completed(&mut runtime, &caller, removal.request).unwrap();
    assert_eq!(named(&mut runtime, &caller, parent), id(&replacement));
    assert_eq!(read(&mut runtime, &observer, retained), 17);
    assert_eq!(read(&mut runtime, &caller, replacement.references[0]), 0);
    assert_eq!(library.count(c"fixture_removals"), 1);
    close(runtime);
    assert_eq!(library.count(c"fixture_created_destroyed"), 2);
}
