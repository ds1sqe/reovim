use super::*;
use reovim_kernel_interface::provider::{Open, Read, ResourceProvider};
use std::cell::RefCell;

type Staged = Rc<RefCell<Option<ResourceProvider>>>;

#[path = "open/runtime.rs"]
mod execution;
fn raw(library: &Library) -> abi::Provider {
    let create: unsafe extern "C" fn(*mut abi::Provider) -> u32 =
        unsafe { library.symbol(c"fixture_open_subject") };
    let mut raw = core::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(raw.as_mut_ptr()) }, 0);
    unsafe { raw.assume_init() }
}
fn subject(library: &Rc<Library>, roundtrip: bool) -> ResourceProvider {
    let raw = raw(library);
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    if !roundtrip {
        return provider;
    }
    let raw = reovim_sdk::ExportedProvider::new(provider, LIMITS)
        .map_err(|(error, _)| error)
        .unwrap()
        .into_raw();
    unsafe { reovim_sdk::import_provider(&raw const raw, Rc::new(()), LIMITS) }
        .unwrap()
}
fn target(state: Staged) -> Open {
    Open::new(state, |state, provider| {
        let mut slot = state.borrow_mut();
        if slot.is_some() {
            return Err((Error::CONFLICT, provider));
        }
        *slot = Some(provider);
        Ok(())
    })
}
fn ready(pending: &mut Pending) {
    assert_eq!(
        pending.poll(&mut Context::from_waker(Waker::noop())),
        Ok(Poll::Ready(()))
    );
}
fn value(mut pending: Pending) -> Value {
    ready(&mut pending);
    let result = pending.take().unwrap();
    let Outcome::Ok(value) = &result.outcome else {
        panic!("{:?}", result.outcome)
    };
    let value = value.clone();
    drop(result);
    pending.finish().unwrap();
    value
}
fn open(provider: &mut ResourceProvider) -> ResourceProvider {
    let slot = Rc::new(RefCell::new(None));
    let target = target(slot.clone());
    assert_eq!(value(provider.open(&target).unwrap()), Value::unit());
    slot.borrow_mut().take().unwrap()
}
fn bytes(provider: &mut ResourceProvider, offset: Option<u64>) -> Value {
    value(provider.read(Read { offset, maximum: 1 }).unwrap())
}
fn assert_byte(value: &Value, expected: u8) {
    let length =
        u32::from_le_bytes(value.data[..4].try_into().unwrap()) as usize;
    assert_eq!(&value.data[4..4 + length], &[expected]);
    assert_eq!(&value.data[4 + length..], &[0]);
}
#[test]
fn independent_native_opens_own_cursors_and_retain_subject_code() {
    for roundtrip in [false, true] {
        let library = Library::compile_source("open.c");
        let mut parent = subject(&library, roundtrip);
        assert!(parent.has_open());
        assert!(parent.io().is_none());
        let mut first = open(&mut parent);
        let mut second = open(&mut parent);
        assert_byte(&bytes(&mut first, None), b'a');
        assert_byte(&bytes(&mut first, None), b'b');
        assert_byte(&bytes(&mut second, None), b'a');
        assert_byte(&bytes(&mut second, Some(0)), b'a');
        assert_byte(&bytes(&mut second, None), b'b');
        parent.stop().unwrap();
        drop(parent);
        assert_eq!(library.count(c"fixture_destroyed"), 0);
        assert_byte(&bytes(&mut first, None), b'c');
        first.stop().unwrap();
        drop(first);
        second.stop().unwrap();
        drop(second);
        assert_eq!(library.count(c"fixture_cursors_destroyed"), 2);
        assert_eq!(library.count(c"fixture_destroyed"), 1);
    }
}
#[test]
fn unknown_open_recovers_once_and_pending_read_delays_cursor_cleanup() {
    let library = Library::compile_source("open.c");
    let mut parent = subject(&library, true);
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_open_unknown") };
    unsafe { uncertain() };
    let slot = Rc::new(RefCell::new(None));
    let target = target(slot.clone());
    let mut pending = parent.open(&target).unwrap();
    ready(&mut pending);
    let completion = pending.take().unwrap();
    assert!(matches!(completion.outcome, Outcome::Unknown { .. }));
    drop(completion);
    assert_eq!(pending.finish(), Err(Error::BUSY));
    assert_eq!(parent.stop(), Err(Error::BUSY));
    pending.reconcile().unwrap();
    assert_eq!(value(pending), Value::unit());
    let mut opened = slot.borrow_mut().take().unwrap();
    let pending = opened.read(Read { offset: None, maximum: 1 }).unwrap();
    assert_eq!(opened.stop(), Err(Error::BUSY));
    assert_byte(&value(pending), b'a');
    opened.stop().unwrap();
    drop(opened);
    parent.stop().unwrap();
    drop(parent);
    assert_eq!(library.count(c"fixture_opens"), 1);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
#[test]
fn rejected_open_staging_leaves_native_cursor_cleanup_with_supplier() {
    let library = Library::compile_source("open.c");
    let mut parent = subject(&library, true);
    let reject = Open::new((), |(), provider| Err((Error::DENIED, provider)));
    assert!(matches!(parent.open(&reject), Err(Error::DENIED)));
    assert_eq!(library.count(c"fixture_opens"), 0);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    parent.stop().unwrap();
}
#[test]
fn malformed_open_table_rejects_without_taking_native_ownership() {
    let library = Library::compile_source("open.c");
    let mut raw = raw(&library);
    let mut table = unsafe { *raw.open };
    table.begin = None;
    raw.open = &raw const table;
    assert!(matches!(
        unsafe {
            reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
        },
        Err(Error::INVALID)
    ));
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn empty() -> ResourceProvider {
    ResourceProvider::new(
        (),
        reovim_kernel_interface::provider::ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}
fn settled() -> Pending {
    use reovim_kernel_interface::call::{Cancellation, Completion, PendingOps};
    Pending::new(
        Some(Completion {
            outcome: Outcome::Ok(Value::unit()),
            activity: 0,
            owner: None,
        }),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value| value.take().ok_or(Error::CLOSED),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    )
}
#[test]
fn safe_open_clone_is_fenced_before_its_borrowed_native_target_expires() {
    let saved = Rc::new(RefCell::new(None));
    let provider = ResourceProvider::new(
        saved.clone(),
        reovim_kernel_interface::provider::ResourceOps {
            open: Some(
                |saved: &mut Rc<RefCell<Option<Open>>>, opened: &Open| {
                    *saved.borrow_mut() = Some(opened.clone());
                    opened.stage(empty()).map_err(|(error, _)| error)?;
                    Ok(settled())
                },
            ),
            namespace: None,
            io: None,
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    let raw = reovim_sdk::ExportedProvider::new(provider, LIMITS)
        .map_err(|(error, _)| error)
        .unwrap()
        .into_raw();
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, Rc::new(()), LIMITS)
    }
    .unwrap();
    let mut opened = open(&mut provider);
    let late = saved.borrow_mut().take().unwrap();
    let Err((error, mut rejected)) = late.stage(empty()) else {
        panic!("expired native target was called")
    };
    assert_eq!(error, Error::CLOSED);
    rejected.stop().unwrap();
    opened.stop().unwrap();
    provider.stop().unwrap();
}
