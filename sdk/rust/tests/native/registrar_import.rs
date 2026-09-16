use super::*;
use reovim_sdk::ImportedRegistrar;

#[test]
fn imported_registrar_returns_rejections_and_fences_expired_native_scope() {
    let library = Library::compile_source("registrar_import.c");
    let create: unsafe extern "C" fn(*mut abi::Registrar) -> u32 =
        unsafe { library.symbol(c"fixture_registry_create") };
    let read: unsafe extern "C" fn(*const abi::Registrar) -> u64 =
        unsafe { library.symbol(c"fixture_registry_read") };
    let destroy: unsafe extern "C" fn(*const abi::Registrar) =
        unsafe { library.symbol(c"fixture_registry_destroy") };
    let mut raw = core::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(raw.as_mut_ptr()) }, 0);
    let raw = unsafe { raw.assume_init() };
    let owner = unsafe { ImportedRegistrar::new(raw, library.clone(), LIMITS) }
        .unwrap();
    let registrar = owner.registrar();
    let dropped = Rc::new(Cell::new(false));
    let (error, original) =
        registrar.stage(b"denied", provider(dropped.clone())).err().unwrap();
    assert_eq!(error, Error::DENIED);
    assert!(!dropped.get());
    registrar.stage(b"device", original).ok().unwrap();
    assert_eq!(unsafe { read(&raw const raw) }, 83);
    drop(owner);
    assert_eq!(unsafe { read(&raw const raw) }, 83);
    assert!(!dropped.get());
    unsafe { destroy(&raw const raw) };
    assert!(dropped.get());
    let before = library.count(c"fixture_registry_calls");
    let late_dropped = Rc::new(Cell::new(false));
    let (error, mut original) = registrar
        .stage(b"device", provider(late_dropped.clone()))
        .err()
        .unwrap();
    assert_eq!(error, Error::CLOSED);
    assert_eq!(library.count(c"fixture_registry_calls"), before);
    assert!(!late_dropped.get());
    original.stop().unwrap();
    drop(original);
    assert!(late_dropped.get());
}
