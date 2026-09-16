use super::*;

fn writer(runtime: &mut Runtime, id: ResourceId) -> (Caller, AccessId) {
    let caller = runtime.bind(&[grant(id, 3)]).unwrap().caller();
    let access = open(runtime, &caller, id);
    query(runtime, &caller, Operation::SetAccess, access, &3_u32.to_le_bytes())
        .unwrap();
    (caller, access)
}
fn deletion(caller: &Caller, parent: AccessId, name: &[u8]) -> Call {
    let mut data = u32::try_from(name.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(name);
    Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Delete,
        subject: parent,
        command: 0,
        expected: None,
        input: Value::copy(&data, &[]).unwrap(),
    }
}
fn remove(
    runtime: &mut Runtime,
    caller: &Caller,
    parent: AccessId,
    name: &[u8],
) -> Result<Value, Error> {
    let call = deletion(caller, parent, name);
    caller.submit(call.clone()).unwrap();
    completed(runtime, caller, call.request)
}
#[test]
fn removing_names_preserves_open_subject_and_remaining_alias() {
    let (mut runtime, library, caller, directory) = installed();
    let id = subjects(&mut runtime, &caller, directory);
    let child = open(&mut runtime, &caller, id);
    let (writer, parent) = writer(&mut runtime, directory);
    remove(&mut runtime, &writer, parent, b"disk").unwrap();
    let rows = ids(&entries(&mut runtime, &writer, parent, 96).unwrap());
    assert_eq!(rows, vec![(b"alias".to_vec(), id)]);
    remove(&mut runtime, &writer, parent, b"alias").unwrap();
    let empty = ids(&entries(&mut runtime, &writer, parent, 96).unwrap());
    assert_eq!(empty, Vec::<(Vec<u8>, ResourceId)>::new());
    let info =
        query(&mut runtime, &writer, Operation::Info, parent, &[]).unwrap();
    assert_eq!(u32::from_le_bytes(info.data[28..32].try_into().unwrap()), 3);
    assert_eq!(read(&mut runtime, &caller, child), 41);
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    query(&mut runtime, &caller, Operation::Close, child, &[]).unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    assert_eq!(library.count(c"fixture_removed"), 2);
    runtime.close().unwrap();
}
#[test]
fn delete_requires_held_parent_write_and_captures_a_valid_child_name() {
    let (mut runtime, library, caller, directory) = installed();
    let parent = open(&mut runtime, &caller, directory);
    let denied = deletion(&caller, parent, b"disk");
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    let (writer, parent) = writer(&mut runtime, directory);
    for name in [&b"../disk"[..], &b"disk/alias"[..], &b""[..], &[0xff][..]] {
        let call = deletion(&writer, parent, name);
        assert_eq!(writer.submit(call.clone()), Err(Error::INVALID));
        writer.release(call.request).unwrap();
    }
    assert_eq!(library.count(c"fixture_lookups"), 0);
    assert_eq!(library.count(c"fixture_removed"), 0);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
#[test]
fn removal_revalidates_the_key_captured_by_lookup() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = writer(&mut runtime, directory);
    let call = deletion(&caller, parent, b"disk");
    caller.submit(call.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let replace: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_replace") };
    unsafe { replace() };
    assert_eq!(
        completed(&mut runtime, &caller, call.request),
        Err(Error::CONFLICT)
    );
    assert_eq!(library.count(c"fixture_removals"), 1);
    assert_eq!(library.count(c"fixture_removed"), 0);
    let rows = ids(&entries(&mut runtime, &caller, parent, 96).unwrap());
    assert_eq!(rows[0].0, b"disk");
    runtime.close().unwrap();
}
#[test]
fn uncertain_removal_recovers_without_deleting_a_later_replacement() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = writer(&mut runtime, directory);
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_remove_unknown") };
    unsafe { uncertain() };
    let call = deletion(&caller, parent, b"disk");
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..5 {
        runtime.advance(cx).unwrap();
    }
    assert_unknown(caller.snapshot(call.request).unwrap());
    let restore: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_restore") };
    unsafe { restore() };
    caller.submit(call.clone()).unwrap();
    caller.reconcile(call.request).unwrap();
    completed(&mut runtime, &caller, call.request).unwrap();
    assert_eq!(library.count(c"fixture_lookups"), 1);
    assert_eq!(library.count(c"fixture_removals"), 1);
    assert_eq!(library.count(c"fixture_removed"), 1);
    let rows = ids(&entries(&mut runtime, &caller, parent, 96).unwrap());
    assert_eq!(rows[0].0, b"disk");
    runtime.close().unwrap();
}
#[test]
fn cancelled_delete_never_enters_lookup_or_mutation() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = writer(&mut runtime, directory);
    let call = deletion(&caller, parent, b"disk");
    caller.submit(call.clone()).unwrap();
    assert_eq!(
        caller.cancel(call.request).unwrap(),
        reovim_kernel_interface::call::Cancellation::Accepted
    );
    assert_eq!(
        completed(&mut runtime, &caller, call.request),
        Err(Error::CANCELLED)
    );
    assert_eq!(library.count(c"fixture_lookups"), 0);
    assert_eq!(library.count(c"fixture_removed"), 0);
    runtime.close().unwrap();
}

#[test]
fn nonempty_container_removal_does_not_recurse_or_remove_children() {
    let (mut runtime, library, _, directory) = installed();
    let nested: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_nested") };
    unsafe { nested() };
    let (caller, parent) = writer(&mut runtime, directory);
    assert_eq!(
        remove(&mut runtime, &caller, parent, b"branch"),
        Err(Error::CONFLICT)
    );
    let rows = ids(&entries(&mut runtime, &caller, parent, 192).unwrap());
    assert_eq!(
        rows.iter().map(|row| row.0.as_slice()).collect::<Vec<_>>(),
        vec![&b"disk"[..], &b"alias"[..], &b"branch"[..]]
    );
    assert_eq!(library.count(c"fixture_removed"), 0);
    runtime.close().unwrap();
}
