use super::*;
use reovim_kernel_interface::provider::{Edit, ResourceProvider, State};
use reovim_sdk::import_state_init;
use std::cell::RefCell;

fn native_init(library: &Library) -> abi::StateInit {
    let create: unsafe extern "C" fn(*mut abi::StateInit) -> u32 =
        unsafe { library.symbol(c"fixture_state_init") };
    let mut output = std::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(output.as_mut_ptr()) }, 0);
    unsafe { output.assume_init() }
}
fn release_state(state: abi::State) {
    unsafe { ((*state.ops).release.unwrap())(state.data) };
}
fn pair(library: &Rc<Library>) -> (State, ResourceProvider) {
    let init = native_init(library);
    let pair =
        unsafe { import_state_init(&raw const init, library.clone(), LIMITS) }
            .unwrap();
    release_state(init.initial);
    pair
}

struct Request {
    write: bool,
    live: bool,
    candidate: Option<State>,
}
fn edit(before: State, write: bool) -> (Edit, Rc<RefCell<Request>>) {
    let request =
        Rc::new(RefCell::new(Request { write, live: true, candidate: None }));
    let edit = Edit::new(before, request.clone(), propose);
    (edit, request)
}
fn propose(request: &Rc<RefCell<Request>>, state: &State) -> Result<(), Error> {
    let mut request = request.try_borrow_mut().map_err(|_| Error::BUSY)?;
    if !request.live {
        return Err(Error::CLOSED);
    }
    if !request.write {
        return Err(Error::DENIED);
    }
    if request.candidate.is_some() {
        return Err(Error::CONFLICT);
    }
    request.candidate = Some(state.clone());
    Ok(())
}
fn input() -> Value {
    Value::copy(&7_u64.to_le_bytes(), &[]).unwrap()
}
fn value(state: &State) -> u64 {
    let mut provider = state.snapshot().unwrap();
    let mut pending =
        provider.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap();
    ready(&mut pending);
    let completion = pending.take().unwrap();
    let Outcome::Ok(value) = &completion.outcome else {
        panic!("expected immutable value");
    };
    let value = u64::from_le_bytes(value.data.as_ref().try_into().unwrap());
    drop(completion);
    pending.finish().unwrap();
    provider.stop().unwrap();
    value
}

#[test]
fn native_edit_retains_before_and_proposes_a_readable_independent_candidate() {
    let library = Library::compile_source("edit.c");
    let duplicate: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_duplicate") };
    unsafe { duplicate(1) };
    let (before, mut provider) = pair(&library);
    assert_eq!(value(&before), 91);
    let selected = before.clone();
    let (edit, request) = edit(before, true);
    let mut pending =
        provider.prepare(0, Operation::Ioctl(7), &edit, &input()).unwrap();
    drop(edit);
    assert!(request.borrow().candidate.is_none());
    ready(&mut pending);
    let completion = pending.take().unwrap();
    assert!(matches!(completion.outcome, Outcome::Ok(_)));
    assert_eq!(value(&selected), 91);
    let candidate = request.borrow().candidate.clone().unwrap();
    assert_eq!(value(&candidate), 98);
    assert_eq!(
        library.count(c"fixture_last_proposal"),
        u64::from(Error::CONFLICT.code.0)
    );
    assert_eq!(pending.finish(), Err(Error::BUSY));
    drop(completion);
    request.borrow_mut().live = false;
    drop(selected);
    assert_eq!(provider.stop(), Err(Error::BUSY));
    pending.finish().unwrap();
    assert_eq!(library.count(c"fixture_last_before"), 91);
    provider.stop().unwrap();
    drop(provider);
    drop(pending);
    drop(request);
    assert_eq!(library.count(c"fixture_editor_dead"), 0);
    assert_eq!(value(&candidate), 98);
    drop(candidate);
    assert_eq!(library.count(c"fixture_editor_dead"), 1);
    assert_eq!(library.count(c"fixture_state_refs"), 0);
}

#[test]
fn foreign_instance_and_unbound_state_reject_before_native_callback() {
    let library = Library::compile_source("edit.c");
    let (before, mut first) = pair(&library);
    let (other, mut second) = pair(&library);
    let (foreign, _) = edit(other, true);
    assert_eq!(
        first.prepare(0, Operation::Ioctl(7), &foreign, &input()).err(),
        Some(Error::INVALID)
    );
    let init = native_init(&library);
    let unbound = unsafe {
        reovim_sdk::import_state(
            &raw const init.initial,
            library.clone(),
            LIMITS,
        )
    }
    .unwrap();
    let (unbound, _) = edit(unbound, true);
    assert_eq!(
        first.prepare(0, Operation::Ioctl(7), &unbound, &input()).err(),
        Some(Error::INVALID)
    );
    assert_eq!(library.count(c"fixture_prepares"), 0);
    drop(unbound);
    release_state(init.initial);
    assert_eq!(
        unsafe {
            init.implementation.stop.unwrap()(init.implementation.context)
        },
        0
    );
    unsafe {
        init.implementation.destroy.unwrap()(init.implementation.context);
    };
    first.stop().unwrap();
    second.stop().unwrap();
    drop(first);
    drop(second);
    drop(before);
    drop(foreign);
    assert_eq!(library.count(c"fixture_state_refs"), 0);
    assert_eq!(library.count(c"fixture_editor_dead"), 3);
}

#[test]
fn denied_and_cancelled_native_edits_leave_selected_state_without_candidates() {
    let library = Library::compile_source("edit.c");
    let (before, mut provider) = pair(&library);
    let (denied, request) = edit(before.clone(), false);
    let mut pending =
        provider.prepare(0, Operation::Ioctl(7), &denied, &input()).unwrap();
    ready(&mut pending);
    let completion = pending.take().unwrap();
    assert!(matches!(completion.outcome, Outcome::Failed(Error::DENIED)));
    assert!(request.borrow().candidate.is_none());
    drop(completion);
    pending.finish().unwrap();
    drop(pending);
    let (allowed, request) = edit(before.clone(), true);
    let mut pending =
        provider.prepare(0, Operation::Ioctl(7), &allowed, &input()).unwrap();
    assert_eq!(
        pending.cancel().unwrap(),
        reovim_kernel_interface::call::Cancellation::Accepted
    );
    ready(&mut pending);
    let completion = pending.take().unwrap();
    assert!(matches!(completion.outcome, Outcome::Failed(Error::CANCELLED)));
    assert!(request.borrow().candidate.is_none());
    assert_eq!(value(&before), 91);
    drop(completion);
    pending.finish().unwrap();
    provider.stop().unwrap();
}

#[test]
fn invalid_initial_state_does_not_take_implementation_ownership() {
    let library = Library::compile_source("edit.c");
    let init = native_init(&library);
    let mut invalid = init;
    let mut ops = unsafe { *init.initial.ops };
    ops.release = None;
    invalid.initial.ops = &raw const ops;
    assert_eq!(
        unsafe {
            import_state_init(&raw const invalid, library.clone(), LIMITS)
        }
        .err(),
        Some(Error::INVALID)
    );
    assert_eq!(library.count(c"fixture_state_refs"), 1);
    assert_eq!(library.count(c"fixture_editor_dead"), 0);
    let (state, mut provider) =
        unsafe { import_state_init(&raw const init, library.clone(), LIMITS) }
            .unwrap();
    release_state(init.initial);
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_editor_dead"), 0);
    drop(state);
    assert_eq!(library.count(c"fixture_editor_dead"), 1);
}

#[test]
fn proposed_candidates_keep_their_binding_for_later_preparation() {
    let library = Library::compile_source("edit.c");
    let (mut selected, mut provider) = pair(&library);
    for expected in [98, 105] {
        let (edit, request) = edit(selected.clone(), true);
        let mut pending =
            provider.prepare(0, Operation::Ioctl(7), &edit, &input()).unwrap();
        ready(&mut pending);
        let completion = pending.take().unwrap();
        assert!(matches!(completion.outcome, Outcome::Ok(_)));
        let next = request.borrow_mut().candidate.take().unwrap();
        assert_eq!(value(&next), expected);
        assert_eq!(value(&selected), expected - 7);
        drop(completion);
        pending.finish().unwrap();
        selected = next;
    }
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_editor_dead"), 0);
    drop(selected);
    assert_eq!(library.count(c"fixture_state_refs"), 0);
    assert_eq!(library.count(c"fixture_editor_dead"), 1);
}
