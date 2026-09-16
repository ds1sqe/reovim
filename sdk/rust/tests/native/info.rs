use super::*;
use reovim_kernel_interface::{declaration::Declaration, provider::IoInfo};
use std::rc::Rc;

struct Info {
    identity: ResourceId,
    access: u32,
    flags: u32,
    times: [Option<(i64, u32)>; 3],
    io: Option<IoInfo>,
    interfaces: Vec<Declaration>,
}

fn take<const N: usize>(data: &mut &[u8]) -> [u8; N] {
    let (value, rest) = data.split_at(N);
    *data = rest;
    value.try_into().unwrap()
}

fn decode(value: &Value) -> Info {
    assert!(value.references.is_empty());
    let data = &mut value.data.as_ref();
    let identity = ResourceId {
        epoch: take(data),
        serial: u64::from_le_bytes(take(data)),
    };
    let access = u32::from_le_bytes(take(data));
    let flags = u32::from_le_bytes(take(data));
    let times =
        core::array::from_fn(|_| match u32::from_le_bytes(take(data)) {
            0 => None,
            1 => Some((
                i64::from_le_bytes(take(data)),
                u32::from_le_bytes(take(data)),
            )),
            _ => panic!("unknown timestamp choice"),
        });
    let io = match u32::from_le_bytes(take(data)) {
        0 => None,
        1 => {
            let io = IoInfo {
                read_modes: u32::from_le_bytes(take(data)),
                write_modes: u32::from_le_bytes(take(data)),
                alignment: u64::from_le_bytes(take(data)),
                transfer_limit: u64::from_le_bytes(take(data)),
            };
            assert_eq!(u32::from_le_bytes(take(data)), 0); // No receive Type.
            assert_eq!(u32::from_le_bytes(take(data)), 0); // No send Type.
            Some(io)
        }
        _ => panic!("unknown I/O choice"),
    };
    let count = u32::from_le_bytes(take(data));
    let interfaces = (0..count)
        .map(|_| {
            let size = u32::from_le_bytes(take(data)) as usize;
            let (encoded, rest) = data.split_at(size);
            *data = rest;
            Declaration::decode(encoded).unwrap()
        })
        .collect();
    assert_eq!(*data, []);
    Info { identity, access, flags, times, io, interfaces }
}

#[test]
fn info_copies_held_rights_and_retries_the_original_passive_observation() {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let mut runtime = Runtime::new([0x94; 16], Limits::default()).unwrap();
    let id = runtime.register(b"/info", provider, None).ok().unwrap();
    let caller = runtime.bind(&[grant(id, 7)]).unwrap().caller();
    let export = Rc::new(ExportedCaller::new(caller, LIMITS, 8).unwrap());
    let caller =
        unsafe { reovim_sdk::import_caller(export.raw(), export, LIMITS, 64) }
            .unwrap();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let opened = acquire(&caller, id, cx);
    let request = call(&caller, Operation::Info, opened, Value::unit());
    caller.submit(request.clone()).unwrap();
    let original = outcome(&caller, request.request, cx);
    let Outcome::Ok(value) = original.as_ref() else { panic!("missing info") };
    let info = decode(value);
    assert_eq!(info.identity, id);
    assert_eq!(info.access, 0);
    assert_eq!(info.flags, 0);
    assert_eq!(info.times, [None; 3]);
    assert_eq!(info.io, None);
    assert_eq!(info.interfaces, vec![discovery::first(&caller, opened, cx)]);
    success(
        &caller,
        call(
            &caller,
            Operation::SetAccess,
            opened,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    caller.submit(request.clone()).unwrap();
    assert_eq!(*outcome(&caller, request.request, cx), *original);
    let latest = success(
        &caller,
        call(&caller, Operation::Info, opened, Value::unit()),
        cx,
    );
    assert_eq!(decode(&latest).access, 1);
    assert_eq!(library.count(c"fixture_released"), 0);
    caller.release(request.request).unwrap();
    drop(original);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

#[test]
fn info_reports_byte_support_and_rejects_incomplete_observations() {
    for (bytes, elements, accepted) in
        [(84, 20, true), (83, 20, false), (84, 19, false)]
    {
        let mut limits = Limits::default();
        limits.values.bytes = bytes;
        limits.values.elements = elements;
        let mut runtime = Runtime::new([0x95; 16], limits).unwrap();
        let provider = crate::memory::provider(b"opaque\xff").unwrap();
        let id = runtime.register(b"/io", provider, None).ok().unwrap();
        let caller = runtime.bind(&[grant(id, 1)]).unwrap().caller();
        runtime.start().unwrap();
        let cx = &mut Context::from_waker(Waker::noop());
        let opened = acquire(&caller, id, cx);
        let request = call(&caller, Operation::Info, opened, Value::unit());
        if accepted {
            let info = decode(&success(&caller, request, cx));
            assert_eq!(info.identity, id);
            assert_eq!(
                info.io,
                Some(IoInfo {
                    read_modes: 2,
                    write_modes: 0,
                    alignment: 1,
                    transfer_limit: u64::from(u32::MAX),
                })
            );
            assert_eq!(info.interfaces, Vec::<Declaration>::new());
            assert_eq!(info.times, [None; 3]);
        } else {
            assert_eq!(caller.submit(request.clone()), Err(Error::PRESSURE));
            caller.release(request.request).unwrap();
        }
        let request = call(&caller, Operation::Info, opened, Value::unit());
        caller.release(request.request).unwrap();
        runtime.advance(cx).unwrap();
        runtime.close().unwrap();
        assert_eq!(caller.submit(request), Err(Error::CLOSED));
    }
}
