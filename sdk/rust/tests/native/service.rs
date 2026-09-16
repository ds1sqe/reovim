use reovim_kernel_interface::{
    Outcome, Value,
    caller::{Call, Operation, Status},
    resource::AccessId,
};
use reovim_kernel_runtime::{Limits, Runtime};
use reovim_service::{
    protocol::{Action, Endpoint, Frame, Reply},
    transport,
};
use std::{
    io::BufReader,
    os::unix::net::UnixStream,
    task::{Context, Waker},
};

use super::{LIMITS, Library, grant, number};

#[path = "io_service.rs"]
mod io;

#[path = "packet_service.rs"]
mod packets;

#[path = "factory_service.rs"]
mod factory;

const TOKEN: [u8; 16] = [0x40; 16];

struct Peer {
    client: UnixStream,
    server: UnixStream,
}

impl Peer {
    fn new() -> Self {
        let (client, server) = UnixStream::pair().unwrap();
        Self { client, server }
    }

    fn deliver(&mut self, endpoint: &Endpoint, action: Action) -> Reply {
        let line = Frame { caller: TOKEN, action }.encode().unwrap();
        transport::send(&mut self.client, &line).unwrap();
        let frame =
            transport::receive(&mut BufReader::new(&mut self.server)).unwrap();
        endpoint.dispatch(Frame::decode(&frame).unwrap())
    }

    fn exchange(&mut self, endpoint: &Endpoint, action: Action) -> Reply {
        let reply = self.deliver(endpoint, action).encode().unwrap();
        transport::send(&mut self.server, &reply).unwrap();
        let frame =
            transport::receive(&mut BufReader::new(&mut self.client)).unwrap();
        Reply::decode(&frame).unwrap()
    }

    fn lose_reply(mut self, reply: &Reply) {
        // Shutdown affects duplicated descriptors too, including transient
        // inheritance by concurrently compiling native fixture processes.
        self.client.shutdown(std::net::Shutdown::Both).unwrap();
        drop(self.client);
        assert!(
            transport::send(&mut self.server, &reply.encode().unwrap())
                .is_err()
        );
    }

    fn adopt_and_release(&mut self, endpoint: &Endpoint, id: u64) {
        assert_eq!(self.exchange(endpoint, Action::Adopt(id)), Reply::Accepted);
        assert_eq!(
            self.exchange(endpoint, Action::Release(id)),
            Reply::Accepted
        );
    }

    fn request(
        &mut self,
        endpoint: &Endpoint,
        operation: Operation,
        subject: AccessId,
        input: Value,
    ) -> Call {
        let Reply::Reserved(request) =
            self.exchange(endpoint, next_reservation())
        else {
            panic!("expected reservation")
        };
        Call { request, operation, subject, command: 0, expected: None, input }
    }

    fn common(
        &mut self,
        endpoint: &Endpoint,
        operation: Operation,
        subject: AccessId,
        input: Value,
    ) -> Value {
        let call = self.request(endpoint, operation, subject, input);
        let id = call.request;
        assert_eq!(
            self.exchange(endpoint, Action::Call(call)),
            Reply::Accepted
        );
        let Reply::Status(Status::Outcome(result)) =
            self.exchange(endpoint, Action::Result(id))
        else {
            panic!("expected common result")
        };
        let Outcome::Ok(value) = result.as_ref() else { panic!("expected OK") };
        assert_eq!(self.exchange(endpoint, Action::Adopt(id)), Reply::Accepted);
        assert_eq!(
            self.exchange(endpoint, Action::Release(id)),
            Reply::Accepted
        );
        value.clone()
    }
}

fn sized(bytes: &[u8]) -> Value {
    let mut data = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(bytes);
    Value::copy(&data, &[]).unwrap()
}

#[test]
fn disconnected_service_reply_recovers_original_c_effect() {
    service_recovery(b"/counter");
}

#[test]
fn service_preserves_non_utf8_paths_and_frame_delimiters() {
    service_recovery(b"/counter-\xff\t\n");
}

fn service_recovery(path: &[u8]) {
    let library = Library::compile();
    let raw = library.create();
    let provider = unsafe {
        reovim_sdk::import_provider(&raw const raw, library.clone(), LIMITS)
    }
    .unwrap();
    let mut runtime = Runtime::new([0x40; 16], Limits::default()).unwrap();
    if !path.is_ascii() {
        runtime.allow_non_ascii_names().unwrap();
    }
    let identity =
        runtime.register(path, provider, None).map_err(|(e, _)| e).unwrap();
    let caller = runtime.bind(&[grant(identity, 7)]).unwrap().caller();
    let endpoint = Endpoint::new(TOKEN, caller);
    runtime.start().unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    let mut peer = Peer::new();
    let mut locator = 0_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&sized(path).data);
    let acquired = peer.common(
        &endpoint,
        Operation::Acquire,
        AccessId(0),
        Value::copy(&locator, &[]).unwrap(),
    );
    assert_eq!(&acquired.data[..16], &identity.epoch);
    assert_eq!(&acquired.data[16..24], &identity.serial.to_le_bytes());
    let handle = acquired.references[0];
    let page = peer.common(
        &endpoint,
        Operation::Interfaces,
        handle,
        super::runtime::discovery::input(0, 65536),
    );
    let (declarations, next) = super::runtime::discovery::decode(&page);
    assert_eq!(next, 0);
    let declaration = &declarations[0];
    peer.common(
        &endpoint,
        Operation::SetAccess,
        handle,
        Value::copy(&7_u32.to_le_bytes(), &[]).unwrap(),
    );
    let bound = peer
        .common(&endpoint, Operation::Bind, handle, sized(declaration.bytes()))
        .references[0];
    let call = peer.request(
        &endpoint,
        Operation::Exec,
        bound,
        Value::copy(&5_u32.to_le_bytes(), &[]).unwrap(),
    );
    reject_foreign(&mut peer, &endpoint, &call);
    let reply = peer.deliver(&endpoint, Action::Call(call.clone()));
    assert_eq!(reply, Reply::Accepted);
    peer.lose_reply(&reply);
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let mut resumed = Peer::new();
    let unknown = resumed.exchange(&endpoint, Action::Result(call.request));
    assert!(matches!(&unknown, Reply::Status(Status::Outcome(value))
        if matches!(value.as_ref(), Outcome::Unknown { .. })));
    assert_eq!(
        resumed.exchange(&endpoint, Action::Call(call.clone())),
        Reply::Accepted
    );
    assert_eq!(
        resumed.exchange(&endpoint, Action::Reconcile(call.request)),
        Reply::Accepted
    );
    runtime.advance(cx).unwrap();
    let Reply::Status(Status::Outcome(result)) =
        resumed.exchange(&endpoint, Action::Result(call.request))
    else {
        panic!("expected recovered result")
    };
    let Outcome::Ok(value) = result.as_ref() else { panic!("expected OK") };
    assert_eq!(number(value), 46);
    assert_eq!(
        resumed.exchange(&endpoint, Action::Adopt(call.request)),
        Reply::Accepted
    );
    assert_eq!(
        resumed.exchange(&endpoint, Action::Release(call.request)),
        Reply::Accepted
    );
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_finished"), 1);
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

fn reject_foreign(peer: &mut Peer, endpoint: &Endpoint, call: &Call) {
    let frame =
        Frame { caller: [0x41; 16], action: Action::Call(call.clone()) };
    transport::send(&mut peer.client, &frame.encode().unwrap()).unwrap();
    let line =
        transport::receive(&mut BufReader::new(&mut peer.server)).unwrap();
    assert_eq!(
        endpoint.dispatch(Frame::decode(&line).unwrap()),
        Reply::Error(reovim_kernel_interface::Error::DENIED)
    );
    assert_eq!(
        peer.exchange(endpoint, Action::Result(call.request)),
        Reply::Status(Status::Reserved)
    );
}

fn next_reservation() -> Action {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    Action::Reserve(SEQUENCE.fetch_add(1, Ordering::Relaxed))
}
