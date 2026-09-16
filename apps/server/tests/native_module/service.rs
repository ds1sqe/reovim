use super::*;
use reovim_kernel_interface::caller::{Call, Status};
use reovim_server::clients::Clients;
use reovim_service::protocol::{Action, Frame, Reply};

#[path = "service_peer.rs"]
mod peer;
use peer::Peer;

const TOKEN: [u8; 16] = [0x98; 16];

fn bind_package(
    peer: &mut Peer,
    clients: &Clients,
    runtime: &mut Runtime,
    package: ResourceId,
) -> AccessId {
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&package.epoch);
    locator.extend_from_slice(&package.serial.to_le_bytes());
    let opened = peer
        .invoke(clients, runtime, Operation::Acquire, AccessId(0), 0, &locator)
        .references[0];
    let mut input = 0_u64.to_le_bytes().to_vec();
    input.extend_from_slice(&65536_u64.to_le_bytes());
    let page =
        peer.invoke(clients, runtime, Operation::Interfaces, opened, 0, &input);
    let length = u32::from_le_bytes(page.data[4..8].try_into().unwrap());
    let encoded = &page.data[8..8 + length as usize];
    let declaration = Declaration::decode(encoded).unwrap();
    assert_eq!(
        declaration.api(),
        &reovim_kernel_interface::module::activation::API
    );
    let mut input = length.to_le_bytes().to_vec();
    input.extend_from_slice(encoded);
    let bound = peer
        .invoke(clients, runtime, Operation::Bind, opened, 0, &input)
        .references[0];
    let denied = peer.request(clients, Operation::Exec, bound, 0, &[]);
    assert_eq!(
        peer.exchange(clients, Action::Call(denied.clone())),
        Reply::Error(Error::DENIED)
    );
    assert_eq!(
        peer.exchange(clients, Action::Release(denied.request)),
        Reply::Accepted
    );
    peer.invoke(
        clients,
        runtime,
        Operation::SetAccess,
        opened,
        0,
        &5_u32.to_le_bytes(),
    );
    bound
}

#[test]
fn socket_recovers_native_activation_access_and_original_unload() {
    let mut runtime =
        reovim_server::assemble([0x98; 16], Limits::default()).unwrap();
    let (clients, loaded, mut peer) = admit_after_connect(&mut runtime);
    let package =
        bind_package(&mut peer, &clients, &mut runtime, loaded.package);
    let activation = peer.request(&clients, Operation::Exec, package, 0, &[]);
    assert_eq!(
        peer.exchange(&clients, Action::Call(activation.clone())),
        Reply::Accepted
    );
    let completed = peer.outcome(&clients, &mut runtime, activation.request);
    let Outcome::Ok(value) = &completed else { panic!("activation failed") };
    let access = value.references[0];
    // Lose the result before adoption, then resume the same logical Caller.
    peer.lose_reply(&clients, Action::Result(activation.request));
    let mut peer = Peer::new();
    assert_eq!(
        peer.exchange(&clients, Action::Call(activation.clone())),
        Reply::Accepted
    );
    assert_eq!(
        peer.outcome(&clients, &mut runtime, activation.request),
        completed
    );
    peer.adopt_and_release(&clients, activation.request);
    let info =
        peer.invoke(&clients, &mut runtime, Operation::Info, access, 0, &[]);
    let instance = ResourceId {
        epoch: info.data[..16].try_into().unwrap(),
        serial: u64::from_le_bytes(info.data[16..24].try_into().unwrap()),
    };
    assert_eq!(Some(instance), loaded.consumer.get());
    let ready = peer.invoke(
        &clients,
        &mut runtime,
        Operation::Ioctl,
        access,
        control::STATUS,
        &[],
    );
    assert_eq!(ready.data.as_ref(), &2_u32.to_le_bytes());
    let unload =
        peer.request(&clients, Operation::Ioctl, access, control::UNLOAD, &[]);
    peer.lose_reply(&clients, Action::Call(unload.clone()));
    let mut peer = Peer::new();
    assert_eq!(
        peer.outcome(&clients, &mut runtime, unload.request),
        Outcome::Ok(Value::unit())
    );
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Closed));
    // Fresh activation cannot retarget either the old access or its receipt.
    let replacement =
        peer.invoke(&clients, &mut runtime, Operation::Exec, package, 0, &[]);
    let replacement_id = loaded.consumer.get().unwrap();
    assert_ne!(replacement_id, instance);
    assert_eq!(
        peer.exchange(&clients, Action::Call(unload.clone())),
        Reply::Accepted
    );
    assert_eq!(
        peer.outcome(&clients, &mut runtime, unload.request),
        Outcome::Ok(Value::unit())
    );
    peer.adopt_and_release(&clients, unload.request);
    let closed = peer.invoke(
        &clients,
        &mut runtime,
        Operation::Ioctl,
        access,
        control::STATUS,
        &[],
    );
    assert_eq!(closed.data.as_ref(), &4_u32.to_le_bytes());
    assert_eq!(runtime.module_status(replacement_id), Ok(Lifecycle::Ready));
    peer.invoke(
        &clients,
        &mut runtime,
        Operation::Ioctl,
        replacement.references[0],
        control::UNLOAD,
        &[],
    );
    let producer = loaded.producer.get().unwrap();
    assert_eq!(runtime.module_status(producer), Ok(Lifecycle::Ready));
    let export = runtime.module_exports(producer).unwrap()[0];
    assert_eq!(read_discovered(&mut runtime, export), 81);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

fn admit_after_connect(runtime: &mut Runtime) -> (Clients, Loaded, Peer) {
    let binding = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(8).unwrap();
    clients.admit(TOKEN, &binding).unwrap();
    let mut peer = Peer::new();
    let Reply::Reserved(retained) =
        peer.exchange(&clients, Action::Reserve(peer::sequence()))
    else {
        panic!("expected pre-load receipt");
    };
    let loaded = packages(runtime, true);
    runtime
        .grant(
            &binding,
            Grant {
                resource: loaded.package,
                access: 5,
                create_access: 0,
                retain_loans: false,
            },
        )
        .unwrap();
    assert_eq!(
        peer.exchange(&clients, Action::Result(retained)),
        Reply::Status(Status::Reserved)
    );
    assert_eq!(
        peer.exchange(&clients, Action::Release(retained)),
        Reply::Accepted
    );
    (clients, loaded, peer)
}
