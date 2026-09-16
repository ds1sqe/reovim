use super::*;

#[test]
fn service_recovers_uncertain_packet_acceptance_without_resending() {
    service_packet(false);
}

#[test]
fn service_recovers_original_received_packet_after_result_loss() {
    service_packet(true);
}

fn service_packet(receive: bool) {
    let library = Library::compile_source("packet.c");
    let set: unsafe extern "C" fn() =
        unsafe { library.symbol(c"fixture_uncertain") };
    unsafe { set() };
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let mut runtime = Runtime::new([0x69; 16], Limits::default()).unwrap();
    let id = runtime
        .register(b"/packets", provider, None)
        .map_err(|(error, _)| error)
        .unwrap();
    let endpoint = Endpoint::new(
        TOKEN,
        runtime
            .bind(&[grant(id, if receive { 1 } else { 2 })])
            .unwrap()
            .caller(),
    );
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let mut peer = Peer::new();
    let mut path = 0_u32.to_le_bytes().to_vec();
    path.extend_from_slice(&sized(b"/packets").data);
    let opened = peer
        .common(
            &endpoint,
            Operation::Acquire,
            AccessId(0),
            Value::copy(&path, &[]).unwrap(),
        )
        .references[0];
    peer.common(
        &endpoint,
        Operation::SetAccess,
        opened,
        Value::copy(&(if receive { 1_u32 } else { 2 }).to_le_bytes(), &[])
            .unwrap(),
    );
    let request = peer.request(
        &endpoint,
        if receive { Operation::Receive } else { Operation::Send },
        opened,
        if receive {
            Value::copy(
                &reovim_kernel_interface::provider::Receive {
                    maximum: 8,
                    oversize:
                        reovim_kernel_interface::provider::Oversize::Reject,
                }
                .encode(),
                &[],
            )
            .unwrap()
        } else {
            Value::copy(&[77, 0, 0, 0, 2, 0, 0, 0, 0xff, 0], &[]).unwrap()
        },
    );
    let reply = peer.deliver(&endpoint, Action::Call(request.clone()));
    assert_eq!(reply, Reply::Accepted);
    peer.lose_reply(&reply);
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let mut peer = Peer::new();
    assert_eq!(
        peer.exchange(&endpoint, Action::Call(request.clone())),
        Reply::Accepted
    );
    assert!(
        matches!(peer.exchange(&endpoint, Action::Result(request.request)),
        Reply::Status(Status::Outcome(outcome))
            if matches!(outcome.as_ref(), Outcome::Unknown { .. }))
    );
    assert_eq!(
        peer.exchange(&endpoint, Action::Reconcile(request.request)),
        Reply::Accepted
    );
    runtime.advance(cx).unwrap();
    let result = peer.deliver(&endpoint, Action::Result(request.request));
    peer.lose_reply(&result);
    let mut peer = Peer::new();
    let recovered = peer.exchange(&endpoint, Action::Result(request.request));
    assert_eq!(recovered, result);
    assert!(matches!(&recovered, Reply::Status(Status::Outcome(outcome))
        if outcome.as_ref() == &Outcome::Ok(expected(receive))));
    peer.adopt_and_release(&endpoint, request.request);
    runtime.advance(cx).unwrap();
    assert_eq!(
        library.count(if receive {
            c"fixture_received"
        } else {
            c"fixture_sent"
        }),
        1
    );
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn expected(receive: bool) -> Value {
    if !receive {
        return Value::copy(&2_u64.to_le_bytes(), &[]).unwrap();
    }
    let mut item = 0_u32.to_le_bytes().to_vec();
    item.extend_from_slice(&41_u32.to_le_bytes()); // Sender metadata.
    item.extend_from_slice(&0_u32.to_le_bytes()); // Empty payload.
    item.push(0); // Not truncated.
    item.extend_from_slice(&1_u32.to_le_bytes()); // Known original length.
    item.extend_from_slice(&0_u64.to_le_bytes());
    Value::copy(&item, &[]).unwrap()
}
