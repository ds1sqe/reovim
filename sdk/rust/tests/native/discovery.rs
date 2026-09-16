use super::*;
use reovim_kernel_interface::{
    declaration::Declaration,
    provider::{ProviderInterface, ResourceOps, ResourceProvider},
};
use std::rc::Rc;

pub fn input(cursor: u64, limit: u64) -> Value {
    let mut data = cursor.to_le_bytes().to_vec();
    data.extend_from_slice(&limit.to_le_bytes());
    Value::copy(&data, &[]).unwrap()
}

pub fn decode(value: &Value) -> (Vec<Declaration>, u64) {
    assert!(value.references.is_empty());
    let (count, mut rest) = value.data.split_at(4);
    let mut declarations = Vec::new();
    for _ in 0..u32::from_le_bytes(count.try_into().unwrap()) {
        let (length, data) = rest.split_at(4);
        let size = u32::from_le_bytes(length.try_into().unwrap()) as usize;
        let (declaration, data) = data.split_at(size);
        declarations.push(Declaration::decode(declaration).unwrap());
        rest = data;
    }
    (declarations, u64::from_le_bytes(rest.try_into().unwrap()))
}

pub(super) fn first(
    caller: &Caller,
    handle: AccessId,
    cx: &mut Context<'_>,
) -> Declaration {
    let value = success(
        caller,
        call(caller, Operation::Interfaces, handle, input(0, 65536)),
        cx,
    );
    let (mut offered, next) = decode(&value);
    assert_eq!(next, 0);
    offered.remove(0)
}

fn empty_api(key: u8) -> Declaration {
    let mut data = [0; 20];
    data[0] = key;
    Declaration::decode(&data).unwrap()
}

#[test]
fn declaration_pages_are_bounded_recoverable_and_require_live_access() {
    let mut limits = Limits::default();
    limits.values.elements = 4; // Record, Sequence, one Bytes and next U64.
    let mut runtime = Runtime::new([0x68; 16], limits).unwrap();
    let offered = [empty_api(7), empty_api(9)];
    let provider = ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new(offered.clone())
            .into_iter()
            .map(|declaration| ProviderInterface {
                prepare: None,
                declaration,
                begin: None,
                create: None,
            })
            .collect(),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    let identity = runtime
        .register(b"/discovery", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 0)]).unwrap().caller();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let opened = acquire(&caller, identity, cx);
    let first = call(&caller, Operation::Interfaces, opened, input(0, 36));
    caller.submit(first.clone()).unwrap();
    let original = outcome(&caller, first.request, cx);
    caller.submit(first.clone()).unwrap();
    assert!(Rc::ptr_eq(&original, &outcome(&caller, first.request, cx)));
    let Outcome::Ok(page) = original.as_ref() else { panic!("missing page") };
    assert_eq!(decode(page), (vec![offered[0].clone()], 1));
    caller.release(first.request).unwrap();
    let page = success(
        &caller,
        call(&caller, Operation::Interfaces, opened, input(1, 36)),
        cx,
    );
    assert_eq!(decode(&page), (vec![offered[1].clone()], 0));
    let bounded = success(
        &caller,
        call(&caller, Operation::Interfaces, opened, input(0, 65536)),
        cx,
    );
    assert_eq!(decode(&bounded), (vec![offered[0].clone()], 1));
    let terminal = success(
        &caller,
        call(&caller, Operation::Interfaces, opened, input(2, 12)),
        cx,
    );
    assert_eq!(decode(&terminal), (vec![], 0));
    for (data, error) in [
        (input(0, 35), Error::PRESSURE),
        (input(2, 11), Error::PRESSURE),
        (input(3, 36), Error::INVALID),
        (Value::unit(), Error::INVALID),
        (Value::copy(&[0; 17], &[]).unwrap(), Error::INVALID),
    ] {
        let request = call(&caller, Operation::Interfaces, opened, data);
        assert_eq!(caller.submit(request.clone()), Err(error));
        caller.release(request.request).unwrap();
    }
    let closing = call(&caller, Operation::Close, opened, Value::unit());
    let closing_id = closing.request;
    caller.submit(closing).unwrap();
    let request = call(&caller, Operation::Interfaces, opened, input(0, 36));
    assert_eq!(caller.submit(request.clone()), Err(Error::CLOSED));
    caller.release(request.request).unwrap();
    runtime.advance(cx).unwrap();
    caller.release(closing_id).unwrap();
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}
