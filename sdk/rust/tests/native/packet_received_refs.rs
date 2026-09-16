use super::*;

struct Host {
    runtime: Runtime,
    caller: Caller,
    imports: Caller,
    source: AccessId,
    receiver: AccessId,
    library: Rc<Library>,
    identity: ResourceId,
}

fn setup(handles: usize) -> Host {
    let (library, provider) = native("packet_receive_refs.c");
    let mut runtime =
        Runtime::new([0x6a; 16], Limits { handles, ..Limits::default() })
            .unwrap();
    let identity = runtime
        .register(b"/source", crate::memory::provider(b"owned").unwrap(), None)
        .map_err(|(error, _)| error)
        .unwrap();
    let imports = runtime.bind(&[grant(identity, 1)]).unwrap();
    let receiver = runtime
        .register(b"/receiver", provider, Some(&imports))
        .map_err(|(error, _)| error)
        .unwrap();
    let caller = runtime.bind(&[grant(receiver, 7)]).unwrap().caller();
    runtime.start().unwrap();
    let imports = imports.caller();
    let cx = &mut Context::from_waker(Waker::noop());
    let source = acquire(&imports, identity, cx);
    success(
        &imports,
        call(
            &imports,
            Operation::SetAccess,
            source,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    let receiver = acquire(&caller, receiver, cx);
    flags(&caller, receiver, cx);
    let set: unsafe extern "C" fn(u64) =
        unsafe { library.symbol(c"fixture_set_reference") };
    let uncertain: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_uncertain") };
    unsafe {
        set(source.0);
        uncertain();
    }
    Host { runtime, caller, imports, source, receiver, library, identity }
}

#[test]
fn recovered_receive_transfers_owned_refs_only_after_result_adoption() {
    let mut host = setup(4);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = receive(&host.caller, host.receiver, 0, Oversize::Reject);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    assert!(matches!(
        outcome(&host.caller, request.request, cx).as_ref(),
        Outcome::Unknown { .. }
    ));
    success(
        &host.imports,
        call(&host.imports, Operation::Info, host.source, Value::unit()),
        cx,
    );
    host.caller.reconcile(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    let recovered = outcome(&host.caller, request.request, cx);
    let Outcome::Ok(value) = recovered.as_ref() else {
        panic!("{recovered:?}")
    };
    assert_eq!(value.data.as_ref(), &[0; 13]);
    let received = value.references[0];
    let provisional =
        call(&host.caller, Operation::Info, received, Value::unit());
    assert_eq!(host.caller.submit(provisional.clone()), Err(Error::DENIED));
    host.caller.release(provisional.request).unwrap();
    let consumed =
        call(&host.imports, Operation::Info, host.source, Value::unit());
    assert_eq!(host.imports.submit(consumed.clone()), Err(Error::DENIED));
    host.imports.release(consumed.request).unwrap();
    host.caller.adopt(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    let info = success(
        &host.caller,
        call(&host.caller, Operation::Info, received, Value::unit()),
        cx,
    );
    assert_eq!(&info.data[..16], &host.identity.epoch);
    assert_eq!(&info.data[16..24], &host.identity.serial.to_le_bytes());
    let read = read(&host.caller, received, Some(0), 5);
    let data = finish(&mut host.runtime, &host.caller, read, cx);
    read_result(&data, b"owned", true);
    assert_eq!(host.library.count(c"fixture_received"), 1);
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_destroyed"), 1);
}

#[test]
fn receive_reserves_output_access_before_consuming_external_observation() {
    let mut host = setup(1);
    let cx = &mut Context::from_waker(Waker::noop());
    let request = receive(&host.caller, host.receiver, 0, Oversize::Reject);
    assert_eq!(host.caller.submit(request.clone()), Err(Error::PRESSURE));
    host.caller.release(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_received"), 0);
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_destroyed"), 1);
}

#[test]
fn cancelled_receive_refunds_access_and_released_recovery_drains_owned_output()
{
    let mut host = setup(2);
    let cx = &mut Context::from_waker(Waker::noop());
    let cancelled = receive(&host.caller, host.receiver, 0, Oversize::Reject);
    host.caller.submit(cancelled.clone()).unwrap();
    assert_eq!(
        host.caller.cancel(cancelled.request),
        Ok(Cancellation::Accepted)
    );
    host.caller.release(cancelled.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_received"), 0);
    let request = receive(&host.caller, host.receiver, 0, Oversize::Reject);
    host.caller.submit(request.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    host.caller.reconcile(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let consumed =
        call(&host.imports, Operation::Info, host.source, Value::unit());
    assert_eq!(host.imports.submit(consumed.clone()), Err(Error::DENIED));
    host.imports.release(consumed.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_received"), 1);
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_destroyed"), 1);
}
