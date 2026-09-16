use super::*;

#[test]
fn ascii_namespace_rejects_non_ascii_without_opt_in() {
    let library = Library::compile();
    let mut runtime = Runtime::new([0x46; 16], Limits::default()).unwrap();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let Err((error, provider)) =
        runtime.register("/内容".as_bytes(), provider, None)
    else {
        panic!("non-ASCII registration must reject")
    };
    assert_eq!(error, Error::INVALID);
    let identity = runtime
        .register(b"/content", provider, None)
        .map_err(|(e, _)| e)
        .unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    let (caller, _) = bridge(caller, true);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    rejected(&caller, "/内容".as_bytes(), Error::INVALID);
    rejected(&caller, b"/name-\xff", Error::INVALID);
    acquired(&caller, b"/content", identity, cx);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn native_paths_preserve_bytes_and_reject_malformed_structure() {
    let library = Library::compile();
    let mut runtime = Runtime::new([0x45; 16], Limits::default()).unwrap();
    runtime.allow_non_ascii_names().unwrap();
    let binary = register(&mut runtime, &library, b"/name-\xff");
    let text = register(&mut runtime, &library, "/name-\u{fffd}".as_bytes());
    let root = register(&mut runtime, &library, b"/");
    let caller = runtime
        .bind(&[grant(binary, 7), grant(text, 7), grant(root, 7)])
        .unwrap()
        .caller();
    assert_eq!(runtime.allow_non_ascii_names(), Err(Error::BUSY));
    let (caller, _) = bridge(caller, true);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    acquired(&caller, b"/name-\xff", binary, cx);
    acquired(&caller, "/name-\u{fffd}".as_bytes(), text, cx);
    acquired(&caller, b"/", root, cx);
    let malformed: &[&[u8]] = &[
        b"",
        b"relative",
        b"//name-\xff",
        b"/name-\xff/",
        b"/./name-\xff",
        b"/../name-\xff",
        b"/name-\xff\0",
    ];
    for path in malformed {
        rejected(&caller, path, Error::INVALID);
    }
    rejected(&caller, b"/name-\xfe", Error::DENIED);
    rejected(&caller, b"/name-?", Error::DENIED);
    acquired(&caller, b"/name-\xff", binary, cx);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

fn register(
    runtime: &mut Runtime,
    library: &std::rc::Rc<Library>,
    path: &[u8],
) -> ResourceId {
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    runtime.register(path, provider, None).map_err(|(e, _)| e).unwrap()
}

fn request(caller: &Caller, path: &[u8]) -> Call {
    let mut locator = 0_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&bytes(path).data);
    call(
        caller,
        Operation::Acquire,
        AccessId(0),
        Value::copy(&locator, &[]).unwrap(),
    )
}

fn acquired(
    caller: &Caller,
    path: &[u8],
    identity: ResourceId,
    cx: &mut Context<'_>,
) {
    let value = success(caller, request(caller, path), cx);
    assert_eq!(&value.data[..16], &identity.epoch);
    assert_eq!(&value.data[16..24], &identity.serial.to_le_bytes());
}

fn rejected(caller: &Caller, path: &[u8], error: Error) {
    let request = request(caller, path);
    assert_eq!(caller.submit(request.clone()), Err(error));
    assert_eq!(caller.snapshot(request.request), Ok(Status::Reserved));
    caller.release(request.request).unwrap();
}
