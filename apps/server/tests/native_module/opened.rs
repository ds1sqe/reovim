use super::*;

#[test]
fn service_create_returns_initialized_access_to_the_named_subject() {
    let (mut runtime, mut peer) = loaded("open_namespace_module.c", 3);
    let parent = peer.open(&mut runtime, b"/mount/directory").references[0];
    peer.call(
        &mut runtime,
        Operation::SetAccess,
        parent,
        0,
        &3_u32.to_le_bytes(),
    );
    let mut name = 3_u32.to_le_bytes().to_vec();
    name.extend_from_slice(b"new");
    let created = peer.call(&mut runtime, Operation::Create, parent, 0, &name);
    let reopened = peer.open(&mut runtime, b"/mount/directory/new");
    assert_eq!(&created.data[..24], &reopened.data[..24]);
    let a = created.references[0];
    let b = reopened.references[0];
    let info = peer.call(&mut runtime, Operation::Info, a, 0, &[]);
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 3);
    peer.call(&mut runtime, Operation::SetAccess, b, 0, &1_u32.to_le_bytes());
    assert_eq!(byte(&mut peer, &mut runtime, a), b'a');
    assert_eq!(byte(&mut peer, &mut runtime, a), b'b');
    assert_eq!(byte(&mut peer, &mut runtime, b), b'a');
    peer.call(&mut runtime, Operation::Close, a, 0, &[]);
    peer.call(&mut runtime, Operation::Close, b, 0, &[]);
    peer.call(&mut runtime, Operation::Close, parent, 0, &[]);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

fn byte(peer: &mut Peer, runtime: &mut Runtime, token: AccessId) -> u8 {
    let value =
        peer.call(runtime, Operation::Read, token, 0, &1_u64.to_le_bytes());
    value.data[4]
}

#[test]
fn service_open_bind_and_capture_keep_one_identity_and_selected_cursor() {
    let (mut runtime, mut peer) = loaded("open_namespace_module.c", 0);
    let first = peer.open(&mut runtime, b"/mount/directory/item");
    let second = peer.open(&mut runtime, b"/mount/directory/item");
    assert_eq!(&first.data[..24], &second.data[..24]);
    let a = first.references[0];
    let b = second.references[0];
    let info = peer.call(&mut runtime, Operation::Info, a, 0, &[]);
    assert_eq!(&info.data[..24], &first.data[..24]);
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 0);
    peer.call(&mut runtime, Operation::SetAccess, a, 0, &1_u32.to_le_bytes());
    peer.call(&mut runtime, Operation::SetAccess, b, 0, &1_u32.to_le_bytes());
    assert_eq!(byte(&mut peer, &mut runtime, a), b'a');
    assert_eq!(byte(&mut peer, &mut runtime, b), b'a');
    let bound = bind(&mut peer, &mut runtime, a);
    let captured = peer
        .call(&mut runtime, Operation::Capture, bound, 0, &1_u32.to_le_bytes())
        .references[0];
    assert_eq!(byte(&mut peer, &mut runtime, captured), b'b');
    let position = peer.call(&mut runtime, Operation::Ioctl, bound, 1, &[]);
    assert_eq!(
        u64::from_le_bytes(position.data.as_ref().try_into().unwrap()),
        2
    );
    peer.call(&mut runtime, Operation::Close, bound, 0, &[]);
    assert_eq!(byte(&mut peer, &mut runtime, captured), b'c');
    assert_eq!(byte(&mut peer, &mut runtime, b), b'b');
    peer.call(&mut runtime, Operation::Close, captured, 0, &[]);
    peer.call(&mut runtime, Operation::Close, b, 0, &[]);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
