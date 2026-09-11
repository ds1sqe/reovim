use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    buffer::{self, BufferState, ClosePolicy, observation},
    provider::Permissions,
};
use reovim_kernel_runtime::Runtime;
use std::future::Future;

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
#[test]
fn close_requires_owner_currentness_and_explicit_discard() {
    let mut service = reovim_server::assemble().unwrap();
    let initial = finish(
        &mut service.runtime,
        reovim_server::client::heading(&service.client),
    )
    .unwrap();
    let directory = service.client.resolve("buffers").into_inner().unwrap();
    let entries = service.client.children(directory).into_inner().unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry.resource == initial.state.buffer)
        .unwrap();
    let path = format!("buffers/{}", entry.name);
    let owned = service.runtime.resource(&path).unwrap();
    let peer = service.runtime.connect(&[(owned, Permissions::ALL)]).unwrap();
    let peer_buffer = peer.resolve(&path).into_inner().unwrap();
    let peer_state =
        BufferState { buffer: peer_buffer, revision: initial.state.revision };
    assert_eq!(
        finish(
            &mut service.runtime,
            buffer::close(&peer, peer_state, ClosePolicy::DiscardLocal)
        ),
        Err(RequestError::Failed(Error::DENIED))
    );
    assert_eq!(
        finish(
            &mut service.runtime,
            buffer::close(
                &service.client,
                initial.state,
                ClosePolicy::RejectModified
            )
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    let stale =
        BufferState { revision: initial.state.revision - 1, ..initial.state };
    assert_eq!(
        finish(
            &mut service.runtime,
            buffer::close(&service.client, stale, ClosePolicy::DiscardLocal)
        ),
        Err(RequestError::Failed(Error::STALE))
    );
    let read = Message::new(Contract::new("heading.source", 1), &[]);
    let actual = finish(
        &mut service.runtime,
        service.client.read(initial.state.buffer, &read),
    )
    .unwrap();
    assert_eq!(observation(actual).unwrap(), initial);
    finish(
        &mut service.runtime,
        buffer::close(
            &service.client,
            initial.state,
            ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    finish(
        &mut service.runtime,
        buffer::close(
            &service.client,
            initial.state,
            ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap(); // Same lifetime: idempotent.
    assert_eq!(
        finish(&mut service.runtime, peer.read(peer_buffer, &read)),
        Err(RequestError::Rejected(Error::UNSUPPORTED))
    );
    service.runtime.unregister(service.heading).unwrap();
    service.runtime.close().unwrap();
}
#[test]
fn close_rejects_before_teardown_while_other_results_are_retained() {
    let mut service = reovim_server::assemble().unwrap();
    let initial = finish(
        &mut service.runtime,
        reovim_server::client::heading(&service.client),
    )
    .unwrap();
    let read = Message::new(Contract::new("heading.source", 1), &[]);
    let waiting = service.client.read(initial.state.buffer, &read);
    assert_eq!(
        finish(
            &mut service.runtime,
            buffer::close(
                &service.client,
                initial.state,
                ClosePolicy::DiscardLocal
            )
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    let actual = finish(&mut service.runtime, waiting).unwrap();
    assert_eq!(observation(actual).unwrap(), initial);
    finish(
        &mut service.runtime,
        buffer::close(
            &service.client,
            initial.state,
            ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    service.runtime.unregister(service.heading).unwrap();
    service.runtime.close().unwrap();
}

#[test]
fn retained_model_prevents_partial_module_teardown() {
    let mut service = reovim_server::assemble().unwrap();
    let initial = finish(
        &mut service.runtime,
        reovim_server::client::heading(&service.client),
    )
    .unwrap();
    let contributions = [service.source, service.heading];
    assert_eq!(
        service.runtime.unregister_contributions(&contributions),
        Err(Error::BUSY)
    );
    let source = service.client.resolve("resource/notes").into_inner().unwrap();
    let reply = finish(
        &mut service.runtime,
        service.client.read(
            source,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"# a\n\xffopaque");
    let read = Message::new(Contract::new("heading.source", 1), &[]);
    let actual = finish(
        &mut service.runtime,
        service.client.read(initial.state.buffer, &read),
    )
    .unwrap();
    assert_eq!(observation(actual).unwrap(), initial);
    finish(
        &mut service.runtime,
        buffer::close(
            &service.client,
            initial.state,
            ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    service.runtime.unregister_contributions(&contributions).unwrap();
    assert_eq!(
        service.runtime.resource("resource/notes"),
        Err(Error::UNAVAILABLE)
    );
    assert_eq!(
        service.runtime.resource("models/heading"),
        Err(Error::UNAVAILABLE)
    );
    service.runtime.close().unwrap();
}
