use reovim_kernel_interface::{Error, caller::Status};
use reovim_kernel_runtime::{Limits, Runtime};
use reovim_server::clients::Clients;
use reovim_service::protocol::{Action, Frame, Reply};
use std::{
    rc::Rc,
    task::{Context, Waker},
};

const TOKEN: [u8; 16] = [0x39; 16];
fn setup() -> (Runtime, Clients) {
    let mut runtime = reovim_server::assemble(
        [0xb8; 16],
        Limits { requests: 1, ..Limits::default() },
    )
    .unwrap();
    let binding = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(2).unwrap();
    clients.admit(TOKEN, &binding).unwrap();
    (runtime, clients)
}
const fn frame(sequence: u64) -> Frame {
    Frame { caller: TOKEN, action: Action::Reserve(sequence) }
}
fn exchange(clients: &Clients, frame: &Frame) -> Reply {
    let encoded = frame.encode().unwrap();
    let reply = clients.dispatch(Frame::decode(&encoded).unwrap());
    Reply::decode(&reply.encode().unwrap()).unwrap()
}
fn id(reply: &Reply) -> u64 {
    let Reply::Reserved(id) = reply else { panic!("reservation: {reply:?}") };
    *id
}
fn release(runtime: &mut Runtime, clients: &Clients, request: u64) {
    assert_eq!(
        exchange(
            clients,
            &Frame { caller: TOKEN, action: Action::Release(request) }
        ),
        Reply::Accepted
    );
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
}

#[test]
fn lost_reservation_ack_recovers_the_same_slot_before_and_after_release() {
    use std::{
        io::{BufReader, Write},
        os::unix::net::UnixStream,
    };
    let (mut runtime, mut clients) = setup();
    let request = frame(41).encode().unwrap();
    let (mut socket, mut server) = UnixStream::pair().unwrap();
    reovim_service::transport::send(&mut socket, &request).unwrap();
    let line =
        reovim_service::transport::receive(&mut BufReader::new(&mut server))
            .unwrap();
    let original = clients.dispatch(Frame::decode(&line).unwrap());
    // The client receives no allocated ID from this connection.
    socket.shutdown(std::net::Shutdown::Both).unwrap();
    drop(socket);
    assert!(server.write_all(original.encode().unwrap().as_bytes()).is_err());
    let recovered = exchange(&clients, &frame(41));
    assert_eq!(recovered, original);
    let request = id(&recovered);
    assert_eq!(
        exchange(
            &clients,
            &Frame { caller: TOKEN, action: Action::Result(request) }
        ),
        Reply::Status(Status::Reserved)
    );
    release(&mut runtime, &clients, request);
    assert_eq!(exchange(&clients, &frame(41)), recovered);
    // A one-record Runtime can now allocate: retries leaked no orphan slot.
    let next = exchange(&clients, &frame(42));
    assert_ne!(id(&next), request);
    assert_eq!(exchange(&clients, &frame(41)), Reply::Error(Error::CLOSED));
    release(&mut runtime, &clients, id(&next));
    clients.retire(&mut runtime, TOKEN).unwrap();
    runtime.close().unwrap();
}

#[test]
fn pressure_and_foreign_traffic_do_not_replace_the_acknowledgement() {
    let (mut runtime, mut clients) = setup();
    let first = exchange(&clients, &frame(10));
    assert_eq!(exchange(&clients, &frame(11)), Reply::Error(Error::PRESSURE));
    assert_eq!(exchange(&clients, &frame(10)), first);
    assert_eq!(
        exchange(
            &clients,
            &Frame { caller: [0x40; 16], action: Action::Reserve(u64::MAX) }
        ),
        Reply::Error(Error::DENIED)
    );
    assert_eq!(exchange(&clients, &frame(10)), first);
    release(&mut runtime, &clients, id(&first));
    let next = exchange(&clients, &frame(11));
    assert_ne!(id(&next), id(&first));
    release(&mut runtime, &clients, id(&next));
    let last = exchange(&clients, &frame(u64::MAX));
    assert_eq!(exchange(&clients, &frame(u64::MAX)), last);
    assert_eq!(clients.dispatch(frame(0)), Reply::Error(Error::INVALID));
    assert_eq!(exchange(&clients, &frame(11)), Reply::Error(Error::CLOSED));
    release(&mut runtime, &clients, id(&last));
    clients.retire(&mut runtime, TOKEN).unwrap();
    runtime.close().unwrap();
}

#[test]
fn retirement_preserves_lost_ack_recovery_until_original_custody_drains() {
    let (mut runtime, mut clients) = setup();
    let original = exchange(&clients, &frame(7));
    assert_eq!(clients.retire(&mut runtime, TOKEN), Err(Error::BUSY));
    assert_eq!(exchange(&clients, &frame(7)), original);
    assert_eq!(exchange(&clients, &frame(8)), Reply::Error(Error::CLOSED));
    release(&mut runtime, &clients, id(&original));
    clients.retire(&mut runtime, TOKEN).unwrap();
    assert_eq!(exchange(&clients, &frame(7)), Reply::Error(Error::CLOSED));
    runtime.close().unwrap();
}

#[path = "reservation/reentry.rs"]
mod reentry;

#[test]
fn separate_credentials_keep_independent_reservation_sequences() {
    let (mut runtime, mut clients) = setup();
    let other = [0x41; 16];
    let binding = Rc::new(runtime.bind(&[]).unwrap());
    clients.admit(other, &binding).unwrap();
    let first = exchange(&clients, &frame(1));
    let second = exchange(
        &clients,
        &Frame { caller: other, action: Action::Reserve(1) },
    );
    release(&mut runtime, &clients, id(&first));
    let next = exchange(&clients, &frame(2));
    assert_eq!(
        exchange(
            &clients,
            &Frame { caller: other, action: Action::Reserve(1) }
        ),
        second
    );
    assert_eq!(
        exchange(
            &clients,
            &Frame { caller: other, action: Action::Release(id(&second)) }
        ),
        Reply::Accepted
    );
    release(&mut runtime, &clients, id(&next));
    clients.retire(&mut runtime, TOKEN).unwrap();
    clients.retire(&mut runtime, other).unwrap();
    runtime.close().unwrap();
}

#[test]
fn advancing_the_sequence_does_not_retire_earlier_request_custody() {
    let mut runtime = reovim_server::assemble(
        [0xba; 16],
        Limits { requests: 2, ..Limits::default() },
    )
    .unwrap();
    let binding = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(1).unwrap();
    clients.admit(TOKEN, &binding).unwrap();
    let first = exchange(&clients, &frame(1));
    let next = exchange(&clients, &frame(2));
    assert_eq!(exchange(&clients, &frame(1)), Reply::Error(Error::CLOSED));
    assert_eq!(
        exchange(
            &clients,
            &Frame { caller: TOKEN, action: Action::Result(id(&first)) }
        ),
        Reply::Status(Status::Reserved)
    );
    assert_eq!(exchange(&clients, &frame(3)), Reply::Error(Error::PRESSURE));
    release(&mut runtime, &clients, id(&first));
    release(&mut runtime, &clients, id(&next));
    clients.retire(&mut runtime, TOKEN).unwrap();
    runtime.close().unwrap();
}
