use super::*;

struct Host {
    runtime: Runtime,
    caller: Caller,
    imports: Caller,
    library: Rc<Library>,
    device: AccessId,
    source: AccessId,
    identity: ResourceId,
}

fn setup() -> Host {
    let (library, provider) = native("packet_refs.c");
    let set: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_uncertain") };
    unsafe { set() };
    let mut runtime = Runtime::new([0x68; 16], Limits::default()).unwrap();
    let imports = runtime.bind(&[]).unwrap();
    let device = runtime
        .register(b"/packets", provider, Some(&imports))
        .map_err(|(error, _)| error)
        .unwrap();
    let identity = runtime
        .register(b"/source", crate::memory::provider(b"owned").unwrap(), None)
        .map_err(|(error, _)| error)
        .unwrap();
    let caller =
        runtime.bind(&[grant(device, 2), grant(identity, 1)]).unwrap().caller();
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let device = acquire(&caller, device, cx);
    let source = acquire(&caller, identity, cx);
    set_access(&caller, device, 2, cx);
    Host {
        runtime,
        caller,
        imports: imports.caller(),
        library,
        device,
        source,
        identity,
    }
}

fn set_access(
    caller: &Caller,
    opened: AccessId,
    rights: u32,
    cx: &mut Context<'_>,
) {
    success(
        caller,
        call(
            caller,
            Operation::SetAccess,
            opened,
            Value::copy(&rights.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
}

fn packet(host: &Host) -> Call {
    send(
        &host.caller,
        host.device,
        Value::copy(&[0; 4], &[host.source]).unwrap(),
    )
}

#[test]
fn send_reference_loans_preserve_authority_until_original_cleanup() {
    let mut host = setup();
    let cx = &mut Context::from_waker(Waker::noop());
    let denied = packet(&host);
    assert_eq!(host.caller.submit(denied.clone()), Err(Error::DENIED));
    host.caller.release(denied.request).unwrap();
    assert_eq!(host.library.count(c"fixture_sent"), 0);
    set_access(&host.caller, host.source, 1, cx);
    let request = packet(&host);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    assert!(matches!(
        outcome(&host.caller, request.request, cx).as_ref(),
        Outcome::Unknown { .. }
    ));
    let loan = AccessId(host.library.count(c"fixture_reference"));
    let close =
        call(&host.caller, Operation::Close, host.source, Value::unit());
    host.caller.submit(close.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    assert!(matches!(
        host.caller.result(close.request, cx).unwrap(),
        Status::Pending
    ));
    let info = success(
        &host.imports,
        call(&host.imports, Operation::Info, loan, Value::unit()),
        cx,
    );
    assert_eq!(&info.data[..16], &host.identity.epoch);
    assert_eq!(&info.data[16..24], &host.identity.serial.to_le_bytes());
    assert_eq!(&info.data[24..28], &1_u32.to_le_bytes());
    let amplify = call(
        &host.imports,
        Operation::SetAccess,
        loan,
        Value::copy(&2_u32.to_le_bytes(), &[]).unwrap(),
    );
    assert_eq!(host.imports.submit(amplify.clone()), Err(Error::DENIED));
    host.imports.release(amplify.request).unwrap();
    host.caller.reconcile(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let expired = call(&host.imports, Operation::Info, loan, Value::unit());
    assert_eq!(host.imports.submit(expired.clone()), Err(Error::DENIED));
    host.imports.release(expired.request).unwrap();
    assert_eq!(host.library.count(c"fixture_sent"), 1);
    assert_eq!(
        outcome(&host.caller, close.request, cx).as_ref(),
        &Outcome::Ok(Value::unit())
    );
    host.caller.release(close.request).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_destroyed"), 1);
}
