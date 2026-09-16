use super::*;
use reovim_kernel_interface::{
    declaration::Declaration,
    provider::{ProviderInterface, ResourceOps, ResourceProvider, State},
};
use reovim_sdk::{ExportedState, import_state};
use std::cell::Cell;

fn bytes_state(data: Rc<[u8; 4]>) -> State {
    State::new(
        data,
        17,
        |data| {
            let provider = crate::memory::provider(data.as_ref());
            drop(data);
            provider
        },
        Rc::new(()),
    )
}

#[test]
fn c_retains_exported_state_and_snapshot_outlives_final_state_release() {
    let library = Library::compile_source("state_export.c");
    let retain: unsafe extern "C" fn(*const abi::State) -> abi::State =
        unsafe { library.symbol(c"fixture_retain_state") };
    let release: unsafe extern "C" fn(*const abi::State) =
        unsafe { library.symbol(c"fixture_release_state") };
    let snapshot: unsafe extern "C" fn(
        *const abi::State,
        *mut abi::Provider,
        *mut abi::Error,
    ) -> u32 = unsafe { library.symbol(c"fixture_state_snapshot") };
    let read: unsafe extern "C" fn(*mut abi::Provider) =
        unsafe { library.symbol(c"fixture_read") };
    let data = Rc::new(*b"\0\xffab");
    let retained = Rc::downgrade(&data);
    let export = ExportedState::new(bytes_state(data), LIMITS);
    let raw = export.raw();
    assert_eq!(raw.cost, 17);
    let held = unsafe { retain(&raw const raw) };
    drop(export);
    let mut provider = std::mem::MaybeUninit::uninit();
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            snapshot(&raw const held, provider.as_mut_ptr(), &raw mut error)
        },
        0
    );
    let mut provider = unsafe { provider.assume_init() };
    unsafe { release(&raw const held) };
    assert!(retained.upgrade().is_some());
    unsafe { read(&raw mut provider) };
    assert_eq!(unsafe { provider.stop.unwrap()(provider.context) }, 0);
    unsafe { provider.destroy.unwrap()(provider.context) };
    assert!(retained.upgrade().is_none());
}

#[test]
fn transferred_state_roundtrip_owns_one_reference_after_native_release() {
    let data = Rc::new(*b"\0\xffab");
    let retained = Rc::downgrade(&data);
    let raw = ExportedState::new(bytes_state(data), LIMITS).into_raw();
    let imported =
        unsafe { import_state(&raw const raw, Rc::new(()), LIMITS) }.unwrap();
    unsafe { ((*raw.ops).release.unwrap())(raw.data) };
    let alias = imported.clone();
    drop(imported);
    let mut snapshot = alias.snapshot().unwrap();
    drop(alias);
    let mut pending = snapshot
        .read(reovim_kernel_interface::provider::Read {
            offset: Some(1),
            maximum: 3,
        })
        .unwrap();
    assert_eq!(
        pending.poll(&mut Context::from_waker(Waker::noop())),
        Ok(Poll::Ready(()))
    );
    let completion = pending.take().unwrap();
    let Outcome::Ok(value) = &completion.outcome else {
        panic!("expected opaque bytes");
    };
    assert_eq!(value.data.as_ref(), &[3, 0, 0, 0, 0xff, b'a', b'b', 1]);
    drop(completion);
    pending.finish().unwrap();
    snapshot.stop().unwrap();
    drop(snapshot);
    assert!(retained.upgrade().is_some());
    drop(pending);
    assert!(retained.upgrade().is_none());
}

#[test]
fn failed_constructor_preserves_native_output_and_diagnostic_identity() {
    let library = Library::compile_source("state_export.c");
    let reject: unsafe extern "C" fn(
        *const abi::State,
        *mut abi::Error,
    ) -> u32 = unsafe { library.symbol(c"fixture_rejected_snapshot") };
    let diagnostic = ResourceId { epoch: [0x31; 16], serial: 91 };
    let state = State::new(
        Rc::new(diagnostic),
        3,
        |id| Err(Error::with_id(Error::PRESSURE.code.0, *id)),
        Rc::new(()),
    );
    let export = ExportedState::new(state, LIMITS);
    let raw = export.raw();
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe { reject(&raw const raw, &raw mut error) },
        Error::PRESSURE.code.0
    );
    assert_eq!(error.detail.epoch, diagnostic.epoch);
    assert_eq!(error.detail.serial, diagnostic.serial);
}

struct Oversized {
    busy: Cell<bool>,
    constructed: Cell<u32>,
    destroyed: Cell<bool>,
}
struct Rejected(Rc<Oversized>);
impl Drop for Rejected {
    fn drop(&mut self) {
        self.0.destroyed.set(true);
    }
}

fn oversized(state: Rc<Oversized>) -> Result<ResourceProvider, Error> {
    state.constructed.set(state.constructed.get() + 1);
    let interfaces = (1..=257_u32)
        .map(|id| {
            let mut bytes = vec![0x76; 16];
            bytes[..4].copy_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&[
                2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 4, 0, b'r', b'e', b'a', b'd',
                0, 4, 0,
            ]);
            ProviderInterface {
                prepare: None,
                declaration: Declaration::decode(&bytes).unwrap(),
                begin: Some(|_, _, _, _| Err(Error::UNSUPPORTED)),
                create: None,
            }
        })
        .collect();
    ResourceProvider::new(
        Rejected(state),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |state| {
                if state.0.busy.get() { Err(Error::BUSY) } else { Ok(()) }
            },
        },
        interfaces,
        Rc::new(()),
    )
    .map_err(|(error, _)| error)
}

#[test]
fn rejected_export_keeps_busy_provider_until_state_release_can_drain() {
    let library = Library::compile_source("state_export.c");
    let reject: unsafe extern "C" fn(
        *const abi::State,
        *mut abi::Error,
    ) -> u32 = unsafe { library.symbol(c"fixture_rejected_snapshot") };
    let data = Rc::new(Oversized {
        busy: Cell::new(true),
        constructed: Cell::new(0),
        destroyed: Cell::new(false),
    });
    let state = State::new(data.clone(), 3, oversized, Rc::new(()));
    let export = ExportedState::new(state, LIMITS);
    let raw = export.raw();
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe { reject(&raw const raw, &raw mut error) },
        Error::PRESSURE.code.0
    );
    assert!(!data.destroyed.get());
    assert_eq!(
        unsafe { reject(&raw const raw, &raw mut error) },
        Error::BUSY.code.0
    );
    assert_eq!(data.constructed.get(), 1);
    data.busy.set(false);
    drop(export);
    assert!(data.destroyed.get());
}
