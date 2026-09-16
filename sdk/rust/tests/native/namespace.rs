#[path = "namespace/runtime.rs"]
mod runtime;

use super::*;
use reovim_kernel_interface::provider::{
    Creation, CreationOps, NamespaceInfo, ResourceProvider,
};
use std::cell::RefCell;

type Staged = Rc<RefCell<Option<ResourceProvider>>>;
fn directory(library: &Rc<Library>) -> abi::Provider {
    let create: unsafe extern "C" fn(*mut abi::Provider) -> u32 =
        unsafe { library.symbol(c"fixture_directory") };
    let mut output = core::mem::MaybeUninit::uninit();
    assert_eq!(unsafe { create(output.as_mut_ptr()) }, 0);
    unsafe { output.assume_init() }
}
fn value(mut pending: Pending) -> Value {
    ready(&mut pending);
    let outcome = pending.take().unwrap();
    let Outcome::Ok(value) = &outcome.outcome else { panic!("not successful") };
    let value = value.clone();
    drop(outcome);
    pending.finish().unwrap();
    value
}
fn staging(state: Staged) -> Creation {
    Creation::new(
        state,
        CreationOps {
            keyed: None,
            resource: |state, provider| {
                let mut state = state.borrow_mut();
                if state.is_some() {
                    return Err((Error::CONFLICT, provider));
                }
                *state = Some(provider);
                Ok(())
            },
            auxiliary: None,
        },
    )
}
fn key(value: &Value) -> &[u8] {
    let size = u32::from_le_bytes(value.data[..4].try_into().unwrap());
    assert_eq!(usize::try_from(size).unwrap(), value.data.len() - 4);
    &value.data[4..]
}
fn page(value: &Value) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut data = &value.data[..];
    let count = u32::from_le_bytes(data[..4].try_into().unwrap());
    data = &data[4..];
    let mut entries = Vec::new();
    for _ in 0..count {
        let name = bytes(&mut data);
        let key = bytes(&mut data);
        entries.push((name, key));
    }
    assert_eq!(u64::from_le_bytes(data.try_into().unwrap()), 0);
    entries
}
fn bytes(data: &mut &[u8]) -> Vec<u8> {
    let size =
        usize::try_from(u32::from_le_bytes(data[..4].try_into().unwrap()))
            .unwrap();
    let value = data[4..4 + size].to_vec();
    *data = &data[4 + size..];
    value
}
#[test]
fn native_names_capture_keys_and_materialize_with_owned_context() {
    let library = Library::compile_source("namespace.c");
    let raw = directory(&library);
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
            .unwrap()
    };
    let offered = NamespaceInfo {
        lookup: true,
        create: true,
        list: true,
        materialize: true,
        remove: true,
        link: true,
        rename: true,
    };
    assert_eq!(provider.namespace(), Some(offered));
    let mut name = b"disk".to_vec();
    let pending = provider.lookup(&name).unwrap();
    name.fill(b'x');
    let original = value(pending);
    assert_eq!(key(&original), [0xff, 0, 1]);
    assert_eq!(key(&value(provider.lookup(b"alias").unwrap())), key(&original));
    let entries = page(&value(provider.list(0, 96).unwrap()));
    assert_eq!(
        entries,
        vec![
            (b"disk".to_vec(), key(&original).to_vec()),
            (b"alias".to_vec(), key(&original).to_vec())
        ]
    );
    assert!(matches!(provider.list(0, 12), Err(Error::PRESSURE)));
    let state = Rc::new(RefCell::new(None));
    let creation = staging(state.clone());
    assert_eq!(
        value(provider.materialize(key(&original), &creation).unwrap()),
        Value::unit()
    );
    let mut child = state.borrow_mut().take().unwrap();
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_directories_destroyed"), 0);
    assert_eq!(
        number(&value(
            child.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap()
        )),
        41
    );
    child.stop().unwrap();
    drop(child);
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}
#[test]
fn replacement_rejects_old_key_and_failed_staging_keeps_native_ownership() {
    let library = Library::compile_source("namespace.c");
    let raw = directory(&library);
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
            .unwrap()
    };
    let original = value(provider.lookup(b"disk").unwrap());
    let replace: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_replace") };
    unsafe { replace() };
    let state = Rc::new(RefCell::new(None));
    let creation = staging(state.clone());
    assert!(matches!(
        provider.materialize(key(&original), &creation),
        Err(Error::CONFLICT)
    ));
    assert!(state.borrow().is_none());
    let current = value(provider.lookup(b"disk").unwrap());
    assert_ne!(key(&current), key(&original));
    value(provider.materialize(key(&current), &creation).unwrap());
    assert!(matches!(
        provider.materialize(key(&current), &creation),
        Err(Error::CONFLICT)
    ));
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    let mut child = state.borrow_mut().take().unwrap();
    child.stop().unwrap();
    drop(child);
    provider.stop().unwrap();
    assert!(matches!(provider.lookup(b"disk"), Err(Error::CLOSED)));
}
#[test]
fn namespace_export_roundtrip_preserves_offers_and_staging_lifetime() {
    let library = Library::compile_source("namespace.c");
    let raw = directory(&library);
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
            .unwrap()
    };
    let export = reovim_sdk::ExportedProvider::new(provider, LIMITS)
        .map_err(|(error, _)| error)
        .unwrap();
    let raw = export.into_raw();
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, Rc::new(()), LIMITS)
            .unwrap()
    };
    let flags = value(provider.namespace_flags().unwrap());
    assert_eq!(u32::from_le_bytes(flags.data.as_ref().try_into().unwrap()), 1);
    assert!(matches!(provider.lookup(b"../disk"), Err(Error::INVALID)));
    assert_eq!(library.count(c"fixture_lookups"), 0);
    let key_value = value(provider.lookup(b"disk").unwrap());
    let entries = page(&value(provider.list(0, 96).unwrap()));
    assert_eq!(entries[0].1, key(&key_value));
    let state = Rc::new(RefCell::new(None));
    let creation = staging(state.clone());
    value(provider.materialize(key(&key_value), &creation).unwrap());
    value(provider.link(b"joined", key(&key_value)).unwrap());
    assert_eq!(
        page(&value(provider.list(0, 192).unwrap())).last(),
        Some(&(b"joined".to_vec(), key(&key_value).to_vec()))
    );
    value(
        provider
            .rename(reovim_kernel_interface::provider::Rename {
                name: b"joined",
                key: key(&key_value),
                destination: None,
                new_name: b"moved",
            })
            .unwrap(),
    );
    assert_eq!(
        key(&value(provider.lookup(b"moved").unwrap())),
        key(&key_value)
    );
    assert!(matches!(provider.lookup(b"joined"), Err(Error::UNAVAILABLE)));
    value(provider.remove(b"moved", key(&key_value)).unwrap());
    value(provider.remove(b"alias", key(&key_value)).unwrap());
    assert_eq!(
        page(&value(provider.list(0, 96).unwrap())),
        vec![(b"disk".to_vec(), key(&key_value).to_vec())]
    );
    provider.stop().unwrap();
    drop(provider);
    let mut child = state.borrow_mut().take().unwrap();
    assert_eq!(
        number(&value(
            child.begin(0, Operation::Ioctl(1), &Value::unit()).unwrap()
        )),
        41
    );
    child.stop().unwrap();
    drop(child);
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}

#[test]
fn namespace_stop_fences_calls_while_original_listing_drains() {
    let library = Library::compile_source("namespace.c");
    let raw = directory(&library);
    let mut provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
            .unwrap()
    };
    let pending = provider.list(0, 96).unwrap();
    assert_eq!(provider.stop(), Err(Error::BUSY));
    assert!(matches!(provider.lookup(b"disk"), Err(Error::CLOSED)));
    assert_eq!(library.count(c"fixture_lookups"), 0);
    let entries = page(&value(pending));
    assert_eq!(entries[0], (b"disk".to_vec(), vec![0xff, 0, 1]));
    provider.stop().unwrap();
    drop(provider);
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}

#[test]
fn malformed_namespace_table_leaves_ownership_with_supplier() {
    let library = Library::compile_source("namespace.c");
    let mut raw = directory(&library);
    let mut table = unsafe { *raw.namespace_ops };
    table.bytes = 0;
    raw.namespace_ops = &raw const table;
    assert!(matches!(
        unsafe {
            reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
        },
        Err(Error::INVALID)
    ));
    assert_eq!(library.count(c"fixture_directories_destroyed"), 0);
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}

#[test]
fn namespace_without_flag_observation_rejects_without_taking_ownership() {
    let library = Library::compile_source("namespace.c");
    let mut raw = directory(&library);
    let mut table = unsafe { *raw.namespace_ops };
    table.flags = None;
    raw.namespace_ops = &raw const table;
    assert!(matches!(
        unsafe {
            reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
        },
        Err(Error::INVALID)
    ));
    assert_eq!(library.count(c"fixture_directories_destroyed"), 0);
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
    assert_eq!(library.count(c"fixture_directories_destroyed"), 1);
}

#[path = "namespace/creation.rs"]
mod creation;
