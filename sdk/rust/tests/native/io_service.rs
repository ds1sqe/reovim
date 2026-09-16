use super::*;

#[test]
fn service_recovers_native_byte_write_without_repeating_effect() {
    let library = Library::compile_source("io.c");
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let mut runtime = Runtime::new([0x64; 16], Limits::default()).unwrap();
    let id = runtime
        .register(b"/io", provider, None)
        .map_err(|(error, _)| error)
        .unwrap();
    let endpoint =
        Endpoint::new(TOKEN, runtime.bind(&[grant(id, 3)]).unwrap().caller());
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let mut peer = Peer::new();
    let mut path = 0_u32.to_le_bytes().to_vec();
    path.extend_from_slice(&sized(b"/io").data);
    let opened = peer
        .common(
            &endpoint,
            Operation::Acquire,
            AccessId(0),
            Value::copy(&path, &[]).unwrap(),
        )
        .references[0];
    let info = peer.common(&endpoint, Operation::Info, opened, Value::unit());
    assert_eq!(&info.data[..16], &id.epoch);
    assert_eq!(
        u64::from_le_bytes(info.data[16..24].try_into().unwrap()),
        id.serial
    );
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 0);
    assert_eq!(&info.data[32..44], &[0; 12]); // Unavailable Host time.
    assert_eq!(u32::from_le_bytes(info.data[44..48].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(info.data[48..52].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(info.data[52..56].try_into().unwrap()), 1);
    peer.common(
        &endpoint,
        Operation::SetAccess,
        opened,
        Value::copy(&3_u32.to_le_bytes(), &[]).unwrap(),
    );
    let write =
        peer.request(&endpoint, Operation::Write, opened, sized(b"\xff\0ab"));
    let reply = peer.deliver(&endpoint, Action::Call(write.clone()));
    assert_eq!(reply, Reply::Accepted);
    peer.lose_reply(&reply);
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let mut peer = Peer::new();
    assert_eq!(
        peer.exchange(&endpoint, Action::Call(write.clone())),
        Reply::Accepted
    );
    let Reply::Status(Status::Outcome(value)) =
        peer.exchange(&endpoint, Action::Result(write.request))
    else {
        panic!("missing write result")
    };
    assert_eq!(
        value.as_ref(),
        &Outcome::Ok(Value::copy(&4_u64.to_le_bytes(), &[]).unwrap())
    );
    peer.adopt_and_release(&endpoint, write.request);
    assert_eq!(library.count(c"fixture_writes"), 1);
    let mut input = 1_u64.to_le_bytes().to_vec();
    input.extend_from_slice(&8_u64.to_le_bytes());
    let read = peer.request(
        &endpoint,
        Operation::ReadAt,
        opened,
        Value::copy(&input, &[]).unwrap(),
    );
    let request = read.request;
    assert_eq!(peer.exchange(&endpoint, Action::Call(read)), Reply::Accepted);
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let Reply::Status(Status::Outcome(value)) =
        peer.exchange(&endpoint, Action::Result(request))
    else {
        panic!("missing read result")
    };
    let mut expected = sized(b"\0ab").data.into_vec();
    expected.push(1);
    assert_eq!(
        value.as_ref(),
        &Outcome::Ok(Value::copy(&expected, &[]).unwrap())
    );
    peer.adopt_and_release(&endpoint, request);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    drop(runtime);
    drop(endpoint);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}
