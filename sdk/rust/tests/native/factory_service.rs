use super::*;
use reovim_kernel_runtime::Grant;

#[test]
fn service_recovers_native_factory_and_reads_its_created_resource() {
    service_factory(false, false);
}

#[test]
fn service_recovers_created_auxiliaries_as_one_factory_result() {
    service_factory(true, false);
}

#[test]
fn service_recovers_initial_access_before_delivering_the_factory_batch() {
    service_factory(true, true);
}

fn service_factory(auxiliary: bool, opened: bool) {
    let library = fixture(opened);
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let declaration = provider.interfaces()[0].clone();
    let mut runtime = Runtime::new([0x61; 16], Limits::default()).unwrap();
    let imports = auxiliary.then(|| runtime.bind(&[]).unwrap());
    let id = runtime
        .register(b"/factory", provider, imports.as_ref())
        .map_err(|(error, _)| error)
        .unwrap();
    let caller = runtime
        .bind(&[Grant { create_access: 1, ..grant(id, 7) }])
        .unwrap()
        .caller();
    let endpoint = Endpoint::new(TOKEN, caller);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let mut peer = Peer::new();
    let bound = bind(&mut peer, &endpoint, &declaration);
    let mode = if opened {
        0_u32
    } else if auxiliary {
        9
    } else {
        1
    };
    let mut request = peer.request(
        &endpoint,
        if auxiliary { Operation::Ioctl } else { Operation::Exec },
        bound,
        Value::copy(&mode.to_le_bytes(), &[]).unwrap(),
    );
    request.command = if auxiliary { 8 } else { 0 };
    let reply = peer.deliver(&endpoint, Action::Call(request.clone()));
    assert_eq!(reply, Reply::Accepted);
    peer.lose_reply(&reply);
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let mut resumed = Peer::new();
    let unknown = resumed.exchange(&endpoint, Action::Result(request.request));
    assert!(matches!(&unknown, Reply::Status(Status::Outcome(value))
        if matches!(value.as_ref(), Outcome::Unknown { .. })));
    assert_eq!(
        resumed.exchange(&endpoint, Action::Call(request.clone())),
        Reply::Accepted
    );
    assert_eq!(
        resumed.exchange(&endpoint, Action::Reconcile(request.request)),
        Reply::Accepted
    );
    runtime.advance(cx).unwrap();
    let result = resumed.exchange(&endpoint, Action::Result(request.request));
    let Reply::Status(Status::Outcome(result)) = result else {
        panic!("missing recovered factory result")
    };
    let Outcome::Ok(value) = result.as_ref() else { panic!("factory failed") };
    let created = verify_result(value, auxiliary);
    resumed.adopt_and_release(&endpoint, request.request);
    let offset = if opened { 0x100 } else { 0 };
    read_child(
        &mut resumed,
        &endpoint,
        &mut runtime,
        created,
        0xfeed_1234 + offset,
        cx,
    );
    read_auxiliaries(
        &mut resumed,
        &endpoint,
        &mut runtime,
        value,
        auxiliary,
        offset,
        cx,
    );
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_created"), 1);
    assert_eq!(library.count(c"fixture_late_rejected"), 1);
    assert_eq!(
        library.count(c"fixture_child_destroyed"),
        if auxiliary { 3 } else { 1 }
    );
    assert_eq!(library.count(c"fixture_factory_destroyed"), 1);
    if opened {
        assert_eq!(library.count(c"fixture_primary_opens"), 1);
        assert_eq!(library.count(c"fixture_first_opens"), 1);
        assert_eq!(library.count(c"fixture_second_opens"), 1);
        assert_eq!(library.count(c"fixture_opened_destroyed"), 3);
    }
}

fn fixture(opened: bool) -> std::rc::Rc<Library> {
    let library = Library::compile_source(if opened {
        "factory_open.c"
    } else {
        "factory.c"
    });
    if opened {
        let set: unsafe extern "C" fn(u32) =
            unsafe { library.symbol(c"fixture_unknown_mask") };
        unsafe { set(2) };
    }
    library
}

fn bind(
    peer: &mut Peer,
    endpoint: &Endpoint,
    declaration: &reovim_kernel_interface::declaration::Declaration,
) -> AccessId {
    let mut path = 0_u32.to_le_bytes().to_vec();
    path.extend_from_slice(&sized(b"/factory").data);
    let acquired = peer
        .common(
            endpoint,
            Operation::Acquire,
            AccessId(0),
            Value::copy(&path, &[]).unwrap(),
        )
        .references[0];
    peer.common(
        endpoint,
        Operation::SetAccess,
        acquired,
        Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
    );
    peer.common(endpoint, Operation::Bind, acquired, sized(declaration.bytes()))
        .references[0]
}

fn read_child(
    peer: &mut Peer,
    endpoint: &Endpoint,
    runtime: &mut Runtime,
    created: AccessId,
    expected: u64,
    cx: &mut Context<'_>,
) {
    let bound = peer
        .common(
            endpoint,
            Operation::Bind,
            created,
            sized(crate::factory_child_declaration().bytes()),
        )
        .references[0];
    let mut read =
        peer.request(endpoint, Operation::Ioctl, bound, Value::unit());
    read.command = 1;
    assert_eq!(
        peer.exchange(endpoint, Action::Call(read.clone())),
        Reply::Accepted
    );
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = peer.exchange(endpoint, Action::Result(read.request));
    let Reply::Status(Status::Outcome(result)) = result else {
        panic!("missing child result")
    };
    let Outcome::Ok(value) = result.as_ref() else { panic!("child failed") };
    assert_eq!(number(value), expected);
    assert_eq!(
        peer.exchange(endpoint, Action::Release(read.request)),
        Reply::Accepted
    );
    runtime.advance(cx).unwrap();
}

fn verify_result(value: &Value, auxiliary: bool) -> AccessId {
    if auxiliary {
        assert_eq!(
            value.data.as_ref(),
            &[2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]
        );
        return value.references[2];
    }
    assert_eq!(&value.data[..4], &0_u32.to_le_bytes());
    assert_eq!(&value.data[4..], &0xcafe_5678_u64.to_le_bytes());
    value.references[0]
}

fn read_auxiliaries(
    peer: &mut Peer,
    endpoint: &Endpoint,
    runtime: &mut Runtime,
    value: &Value,
    auxiliary: bool,
    offset: u64,
    cx: &mut Context<'_>,
) {
    if auxiliary {
        read_child(
            peer,
            endpoint,
            runtime,
            value.references[0],
            0x00ab_c000 + offset,
            cx,
        );
        read_child(
            peer,
            endpoint,
            runtime,
            value.references[1],
            0x00ab_c001 + offset,
            cx,
        );
    }
}
