use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::integer, provider::Permissions, requests,
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{cell::Cell, future::Future, rc::Rc};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
fn register(runtime: &mut Runtime, path: &str, initial: u64) -> ResourceId {
    runtime
        .register(
            path,
            reovim_module_counter::provider(Rc::new(Cell::new(initial))),
        )
        .map_err(|(error, _)| error)
        .unwrap()
}
fn query(data: &[u8]) -> Message {
    Message::new(Contract::new("external.counter", 1), data)
}
fn read(runtime: &mut Runtime, client: &Connection, path: &str) -> u64 {
    let id = client.resolve(path).into_inner().unwrap();
    integer(&finish(runtime, client.read(id, &query(&[]))).unwrap().data)
        .unwrap()
}
fn setup() -> (Runtime, Binding, Binding, ResourceId) {
    let mut runtime = Runtime::new(188, Limits::default()).unwrap();
    runtime.start().unwrap();
    register(&mut runtime, "clients/a/env/value", 10);
    register(&mut runtime, "clients/b/env/value", 20);
    let shared = register(&mut runtime, "shared/counter", 3);
    let a = runtime.resource("clients/a").unwrap();
    let b = runtime.resource("clients/b").unwrap();
    let a = runtime
        .bind_at(a, &[(a, Permissions::ALL), (shared, Permissions::ALL)])
        .unwrap();
    let b = runtime
        .bind_at(b, &[(b, Permissions::ALL), (shared, Permissions::READ)])
        .unwrap();
    runtime.mount(&a, "shared", shared).unwrap();
    runtime.mount(&b, "shared", shared).unwrap();
    (runtime, a, b, shared)
}

#[test]
fn clients_have_private_roots_and_explicitly_shared_subjects() {
    let (mut runtime, a, b, _) = setup();
    let a = reovim_sdk::local(a.connection());
    let b = reovim_sdk::local(b.connection());
    assert_eq!(read(&mut runtime, &a, "env/value"), 10);
    assert_eq!(read(&mut runtime, &b, "env/value"), 20);
    let private = a.resolve("env/value").into_inner().unwrap();
    finish(&mut runtime, a.write(private, &query(&7_u64.to_le_bytes())))
        .unwrap();
    assert_eq!(read(&mut runtime, &a, "env/value"), 17);
    assert_eq!(read(&mut runtime, &b, "env/value"), 20);
    let shared = a.resolve("shared").into_inner().unwrap();
    finish(&mut runtime, a.write(shared, &query(&2_u64.to_le_bytes())))
        .unwrap();
    assert_eq!(read(&mut runtime, &b, "shared"), 5);
    let foreign = b.resolve("env/value").into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, a.read(foreign, &query(&[]))),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(
        a.resolve("../b/env/value").into_inner(),
        Err(RequestError::Rejected(Error::INVALID))
    );
    let names = a
        .children(a.root().into_inner().unwrap())
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|entry| entry.name)
        .collect::<Vec<_>>();
    assert_eq!(names, ["env", "requests", "shared"].map(Into::into));
    runtime.close().unwrap();
}

#[test]
fn bound_modules_share_names_without_inheriting_client_permissions() {
    let (mut runtime, a, b, shared) = setup();
    let module = runtime.bind_for(&a, &[(shared, Permissions::READ)]).unwrap();
    let imports = reovim_sdk::local(module.connection());
    runtime.mount(&a, "again", shared).unwrap();
    assert_eq!(read(&mut runtime, &imports, "again"), 3);
    assert_eq!(
        imports.resolve("env/value").into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(
        b.connection().resolve("again").into_inner(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    assert_eq!(runtime.mount(&a, "env", shared), Err(Error::INVALID));
    assert_eq!(runtime.mount(&a, "shared", shared), Err(Error::INVALID));
    let subject = imports.resolve("shared").into_inner().unwrap();
    assert_eq!(
        finish(
            &mut runtime,
            imports.write(subject, &query(&1_u64.to_le_bytes()))
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    runtime.close().unwrap();
}

#[test]
fn private_root_reconnect_recovers_original_work_without_exposing_it_to_peers()
{
    let (mut runtime, a, b, _) = setup();
    let a = reovim_sdk::local(a.connection());
    let subject = a.resolve("shared").into_inner().unwrap();
    let waiting = a.write(subject, &query(&7_u64.to_le_bytes()));
    let a = a.reconnect().unwrap();
    let RequestError::Unknown { recovery, .. } =
        finish(&mut runtime, waiting).unwrap_err()
    else {
        panic!("expected retained work")
    };
    let work = a.resolve(&recovery).into_inner().unwrap();
    assert_eq!(
        b.connection().resolve(&recovery).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let first =
        finish(&mut runtime, requests::result(&a, work)).unwrap().unwrap();
    let second =
        finish(&mut runtime, requests::result(&a, work)).unwrap().unwrap();
    assert_eq!(first, second);
    assert_eq!(integer(&first.data), Ok(10));
    a.close(work).into_inner().unwrap();
    assert_eq!(read(&mut runtime, &a, "shared"), 10);
    runtime.close().unwrap();
}

#[test]
fn buffers_remain_named_under_the_client_root_after_reconnect() {
    use reovim_kernel_interface::buffer::{self, ClosePolicy};
    let (mut runtime, a, b, _) = setup();
    runtime
        .register_model(
            "clients/a/model",
            reovim_representation_basic::heading(),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(a.connection());
    let model = client.resolve("model").into_inner().unwrap();
    let created = finish(
        &mut runtime,
        reovim_module_editing::open(&client, model, b"# Private\n"),
    )
    .unwrap();
    let directory = client.resolve("buffers").into_inner().unwrap();
    let name = client
        .children(directory)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == created.state.buffer)
        .unwrap()
        .name;
    assert_eq!(
        b.connection().resolve("buffers").into_inner(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    let client = client.reconnect().unwrap();
    let buffer =
        client.resolve(&format!("buffers/{name}")).into_inner().unwrap();
    let observed = buffer::observation(
        finish(
            &mut runtime,
            client.read(
                buffer,
                &Message::new(created.value.contract.clone(), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(observed.value, created.value);
    assert_eq!(observed.state.revision, created.state.revision);
    assert_eq!(
        client.resolve_at(buffer, "history").into_inner(),
        Ok(observed.history)
    );
    finish(
        &mut runtime,
        buffer::close(&client, observed.state, ClosePolicy::DiscardLocal),
    )
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn clients_can_share_a_session_without_sharing_their_private_root_values() {
    use reovim_kernel_interface::session;
    let (mut runtime, a, _, _) = setup();
    runtime
        .register_model(
            "clients/a/model",
            reovim_representation_basic::heading(),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(a.connection());
    let model = client.resolve("model").into_inner().unwrap();
    let created = finish(
        &mut runtime,
        session::create(
            &client,
            model,
            &Message::new(Contract::new("heading.initial", 1), b"# Shared\n"),
        ),
    )
    .unwrap();
    let directory = client.resolve("sessions").into_inner().unwrap();
    let name = client
        .children(directory)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == created.session)
        .unwrap()
        .name;
    let shared =
        runtime.resource(&format!("clients/a/sessions/{name}")).unwrap();
    let root = runtime.resource("clients/b").unwrap();
    let peer = runtime
        .bind_at(root, &[(root, Permissions::ALL), (shared, Permissions::READ)])
        .unwrap();
    runtime.mount(&peer, "shared-session", shared).unwrap();
    let peer = reovim_sdk::local(peer.connection());
    let shared = peer.resolve("shared-session").into_inner().unwrap();
    let observed = session::observation(
        finish(
            &mut runtime,
            peer.read(
                shared,
                &Message::new(created.value.contract.clone(), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(observed.value, created.value);
    assert_eq!(read(&mut runtime, &client, "env/value"), 10);
    assert_eq!(read(&mut runtime, &peer, "env/value"), 20);
    finish(&mut runtime, session::release(&peer, observed.snapshot)).unwrap();
    finish(&mut runtime, session::release(&client, created.snapshot)).unwrap();
    finish(&mut runtime, session::close(&client, created.session)).unwrap();
    runtime.close().unwrap();
}

#[test]
fn aliases_neither_duplicate_names_nor_retarget_after_source_removal() {
    let (mut runtime, a, _, shared) = setup();
    let client = reovim_sdk::local(a.connection());
    let replacement = register(&mut runtime, "clients/a/shared", 100);
    assert_eq!(read(&mut runtime, &client, "shared"), 3);
    let entries = client
        .children(client.root().into_inner().unwrap())
        .into_inner()
        .unwrap();
    let names = entries.into_iter().map(|entry| entry.name).collect::<Vec<_>>();
    assert_eq!(names, ["env", "requests", "shared"].map(Into::into));
    runtime.unregister(shared).unwrap();
    assert_eq!(
        client.resolve("shared").into_inner(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    assert_eq!(runtime.mount(&a, "shared", replacement), Err(Error::INVALID));
    runtime.close().unwrap();
}

#[test]
fn client_retirement_waits_for_adoption_and_revokes_only_its_connections() {
    let (mut runtime, a, b, shared) = setup();
    let module = runtime.bind_for(&a, &[(shared, Permissions::READ)]).unwrap();
    let imports = reovim_sdk::local(module.connection());
    let first = reovim_sdk::local(a.connection());
    let peer = reovim_sdk::local(b.connection());
    let subject = first.resolve("shared").into_inner().unwrap();
    assert_eq!(
        first.close(subject).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(read(&mut runtime, &first, "shared"), 3);
    let pending = first.write(subject, &query(&7_u64.to_le_bytes()));
    assert_eq!(runtime.retire_client(&a), Err(Error::BUSY));
    let result = finish(&mut runtime, pending).unwrap();
    assert_eq!(integer(&result.data), Ok(10));
    runtime.retire_client(&a).unwrap();
    assert_eq!(runtime.resource("clients/a"), Err(Error::UNAVAILABLE));
    assert_eq!(
        first.root().into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        imports.resolve("shared").into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(runtime.bind_for(&a, &[]).err(), Some(Error::CLOSED));
    assert_eq!(read(&mut runtime, &peer, "env/value"), 20);
    assert_eq!(read(&mut runtime, &peer, "shared"), 10);
    runtime.retire_client(&b).unwrap();
    assert!(runtime.resource("shared/counter").is_ok());
    runtime.close().unwrap();
}
