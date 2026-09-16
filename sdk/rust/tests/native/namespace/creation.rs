use super::*;

type Target = Rc<RefCell<Option<(Vec<u8>, ResourceProvider)>>>;
fn target(state: Target, reject: bool) -> Creation {
    Creation::new(
        (state, reject),
        CreationOps {
            resource: |_, provider| Err((Error::UNSUPPORTED, provider)),
            auxiliary: None,
            keyed: Some(|(target, reject), key, provider| {
                if *reject {
                    return Err((Error::DENIED, provider));
                }
                let mut target = target.borrow_mut();
                if target.is_some() {
                    return Err((Error::CONFLICT, provider));
                }
                *target = Some((key.to_vec(), provider));
                Ok(())
            }),
        },
    )
}
fn imported(library: &Rc<Library>, roundtrip: bool) -> ResourceProvider {
    let raw = directory(library);
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
#[test]
fn default_creation_stages_the_same_key_as_lookup_and_retains_code() {
    for roundtrip in [false, true] {
        let library = Library::compile_source("namespace.c");
        let mut parent = imported(&library, roundtrip);
        let staged = Rc::new(RefCell::new(None));
        let creation = target(staged.clone(), false);
        let original = value(parent.lookup(b"disk").unwrap());
        let pending = parent.create_child(b"new", &creation).unwrap();
        let observed = value(parent.lookup(b"new").unwrap());
        assert_eq!(super::key(&observed), staged.borrow().as_ref().unwrap().0);
        assert!(matches!(
            parent.create_child(b"new", &creation),
            Err(Error::CONFLICT)
        ));
        assert_eq!(parent.stop(), Err(Error::BUSY));
        assert_eq!(value(pending), Value::unit());
        let (key, mut child) = staged.borrow_mut().take().unwrap();
        assert_ne!(key.as_slice(), super::key(&original));
        assert_eq!(key, super::key(&observed));
        parent.stop().unwrap();
        drop(parent);
        assert_eq!(library.count(c"fixture_directories_destroyed"), 0);
        assert_eq!(
            number(&value(
                child.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap()
            )),
            0
        );
        child.stop().unwrap();
        drop(child);
        drop(creation);
        assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
    }
}
#[test]
fn rejected_key_staging_neither_publishes_name_nor_transfers_ownership() {
    for roundtrip in [false, true] {
        let library = Library::compile_source("namespace.c");
        let mut parent = imported(&library, roundtrip);
        let staged = Rc::new(RefCell::new(None));
        let creation = target(staged.clone(), true);
        assert!(matches!(
            parent.create_child(b"new", &creation),
            Err(Error::DENIED)
        ));
        assert!(staged.borrow().is_none());
        assert!(matches!(parent.lookup(b"new"), Err(Error::UNAVAILABLE)));
        assert_eq!(library.count(c"fixture_destroyed"), 1);
        assert_eq!(library.count(c"fixture_creations"), 0);
        parent.stop().unwrap();
    }
}
#[test]
fn uncertain_creation_preserves_its_key_and_never_creates_twice() {
    let library = Library::compile_source("namespace.c");
    let mut parent = imported(&library, true);
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_create_unknown") };
    unsafe { uncertain() };
    let staged = Rc::new(RefCell::new(None));
    let creation = target(staged.clone(), false);
    let mut pending = parent.create_child(b"new", &creation).unwrap();
    ready(&mut pending);
    let outcome = pending.take().unwrap();
    assert!(matches!(outcome.outcome, Outcome::Unknown { .. }));
    drop(outcome);
    assert_eq!(pending.finish(), Err(Error::BUSY));
    let observed = value(parent.lookup(b"new").unwrap());
    assert_eq!(super::key(&observed), staged.borrow().as_ref().unwrap().0);
    pending.reconcile().unwrap();
    assert_eq!(value(pending), Value::unit());
    assert_eq!(library.count(c"fixture_creations"), 1);
    let (_, mut child) = staged.borrow_mut().take().unwrap();
    child.stop().unwrap();
    drop(child);
    parent.stop().unwrap();
}
