use super::*;

fn toggle(library: &Library, name: &core::ffi::CStr) {
    let toggle: unsafe extern "C" fn() = unsafe { library.symbol(name) };
    unsafe { toggle() };
}

fn byte(runtime: &mut Runtime, caller: &Caller, access: AccessId) -> u8 {
    let value =
        query(runtime, caller, Operation::Read, access, &1_u64.to_le_bytes())
            .unwrap();
    value.data[4]
}

fn uncertain(runtime: &mut Runtime, caller: &Caller, request: u64) {
    for _ in 0..16 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
        if let Status::Outcome(outcome) = caller.snapshot(request).unwrap() {
            assert!(matches!(
                outcome.as_ref(),
                Outcome::Unknown { error: Error::UNAVAILABLE, progress: None }
            ));
            return;
        }
    }
    panic!("initialization did not report its uncertainty");
}

#[test]
fn creation_initializes_the_returned_access_before_installation() {
    let (mut runtime, library, observer, directory) =
        installed_source("open_namespace.c");
    let (caller, parent) = creator(&mut runtime, directory);
    let request = create(&caller, parent, b"new");
    caller.submit(request.clone()).unwrap();
    let created = completed(&mut runtime, &caller, request.request).unwrap();
    let token = created.references[0];
    assert_eq!(byte(&mut runtime, &caller, token), b'a');
    let reopened = open(&mut runtime, &observer, id(&created));
    query(
        &mut runtime,
        &observer,
        Operation::SetAccess,
        reopened,
        &1_u32.to_le_bytes(),
    )
    .unwrap();
    assert_eq!(byte(&mut runtime, &observer, reopened), b'a');
    assert_eq!(byte(&mut runtime, &caller, token), b'b');
    assert_eq!(library.count(c"fixture_opens"), 2);
    assert_eq!(library.count(c"fixture_creations"), 1);
    close(runtime);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 2);
}

#[test]
fn failed_initial_open_recovers_published_creation_without_creating_again() {
    let (mut runtime, library, observer, directory) =
        installed_source("open_namespace.c");
    let (caller, parent) = creator(&mut runtime, directory);
    toggle(&library, c"fixture_open_failed");
    let request = create(&caller, parent, b"new");
    caller.submit(request.clone()).unwrap();
    uncertain(&mut runtime, &caller, request.request);
    assert_eq!(caller.adopt(request.request), Err(Error::CONFLICT));
    let rows = ids(&entries(&mut runtime, &caller, parent, 256).unwrap());
    let identity = rows.iter().find(|(name, _)| name == b"new").unwrap().1;
    let competing = acquisition(&observer, identity);
    observer.submit(competing.clone()).unwrap();
    assert_eq!(
        completed(&mut runtime, &observer, competing.request),
        Err(Error::BUSY)
    );
    toggle(&library, c"fixture_stop_busy");
    assert_eq!(caller.reconcile(request.request), Err(Error::BUSY));
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 0);
    assert_eq!(library.count(c"fixture_creations"), 1);
    assert_eq!(library.count(c"fixture_opens"), 1);
    toggle(&library, c"fixture_open_ready");
    toggle(&library, c"fixture_stop_ready");
    caller.submit(request.clone()).unwrap();
    caller.reconcile(request.request).unwrap();
    let created = completed(&mut runtime, &caller, request.request).unwrap();
    assert_eq!(id(&created), identity);
    assert_eq!(byte(&mut runtime, &caller, created.references[0]), b'a');
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    assert_eq!(library.count(c"fixture_opens"), 2);
    assert_eq!(library.count(c"fixture_creations"), 1);
    close(runtime);
}

#[test]
fn unknown_initial_open_reconciles_the_same_pending_initialization() {
    let (mut runtime, library, _, directory) =
        installed_source("open_namespace.c");
    let (caller, parent) = creator(&mut runtime, directory);
    toggle(&library, c"fixture_open_unknown");
    let request = create(&caller, parent, b"new");
    caller.submit(request.clone()).unwrap();
    uncertain(&mut runtime, &caller, request.request);
    assert_eq!(
        caller.cancel(request.request),
        Ok(reovim_kernel_interface::call::Cancellation::TooLate)
    );
    caller.reconcile(request.request).unwrap();
    let created = completed(&mut runtime, &caller, request.request).unwrap();
    assert_eq!(byte(&mut runtime, &caller, created.references[0]), b'a');
    assert_eq!(library.count(c"fixture_creations"), 1);
    assert_eq!(library.count(c"fixture_opens"), 1);
    close(runtime);
}
