use reovim_kernel_interface::interaction::history::InteractionHistoryAction;
use reovim_kernel_interface::interaction::identity::InteractionRequestIdentity;
use reovim_kernel_interface::interaction::root::{
    InteractionRootPort, RootClientHandle, RootFailure, RootOpenRequest,
    RootOpenTarget,
};
use reovim_kernel_interface::lifecycle::ShutdownOutcome;
use reovim_kernel_interface::namespace::ResourceEntry;
use reovim_kernel_interface::resource::ResourceId;
use reovim_server::assembly::{HeadingService, life};

#[path = "retirement/support.rs"]
mod support;

#[test]
fn lookup_is_passive_and_listings_are_owned_observations() {
    let (mut service, [client, _]) = attached();
    let root = service.runtime.root(client).unwrap();
    let source = service.runtime.resolve(client, "source").unwrap();
    let before = service.runtime.children(client, root).unwrap();
    assert_eq!(before, [entry("source", source)]);
    assert_eq!(service.runtime.resolve(client, ""), Ok(root));
    assert_eq!(service.runtime.resolve_at(client, source, ""), Ok(source));
    assert_eq!(
        service.runtime.resolve(client, "buffer"),
        Err(RootFailure::Unavailable)
    );
    assert_eq!(service.runtime.retained_native_bytes(), None);
    assert_eq!(service.runtime.progress(service.runtime.limits().work), Ok(0));

    service.open(0).unwrap();
    let buffer = service.runtime.resolve(client, "buffer").unwrap();
    let history = service.runtime.resolve(client, "buffer/history").unwrap();
    assert_eq!(
        service.runtime.resolve_at(client, buffer, "history"),
        Ok(history)
    );
    assert_eq!(
        service.runtime.children(client, buffer),
        Ok(vec![entry("history", history)])
    );
    let mut after = service.runtime.children(client, root).unwrap();
    assert_eq!(after, [entry("buffer", buffer), entry("source", source)]);
    after[0].name.make_ascii_uppercase();
    assert_eq!(service.runtime.resolve(client, "buffer"), Ok(buffer));
    assert_eq!(
        service.runtime.resolve(client, "BUFFER"),
        Err(RootFailure::Unavailable)
    );
    assert_eq!(before, [entry("source", source)]);
    assert_eq!(
        service.runtime.children(client, source),
        Err(RootFailure::Unsupported)
    );
    assert_eq!(
        service.runtime.children(client, history),
        Err(RootFailure::Unsupported)
    );
    service.close().unwrap();
}

#[test]
fn path_and_reference_checks_do_not_admit_work() {
    let (mut service, [first, second]) = attached();
    let source = service.runtime.resolve(first, "source").unwrap();
    assert_eq!(
        service.runtime.resolve(first, &"a".repeat(257)),
        Err(RootFailure::Pressure)
    );
    for path in [
        "/",
        "/source",
        "source/",
        "source//info",
        ".",
        "..",
        "buffer/../source",
    ] {
        assert_eq!(
            service.runtime.resolve(first, path),
            Err(RootFailure::InvalidRequest)
        );
    }
    assert_eq!(
        service.runtime.resolve_at(second, source, ""),
        Err(RootFailure::Denied)
    );
    assert_eq!(
        service.runtime.children(second, source),
        Err(RootFailure::Denied)
    );
    let forged = RootClientHandle { root: life(999), ..first };
    assert_eq!(
        service.runtime.resolve_at(forged, source, ""),
        Err(RootFailure::Denied)
    );
    assert_eq!(
        service.runtime.resolve_at(first, ResourceId(u64::MAX), ""),
        Err(RootFailure::Denied)
    );
    let request = RootOpenRequest {
        client: second,
        request: InteractionRequestIdentity::new(1),
        target: RootOpenTarget::Independent(source),
        work: service.runtime.limits().work,
    };
    assert_eq!(service.runtime.open(request), Err(RootFailure::Denied));
    assert_eq!(service.runtime.retained_native_bytes(), None);
    // Refusal did not consume this caller's request ID or choose a Buffer.
    service.open(1).unwrap();
    service.open(0).unwrap();
    assert_eq!(service.clients[0].state(), service.clients[1].state());
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01a"[..]));
    service.close().unwrap();
}

#[test]
fn resource_names_keep_identity_across_shared_edits_and_undo() {
    let mut service = support::opened();
    let first = service.clients[0].handle().unwrap();
    let second = service.clients[1].handle().unwrap();
    let buffer = service.runtime.resolve(first, "buffer").unwrap();
    let history = service.runtime.resolve(first, "buffer/history").unwrap();
    let peer_buffer = service.runtime.resolve(second, "buffer").unwrap();
    assert_ne!(buffer, peer_buffer);
    let initial = service.clients[0].state().unwrap();
    let changed = service.edit(0, 10, b"b").unwrap().accepted_state().unwrap();
    assert_ne!(changed.result().history().node(), initial.history().node());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    let restored = service
        .history(1, 11, InteractionHistoryAction::Undo)
        .unwrap()
        .accepted_state()
        .unwrap();
    assert_eq!(restored.result().history().node(), initial.history().node());
    service.receive(0).unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[0].state(), service.clients[1].state());
    assert_eq!(service.clients[0].payload(), Some(&b"H\x01a"[..]));
    assert_eq!(service.runtime.resolve(first, "buffer"), Ok(buffer));
    assert_eq!(service.runtime.resolve(first, "buffer/history"), Ok(history));
    assert_eq!(service.runtime.resolve(second, "buffer"), Ok(peer_buffer));
    service.close().unwrap();
}

#[test]
fn detached_namespace_closes_without_retiring_the_shared_buffer() {
    let mut service = support::opened();
    let client = service.clients[0].handle().unwrap();
    let root = service.runtime.root(client).unwrap();
    let buffer = service.runtime.resolve(client, "buffer").unwrap();
    let before = service.runtime.children(client, root).unwrap();
    assert_eq!(
        service.runtime.detach_client(client, support::budget(&service)),
        Ok(ShutdownOutcome::Closed)
    );
    assert_eq!(service.runtime.root(client), Err(RootFailure::Closed));
    assert_eq!(
        service.runtime.resolve_at(client, buffer, ""),
        Err(RootFailure::Closed)
    );
    assert_eq!(
        service.runtime.children(client, root),
        Err(RootFailure::Closed)
    );
    assert_eq!(before[0], entry("buffer", buffer));
    service.edit(1, 10, b"b").unwrap().accepted_state().unwrap();
    service.receive(1).unwrap();
    assert_eq!(service.clients[1].payload(), Some(&b"H\x01b"[..]));
    service.close().unwrap();
}

fn attached() -> (HeadingService, [RootClientHandle; 2]) {
    let mut service = support::constructed();
    service.runtime.start(support::budget(&service)).unwrap();
    let handles = [0, 1].map(|index| {
        let client = &service.clients[index];
        service
            .runtime
            .attach_client(client.attach_request(service.runtime.owner()))
            .unwrap()
    });
    (service, handles)
}

fn entry(name: &str, resource: ResourceId) -> ResourceEntry {
    ResourceEntry { name: name.into(), resource }
}
