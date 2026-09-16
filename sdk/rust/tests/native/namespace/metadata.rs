use super::*;

fn setup() -> (Runtime, Rc<Library>, Caller, AccessId) {
    let library = Library::compile_source("namespace.c");
    let mut runtime = Runtime::new([0x85; 16], Limits::default()).unwrap();
    let directory = mounted(&mut runtime, &library, b"/device");
    let caller = runtime.bind(&[grant(directory, 0)]).unwrap().caller();
    runtime.start().unwrap();
    let access = open(&mut runtime, &caller, directory);
    (runtime, library, caller, access)
}
fn request(caller: &Caller, subject: AccessId) -> Call {
    Call {
        request: caller.reserve().unwrap(),
        operation: Operation::Info,
        subject,
        command: 0,
        expected: None,
        input: Value::unit(),
    }
}
fn flags(value: &Value) -> u32 {
    u32::from_le_bytes(value.data[28..32].try_into().unwrap())
}
#[test]
fn native_flags_are_driven_copied_observations_without_listing_or_read() {
    let (mut runtime, library, caller, access) = setup();
    let call = request(&caller, access);
    caller.submit(call.clone()).unwrap();
    assert_eq!(library.count(c"fixture_flag_queries"), 0);
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let Status::Outcome(first) = caller.snapshot(call.request).unwrap() else {
        panic!("missing metadata")
    };
    assert_eq!(flags(&success(first.as_ref()).unwrap()), 1);
    let empty: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_empty") };
    unsafe { empty() };
    caller.submit(call.clone()).unwrap();
    assert_eq!(caller.snapshot(call.request).unwrap(), Status::Outcome(first));
    let original = completed(&mut runtime, &caller, call.request).unwrap();
    let current =
        query(&mut runtime, &caller, Operation::Info, access, &[]).unwrap();
    assert_eq!(flags(&original), 1);
    assert_eq!(flags(&current), 3);
    assert_eq!(u32::from_le_bytes(current.data[24..28].try_into().unwrap()), 0);
    assert_eq!(library.count(c"fixture_flag_queries"), 2);
    assert_eq!(library.count(c"fixture_listings"), 0);
    query(&mut runtime, &caller, Operation::Close, access, &[]).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}
#[test]
fn unknown_metadata_recovers_the_original_observation() {
    let (mut runtime, library, caller, access) = setup();
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_flags_unknown") };
    unsafe { uncertain() };
    let call = request(&caller, access);
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let Status::Outcome(outcome) = caller.snapshot(call.request).unwrap()
    else {
        panic!("missing unknown")
    };
    assert!(matches!(outcome.as_ref(), Outcome::Unknown { .. }));
    let empty: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_empty") };
    unsafe { empty() };
    caller.reconcile(call.request).unwrap();
    let value = completed(&mut runtime, &caller, call.request).unwrap();
    assert_eq!(flags(&value), 1);
    assert_eq!(library.count(c"fixture_flag_queries"), 1);
    query(&mut runtime, &caller, Operation::Close, access, &[]).unwrap();
    runtime.close().unwrap();
}
#[test]
fn empty_without_container_fences_metadata_and_retains_cleanup() {
    let (mut runtime, library, caller, access) = setup();
    let invalid: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_bad_flags") };
    unsafe { invalid() };
    let call = request(&caller, access);
    caller.submit(call.clone()).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let Status::Outcome(outcome) = caller.snapshot(call.request).unwrap()
    else {
        panic!("missing fenced outcome")
    };
    let Outcome::Unknown { error, progress } = outcome.as_ref() else {
        panic!("invalid flags were accepted")
    };
    assert_eq!(*error, Error::PROVIDER);
    assert!(progress.is_none());
    assert_eq!(caller.reconcile(call.request), Err(Error::CLOSED));
    caller.release(call.request).unwrap();
    assert_eq!(runtime.close(), Err(Error::INCOMPLETE));
    assert_eq!(library.count(c"fixture_directories_destroyed"), 0);
}
