use super::*;

fn path(caller: &Caller, path: &[u8]) -> Call {
    let mut input = 0_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&u32::try_from(path.len()).unwrap().to_le_bytes());
    input.extend_from_slice(path);
    Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Acquire,
        subject: AccessId(0),
        command: 0,
        expected: None,
        input: Value::copy(&input, &[]).unwrap(),
    }
}
fn identity(value: &Value) -> ResourceId {
    ResourceId {
        epoch: value.data[..16].try_into().unwrap(),
        serial: u64::from_le_bytes(value.data[16..24].try_into().unwrap()),
    }
}
#[test]
fn native_paths_join_listed_identity_without_repeating_lost_requests() {
    let (mut runtime, library, caller, directory) = installed();
    let listed = subjects(&mut runtime, &caller, directory);
    let a = path(&caller, b"/mount/directory/disk");
    let b = path(&caller, b"/mount/directory/alias");
    caller.submit(a.clone()).unwrap();
    caller.submit(b.clone()).unwrap();
    assert_eq!(library.count(c"fixture_lookups"), 0);
    caller.submit(a.clone()).unwrap();
    let a = completed(&mut runtime, &caller, a.request).unwrap();
    let b = completed(&mut runtime, &caller, b.request).unwrap();
    assert_eq!(identity(&a), listed);
    assert_eq!(identity(&b), listed);
    assert_ne!(a.references[0], b.references[0]);
    assert_eq!(library.count(c"fixture_lookups"), 2);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    query(&mut runtime, &caller, Operation::Close, a.references[0], &[])
        .unwrap();
    assert_eq!(read(&mut runtime, &caller, b.references[0]), 41);
    query(&mut runtime, &caller, Operation::Close, b.references[0], &[])
        .unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
#[test]
fn nested_aliases_share_the_mounted_key_space_and_bound_cycles() {
    let (mut runtime, library, caller, _) = installed();
    let nested: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_nested") };
    unsafe { nested() };
    let a = path(&caller, b"/mount/directory/branch/again/disk");
    let b = path(&caller, b"/mount/directory/alias");
    caller.submit(a.clone()).unwrap();
    caller.submit(b.clone()).unwrap();
    let a = completed(&mut runtime, &caller, a.request).unwrap();
    let b = completed(&mut runtime, &caller, b.request).unwrap();
    assert_eq!(identity(&a), identity(&b));
    assert_eq!(library.count(c"fixture_materializations"), 2);
    assert_eq!(read(&mut runtime, &caller, a.references[0]), 41);
    query(&mut runtime, &caller, Operation::Close, a.references[0], &[])
        .unwrap();
    query(&mut runtime, &caller, Operation::Close, b.references[0], &[])
        .unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_directories_destroyed"), 2);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
#[test]
fn namespace_paths_do_not_turn_parent_read_into_child_authority() {
    let (mut runtime, library, _, directory) = installed();
    let caller = runtime.bind(&[grant(directory, 1)]).unwrap().caller();
    let call = path(&caller, b"/mount/directory/disk");
    caller.submit(call.clone()).unwrap();
    assert_eq!(
        completed(&mut runtime, &caller, call.request),
        Err(Error::DENIED)
    );
    assert_eq!(library.count(c"fixture_lookups"), 1);
    assert_eq!(library.count(c"fixture_materializations"), 0);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
#[test]
fn path_materialization_reconciles_without_repeating_lookup() {
    let (mut runtime, library, caller, _) = installed();
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_materialize_unknown") };
    unsafe { uncertain() };
    let call = path(&caller, b"/mount/directory/disk");
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..8 {
        runtime.advance(cx).unwrap();
    }
    assert_unknown(caller.snapshot(call.request).unwrap());
    caller.reconcile(call.request).unwrap();
    let value = completed(&mut runtime, &caller, call.request).unwrap();
    assert_eq!(read(&mut runtime, &caller, value.references[0]), 41);
    query(&mut runtime, &caller, Operation::Close, value.references[0], &[])
        .unwrap();
    assert_eq!(library.count(c"fixture_lookups"), 1);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

#[test]
fn lookup_recovery_keeps_the_original_query_and_captured_key() {
    let (mut runtime, library, caller, _) = installed();
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_lookup_unknown") };
    unsafe { uncertain() };
    let call = path(&caller, b"/mount/directory/disk");
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_unknown(caller.snapshot(call.request).unwrap());
    caller.submit(call.clone()).unwrap();
    caller.reconcile(call.request).unwrap();
    let value = completed(&mut runtime, &caller, call.request).unwrap();
    assert_eq!(read(&mut runtime, &caller, value.references[0]), 41);
    query(&mut runtime, &caller, Operation::Close, value.references[0], &[])
        .unwrap();
    assert_eq!(library.count(c"fixture_lookups"), 1);
    assert_eq!(library.count(c"fixture_materializations"), 1);
    runtime.close().unwrap();
}
#[test]
fn replacement_after_lookup_never_retargets_the_captured_key() {
    let (mut runtime, library, caller, _) = installed();
    let call = path(&caller, b"/mount/directory/disk");
    caller.submit(call.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let replace: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_replace") };
    unsafe { replace() };
    assert_eq!(
        completed(&mut runtime, &caller, call.request),
        Err(Error::CONFLICT)
    );
    let fresh = path(&caller, b"/mount/directory/disk");
    caller.submit(fresh.clone()).unwrap();
    let value = completed(&mut runtime, &caller, fresh.request).unwrap();
    assert_eq!(read(&mut runtime, &caller, value.references[0]), 41);
    query(&mut runtime, &caller, Operation::Close, value.references[0], &[])
        .unwrap();
    assert_eq!(library.count(c"fixture_lookups"), 2);
    runtime.close().unwrap();
}
#[test]
fn cancellation_before_path_driving_never_calls_the_namespace() {
    let (mut runtime, library, caller, _) = installed();
    let call = path(&caller, b"/mount/directory/disk");
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
    assert_eq!(library.count(c"fixture_materializations"), 0);
    runtime.close().unwrap();
}

#[test]
fn exact_child_grants_support_paths_without_publication_authority() {
    let (mut runtime, library, caller, directory) = installed();
    let id = subjects(&mut runtime, &caller, directory);
    let child = open(&mut runtime, &caller, id);
    let binding = runtime.bind(&[grant(directory, 1), grant(id, 0)]).unwrap();
    let other = binding.caller();
    let call = path(&other, b"/mount/directory/disk");
    other.submit(call.clone()).unwrap();
    let value = completed(&mut runtime, &other, call.request).unwrap();
    assert_eq!(identity(&value), id);
    assert_eq!(
        query(
            &mut runtime,
            &other,
            Operation::SetAccess,
            value.references[0],
            &1_u32.to_le_bytes()
        ),
        Err(Error::DENIED)
    );
    query(&mut runtime, &other, Operation::Close, value.references[0], &[])
        .unwrap();
    query(&mut runtime, &caller, Operation::Close, child, &[]).unwrap();
    assert_eq!(library.count(c"fixture_materializations"), 1);
    // Closing releases exact grants; driving then reclaims the unnamed child.
    assert_eq!(runtime.close(), Err(Error::BUSY));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
