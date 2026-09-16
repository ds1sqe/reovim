use super::*;
use reovim_sdk::import_state;

fn native_state(library: &Library, mode: u32) -> abi::State {
    let create: unsafe extern "C" fn(u32, *mut abi::State) -> u32 =
        unsafe { library.symbol(c"fixture_state") };
    let mut state = std::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(mode, state.as_mut_ptr()) }, 0);
    unsafe { state.assume_init() }
}
fn release(state: abi::State) {
    unsafe { ((*state.ops).release.unwrap())(state.data) };
}

#[test]
fn native_state_and_code_survive_original_owner_and_pending_snapshot() {
    let library = Library::compile_source("state.c");
    let raw = native_state(&library, 0);
    let state =
        unsafe { import_state(&raw const raw, library.clone(), LIMITS) }
            .unwrap();
    assert_eq!(state.cost(), 17);
    release(raw);
    let mut provider = state.snapshot().unwrap();
    let mut pending =
        provider.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap();
    drop(state);
    let code = Rc::downgrade(&library);
    drop(library);
    assert!(code.upgrade().is_some());
    ready(&mut pending);
    let completion = pending.take().unwrap();
    let Outcome::Ok(value) = &completion.outcome else {
        panic!("expected snapshot value");
    };
    assert_eq!(value.data.as_ref(), &91_u64.to_le_bytes());
    drop(completion);
    pending.finish().unwrap();
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(code.upgrade().unwrap().count(c"fixture_state_dead"), 0);
    drop(pending);
    assert!(code.upgrade().is_none());
}

#[test]
fn malformed_state_table_does_not_retain_or_invoke() {
    let library = Library::compile_source("state.c");
    let raw = native_state(&library, 0);
    let mut ops = unsafe { *raw.ops };
    ops.snapshot = None;
    let invalid = abi::State { ops: &raw const ops, ..raw };
    assert_eq!(
        unsafe { import_state(&raw const invalid, library.clone(), LIMITS) }
            .err(),
        Some(Error::INVALID)
    );
    ops.snapshot = unsafe { (*raw.ops).snapshot };
    ops.bytes = 0;
    let invalid = abi::State { ops: &raw const ops, ..raw };
    assert_eq!(
        unsafe { import_state(&raw const invalid, library.clone(), LIMITS) }
            .err(),
        Some(Error::INVALID)
    );
    assert_eq!(library.count(c"fixture_state_refs"), 1);
    assert_eq!(library.count(c"fixture_snapshots"), 0);
    release(raw);
    assert_eq!(library.count(c"fixture_state_dead"), 1);
}

#[test]
fn snapshot_failure_preserves_state_and_busy_rejection_fences_new_calls() {
    let library = Library::compile_source("state.c");
    let raw = native_state(&library, 1);
    let state =
        unsafe { import_state(&raw const raw, library.clone(), LIMITS) }
            .unwrap();
    let mode: unsafe extern "C" fn(*const abi::State, u32) =
        unsafe { library.symbol(c"fixture_state_mode") };
    let busy: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_cleanup_busy") };
    assert_eq!(state.snapshot().err(), Some(Error::PRESSURE));
    assert_eq!(library.count(c"fixture_state_refs"), 2);
    unsafe { mode(&raw const raw, 2) };
    unsafe { busy(1) };
    assert_eq!(state.snapshot().err(), Some(Error::INVALID));
    assert_eq!(library.count(c"fixture_snapshots"), 2);
    assert_eq!(state.snapshot().err(), Some(Error::BUSY));
    assert_eq!(library.count(c"fixture_snapshots"), 2);
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    unsafe { busy(0) };
    unsafe { mode(&raw const raw, 0) };
    let mut provider = state.snapshot().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    release(raw);
    drop(state);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_state_dead"), 1);
}

#[test]
fn native_snapshot_reentry_is_busy_without_a_second_constructor() {
    use reovim_kernel_interface::provider::State;
    use std::cell::Cell;

    struct Reentry {
        state: State,
        result: Cell<Option<Error>>,
    }
    unsafe extern "C" fn reenter(context: *mut c_void) {
        let entry = unsafe { &*context.cast::<Reentry>() };
        entry.result.set(entry.state.snapshot().err());
    }
    let library = Library::compile_source("state.c");
    let raw = native_state(&library, 0);
    let state =
        unsafe { import_state(&raw const raw, library.clone(), LIMITS) }
            .unwrap();
    release(raw);
    let hook: unsafe extern "C" fn(
        unsafe extern "C" fn(*mut c_void),
        *mut c_void,
    ) = unsafe { library.symbol(c"fixture_snapshot_hook") };
    let mut entry = Reentry { state: state.clone(), result: Cell::new(None) };
    unsafe { hook(reenter, (&raw mut entry).cast()) };
    let mut provider = state.snapshot().unwrap();
    assert_eq!(entry.result.get(), Some(Error::BUSY));
    assert_eq!(library.count(c"fixture_snapshots"), 1);
    provider.stop().unwrap();
    drop(provider);
    drop(entry);
    drop(state);
    assert_eq!(library.count(c"fixture_state_dead"), 1);
}
