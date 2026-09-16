use super::*;
#[path = "registrar_import.rs"]
mod registrar_import;
use reovim_kernel_interface::{
    call::{Cancellation, Completion, PendingOps},
    declaration::Declaration,
    provider::{ProviderInterface, ResourceOps, ResourceProvider},
};
use reovim_sdk::ExportedProvider;
use std::cell::Cell;

struct State {
    values: [u64; 2],
    dropped: Rc<Cell<bool>>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}
fn api(key: u8) -> Declaration {
    let mut data = vec![key; 16];
    data.extend_from_slice(&[
        2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 4, 0, 0, 0, 2, 3, 0, b'a', b'd', b'd', 3,
        4, 4, 2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 4, 0, b'r', b'e', b'a', b'd', 0,
        4, 0,
    ]);
    Declaration::decode(&data).unwrap()
}
fn provider(dropped: Rc<Cell<bool>>) -> ResourceProvider {
    ResourceProvider::new(
        State { values: [41, 83], dropped },
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([0x91, 0x92].map(|key| ProviderInterface {
            prepare: None,
            declaration: api(key),
            begin: Some(begin),
            create: None,
        })),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}
fn begin(
    state: &mut State,
    index: usize,
    operation: Operation,
    input: &Value,
) -> Result<Pending, Error> {
    let activity = if operation == Operation::Exec {
        state.values[index] += u64::from(u32::from_le_bytes(
            input.data.as_ref().try_into().map_err(|_| Error::INVALID)?,
        ));
        2
    } else {
        1
    };
    Ok(Pending::new(
        Some(Completion {
            outcome: Outcome::Ok(Value::copy(
                &state.values[index].to_le_bytes(),
                &[],
            )?),
            activity,
            owner: None,
        }),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value| value.take().ok_or(Error::CLOSED),
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}

#[test]
fn c_calls_rust_interfaces_and_borrowed_export_returns_the_same_provider() {
    let library = Library::compile_source("provider_export.c");
    let control: unsafe extern "C" fn(*mut abi::Provider, u32) -> u64 =
        unsafe { library.symbol(c"fixture_control") };
    let execute: unsafe extern "C" fn(*mut abi::Provider, u32) -> u64 =
        unsafe { library.symbol(c"fixture_execute") };
    let dropped = Rc::new(Cell::new(false));
    let export =
        ExportedProvider::new(provider(dropped.clone()), LIMITS).ok().unwrap();
    let mut raw = export.raw();
    assert_eq!(unsafe { control(&raw mut raw, 0) }, 41);
    assert_eq!(unsafe { execute(&raw mut raw, 1) }, 88);
    assert_eq!(unsafe { control(&raw mut raw, 0) }, 41);
    let provider = export.into_provider();
    assert!(!dropped.get());
    let export = ExportedProvider::new(provider, LIMITS).ok().unwrap();
    let mut raw = export.into_raw();
    assert_eq!(unsafe { control(&raw mut raw, 1) }, 88);
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
    assert!(dropped.get());
}

#[test]
fn c_reads_opaque_bytes_from_snapshot_and_releases_retained_state() {
    let library = Library::compile_source("provider_export.c");
    let read: unsafe extern "C" fn(*mut abi::Provider) =
        unsafe { library.symbol(c"fixture_read") };
    let data = Rc::new(*b"\0\xffab");
    let retained = Rc::downgrade(&data);
    let state = reovim_kernel_interface::provider::State::new(
        data,
        4,
        |data| {
            let result = crate::memory::provider(data.as_ref());
            drop(data);
            result
        },
        Rc::new(()),
    );
    let provider = state.snapshot().unwrap();
    let export = ExportedProvider::new(provider, LIMITS).ok().unwrap();
    let mut raw = export.into_raw();
    drop(state);
    assert!(retained.upgrade().is_some());
    unsafe { read(&raw mut raw) };
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
    assert!(retained.upgrade().is_none());
}

#[test]
fn c_writes_borrowed_bytes_into_rust_owned_state() {
    use reovim_kernel_interface::provider::{IoInfo, IoOps};
    use std::cell::RefCell;
    let library = Library::compile_source("provider_export.c");
    let write: unsafe extern "C" fn(*mut abi::Provider) -> u64 =
        unsafe { library.symbol(c"fixture_write") };
    let bytes = Rc::new(RefCell::new(Vec::<u8>::new()));
    let provider = ResourceProvider::new(
        bytes.clone(),
        ResourceOps {
            open: None,
            namespace: None,
            io: Some(IoOps {
                receive: None,
                send: None,
                info: IoInfo {
                    read_modes: 0,
                    write_modes: 1,
                    alignment: 1,
                    transfer_limit: 8,
                },
                read: None,
                write: Some(|state: &mut Rc<RefCell<Vec<u8>>>, input| {
                    state.borrow_mut().extend_from_slice(input.data);
                    Ok(Pending::new(
                        Some(Completion {
                            outcome: Outcome::Ok(Value::copy(
                                &u64::try_from(input.data.len())
                                    .unwrap()
                                    .to_le_bytes(),
                                &[],
                            )?),
                            activity: 2,
                            owner: None,
                        }),
                        PendingOps {
                            poll: |_, _| Ok(Poll::Ready(())),
                            take: |v| v.take().ok_or(Error::CLOSED),
                            cancel: |_| Ok(Cancellation::TooLate),
                            reconcile: |_| Err(Error::UNSUPPORTED),
                            finish: |_| Ok(()),
                        },
                    ))
                }),
            }),
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    let mut raw =
        ExportedProvider::new(provider, LIMITS).ok().unwrap().into_raw();
    assert_eq!(unsafe { write(&raw mut raw) }, 4);
    assert_eq!(*bytes.borrow(), b"\0\xffab");
    assert_eq!(unsafe { raw.stop.unwrap()(raw.context) }, 0);
    unsafe { raw.destroy.unwrap()(raw.context) };
}
