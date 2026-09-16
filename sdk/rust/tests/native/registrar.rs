use super::*;
use reovim_kernel_interface::provider::{
    Registrar, RegistrarOps, ResourceProvider,
};
use reovim_sdk::ExportedRegistrar;
use std::cell::RefCell;

type Stage = unsafe extern "C" fn(
    *const abi::Registrar,
    abi::Bytes,
    *const abi::Provider,
) -> u32;

#[derive(Default)]
struct Batch {
    closed: bool,
    staged: Option<(Vec<u8>, ResourceProvider)>,
}

fn registrar(batch: Rc<RefCell<Batch>>) -> Registrar {
    Registrar::new(
        batch,
        RegistrarOps {
            stage: |batch, path, provider| {
                let mut batch = batch.borrow_mut();
                if batch.closed {
                    return Err((Error::CLOSED, provider));
                }
                if path != b"device" {
                    return Err((Error::DENIED, provider));
                }
                if batch.staged.is_some() {
                    return Err((Error::CONFLICT, provider));
                }
                batch.staged = Some((path.to_vec(), provider));
                Ok(())
            },
        },
    )
}

const fn span(bytes: &[u8]) -> abi::Bytes {
    abi::Bytes { ptr: bytes.as_ptr(), len: bytes.len() }
}

#[test]
fn native_staging_transfers_only_on_acceptance_and_retains_module_lease() {
    let library = Library::compile_source("registrar.c");
    let weak = Rc::downgrade(&library);
    let stage: Stage = unsafe { library.symbol(c"fixture_stage") };
    let batch = Rc::new(RefCell::new(Batch::default()));
    let capability = registrar(batch.clone());
    let exported =
        ExportedRegistrar::new(capability.clone(), library.clone(), LIMITS);
    let raw = exported.raw();
    let provider = library.create();
    assert_eq!(
        unsafe {
            stage(&raw const raw, span(b"../escape"), &raw const provider)
        },
        Error::DENIED.code.0
    );
    assert!(batch.borrow().staged.is_none());
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    let oversized = vec![b'x'; 4097];
    assert_eq!(
        unsafe { stage(&raw const raw, span(&oversized), &raw const provider) },
        Error::PRESSURE.code.0
    );
    assert!(batch.borrow().staged.is_none());
    let mut path = b"device".to_vec();
    assert_eq!(
        unsafe { stage(&raw const raw, span(&path), &raw const provider) },
        0
    );
    path.fill(b'x');
    batch.borrow_mut().closed = true;
    let (path, mut staged) = batch.borrow_mut().staged.take().unwrap();
    assert_eq!(path, b"device");
    drop(exported);
    drop(capability);
    drop(library);
    assert!(weak.upgrade().is_some());
    let mut call =
        staged.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap();
    ready(&mut call);
    let result = call.take().unwrap();
    let Outcome::Ok(value) = &result.outcome else { panic!("missing value") };
    assert_eq!(number(value), 41);
    drop(result);
    call.finish().unwrap();
    drop(call);
    staged.stop().unwrap();
    let retained = weak.upgrade().unwrap();
    drop(staged);
    assert_eq!(retained.count(c"fixture_destroyed"), 1);
    drop(retained);
    assert!(weak.upgrade().is_none());
}

#[test]
fn copied_native_registrar_cannot_reopen_closed_staging() {
    let library = Library::compile_source("registrar.c");
    let stage: Stage = unsafe { library.symbol(c"fixture_stage") };
    let batch = Rc::new(RefCell::new(Batch::default()));
    let capability = registrar(batch.clone());
    let first =
        ExportedRegistrar::new(capability.clone(), library.clone(), LIMITS);
    let second = ExportedRegistrar::new(capability, library.clone(), LIMITS);
    let copied = first.raw();
    batch.borrow_mut().closed = true;
    let provider = library.create();
    for registrar in [copied, second.raw()] {
        assert_eq!(
            unsafe {
                stage(
                    &raw const registrar,
                    span(b"device"),
                    &raw const provider,
                )
            },
            Error::CLOSED.code.0
        );
    }
    assert!(batch.borrow().staged.is_none());
    assert_eq!(library.count(c"fixture_destroyed"), 0);
    let mut returned = unsafe {
        reovim_sdk::import_provider(
            &raw const provider,
            library.clone(),
            LIMITS,
        )
    }
    .unwrap();
    returned.stop().unwrap();
    drop(returned);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
