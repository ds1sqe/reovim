use super::*;

fn call(
    caller: &Caller,
    parent: AccessId,
    destination: AccessId,
    old: &[u8],
    new: &[u8],
) -> Call {
    let mut data = u32::try_from(old.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(old);
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&u32::try_from(new.len()).unwrap().to_le_bytes());
    data.extend_from_slice(new);
    Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Rename,
        subject: parent,
        command: 0,
        expected: None,
        input: Value::copy(&data, &[destination]).unwrap(),
    }
}
fn writer(runtime: &mut Runtime, directory: ResourceId) -> (Caller, AccessId) {
    let caller = runtime.bind(&[grant(directory, 3)]).unwrap().caller();
    let parent = open(runtime, &caller, directory);
    write_access(runtime, &caller, parent);
    (caller, parent)
}
fn write_access(runtime: &mut Runtime, caller: &Caller, parent: AccessId) {
    query(runtime, caller, Operation::SetAccess, parent, &3_u32.to_le_bytes())
        .unwrap();
}
#[test]
fn rename_preserves_open_identity_and_rejects_occupied_destination() {
    let (mut runtime, library, original, directory) = installed();
    let id = subjects(&mut runtime, &original, directory);
    let child = open(&mut runtime, &original, id);
    let (caller, parent) = writer(&mut runtime, directory);
    let occupied = call(&caller, parent, parent, b"disk", b"alias");
    caller.submit(occupied.clone()).unwrap();
    assert_eq!(
        completed(&mut runtime, &caller, occupied.request),
        Err(Error::CONFLICT)
    );
    let renamed = call(&caller, parent, parent, b"disk", b"moved");
    caller.submit(renamed.clone()).unwrap();
    completed(&mut runtime, &caller, renamed.request).unwrap();
    assert_eq!(
        ids(&entries(&mut runtime, &caller, parent, 192).unwrap()),
        vec![(b"alias".to_vec(), id), (b"moved".to_vec(), id)]
    );
    assert_eq!(read(&mut runtime, &original, child), 41);
    assert_eq!(library.count(c"fixture_renamed"), 1);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    close(runtime);
}
#[test]
fn rename_revalidates_source_after_lookup_before_any_mutation() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = writer(&mut runtime, directory);
    let rename = call(&caller, parent, parent, b"disk", b"moved");
    caller.submit(rename.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let replace: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_replace") };
    unsafe { replace() };
    assert_eq!(
        completed(&mut runtime, &caller, rename.request),
        Err(Error::CONFLICT)
    );
    let rows = ids(&entries(&mut runtime, &caller, parent, 192).unwrap());
    assert_eq!(rows[0].0, b"disk");
    assert_eq!(rows[1].0, b"alias");
    assert_eq!(library.count(c"fixture_renamed"), 0);
    close(runtime);
}
#[test]
fn rename_requires_write_on_each_parent_and_rejects_foreign_instances() {
    let (mut runtime, library, _, directory) = installed();
    let other = mounted(&mut runtime, &library, b"/other");
    let caller =
        runtime.bind(&[grant(directory, 3), grant(other, 3)]).unwrap().caller();
    let parent = open(&mut runtime, &caller, directory);
    let target = open(&mut runtime, &caller, other);
    let denied = call(&caller, parent, target, b"disk", b"moved");
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    write_access(&mut runtime, &caller, parent);
    let denied = call(&caller, parent, target, b"disk", b"moved");
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    write_access(&mut runtime, &caller, target);
    let foreign = call(&caller, parent, target, b"disk", b"moved");
    assert_eq!(caller.submit(foreign.clone()), Err(Error::UNSUPPORTED));
    caller.release(foreign.request).unwrap();
    assert_eq!(library.count(c"fixture_lookups"), 0);
    assert_eq!(library.count(c"fixture_renames"), 0);
    close(runtime);
}
#[test]
fn cancelled_rename_never_enters_lookup_or_mutation() {
    let (mut runtime, library, _, directory) = installed();
    let (caller, parent) = writer(&mut runtime, directory);
    let rename = call(&caller, parent, parent, b"disk", b"moved");
    caller.submit(rename.clone()).unwrap();
    assert_eq!(
        caller.cancel(rename.request).unwrap(),
        reovim_kernel_interface::call::Cancellation::Accepted
    );
    assert_eq!(
        completed(&mut runtime, &caller, rename.request),
        Err(Error::CANCELLED)
    );
    assert_eq!(library.count(c"fixture_lookups"), 0);
    assert_eq!(library.count(c"fixture_renames"), 0);
    close(runtime);
}

fn close(mut runtime: Runtime) {
    for _ in 0..16 {
        match runtime.close() {
            Ok(()) => return,
            Err(error) => assert_eq!(error, Error::BUSY),
        }
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    panic!("rename fixture cleanup did not drain")
}
#[test]
fn cross_parent_unknown_rename_holds_destination_close_and_never_replays() {
    let (mut runtime, library, original, directory) = installed();
    let nested: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_nested") };
    unsafe { nested() };
    let root = open(&mut runtime, &original, directory);
    query(
        &mut runtime,
        &original,
        Operation::SetAccess,
        root,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let rows = ids(&entries(&mut runtime, &original, root, 192).unwrap());
    let id = rows[0].1;
    let branch_id = rows[2].1;
    let branch = open(&mut runtime, &original, branch_id);
    let caller = runtime
        .bind(&[grant(directory, 3), grant(branch_id, 3)])
        .unwrap()
        .caller();
    let parent = open(&mut runtime, &caller, directory);
    let target = open(&mut runtime, &caller, branch_id);
    write_access(&mut runtime, &caller, parent);
    write_access(&mut runtime, &caller, target);
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_rename_unknown") };
    unsafe { uncertain() };
    let rename = call(&caller, parent, target, b"disk", b"moved");
    caller.submit(rename.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..5 {
        runtime.advance(cx).unwrap();
    }
    assert_unknown(caller.snapshot(rename.request).unwrap());
    let closing = caller.reserve().unwrap();
    caller
        .submit(Call {
            request: closing,
            operation: Operation::Close,
            subject: target,
            command: 0,
            expected: None,
            input: Value::unit(),
        })
        .unwrap();
    let alternate = open(&mut runtime, &caller, branch_id);
    write_access(&mut runtime, &caller, alternate);
    let moved = ids(&entries(&mut runtime, &caller, alternate, 192).unwrap());
    assert!(
        moved
            .iter()
            .any(|(name, resource)| name == b"moved" && *resource == id)
    );
    let mut name = 5_u32.to_le_bytes().to_vec();
    name.extend_from_slice(b"moved");
    query(&mut runtime, &caller, Operation::Delete, alternate, &name).unwrap();
    assert_eq!(caller.snapshot(closing).unwrap(), Status::Pending);
    caller.submit(rename.clone()).unwrap();
    caller.reconcile(rename.request).unwrap();
    completed(&mut runtime, &caller, rename.request).unwrap();
    completed(&mut runtime, &caller, closing).unwrap();
    let rows = ids(&entries(&mut runtime, &caller, parent, 192).unwrap());
    assert_eq!(rows[0], (b"alias".to_vec(), id));
    query(
        &mut runtime,
        &original,
        Operation::SetAccess,
        branch,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    let rows = ids(&entries(&mut runtime, &original, branch, 192).unwrap());
    assert!(!rows.iter().any(|(name, _)| name == b"moved"));
    assert_eq!(library.count(c"fixture_renames"), 1);
    assert_eq!(library.count(c"fixture_renamed"), 1);
    close(runtime);
}
