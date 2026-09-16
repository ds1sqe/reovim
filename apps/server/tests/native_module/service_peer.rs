use super::*;
use reovim_service::transport;
use std::{io::BufReader, os::unix::net::UnixStream};

pub(super) struct Peer {
    client: UnixStream,
    server: UnixStream,
}

impl Peer {
    pub fn new() -> Self {
        let (client, server) = UnixStream::pair().unwrap();
        Self { client, server }
    }

    fn deliver(&mut self, clients: &Clients, action: Action) -> Reply {
        let line = Frame { caller: TOKEN, action }.encode().unwrap();
        transport::send(&mut self.client, &line).unwrap();
        let line =
            transport::receive(&mut BufReader::new(&mut self.server)).unwrap();
        clients.dispatch(Frame::decode(&line).unwrap())
    }

    pub fn exchange(&mut self, clients: &Clients, action: Action) -> Reply {
        let reply = self.deliver(clients, action).encode().unwrap();
        transport::send(&mut self.server, &reply).unwrap();
        let line =
            transport::receive(&mut BufReader::new(&mut self.client)).unwrap();
        Reply::decode(&line).unwrap()
    }

    pub fn lose_reply(mut self, clients: &Clients, action: Action) {
        let reply = self.deliver(clients, action);
        self.client.shutdown(std::net::Shutdown::Both).unwrap();
        drop(self.client);
        assert!(
            transport::send(&mut self.server, &reply.encode().unwrap())
                .is_err()
        );
    }

    pub fn request(
        &mut self,
        clients: &Clients,
        operation: Operation,
        subject: AccessId,
        command: u32,
        input: &[u8],
    ) -> Call {
        let Reply::Reserved(request) =
            self.exchange(clients, Action::Reserve(sequence()))
        else {
            panic!("expected reservation");
        };
        Call {
            request,
            operation,
            subject,
            command,
            expected: None,
            input: Value::copy(input, &[]).unwrap(),
        }
    }

    pub fn outcome(
        &mut self,
        clients: &Clients,
        runtime: &mut Runtime,
        request: u64,
    ) -> Outcome {
        drive(self, clients, runtime, request)
    }

    pub fn adopt_and_release(&mut self, clients: &Clients, request: u64) {
        assert_eq!(
            self.exchange(clients, Action::Adopt(request)),
            Reply::Accepted
        );
        assert_eq!(
            self.exchange(clients, Action::Release(request)),
            Reply::Accepted
        );
    }

    pub fn invoke(
        &mut self,
        clients: &Clients,
        runtime: &mut Runtime,
        operation: Operation,
        subject: AccessId,
        command: u32,
        input: &[u8],
    ) -> Value {
        let call = self.request(clients, operation, subject, command, input);
        let id = call.request;
        assert_eq!(self.exchange(clients, Action::Call(call)), Reply::Accepted);
        let Outcome::Ok(value) = self.outcome(clients, runtime, id) else {
            panic!("expected successful operation");
        };
        self.adopt_and_release(clients, id);
        value
    }
}

fn completed(reply: Reply) -> Option<Outcome> {
    match reply {
        Reply::Status(Status::Pending) => None,
        Reply::Status(Status::Outcome(outcome)) => Some((*outcome).clone()),
        other => panic!("unexpected result: {other:?}"),
    }
}

fn drive(
    peer: &mut Peer,
    clients: &Clients,
    runtime: &mut Runtime,
    request: u64,
) -> Outcome {
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..128 {
        let reply = peer.exchange(clients, Action::Result(request));
        if let Some(outcome) = completed(reply) {
            return outcome;
        }
        runtime.advance(cx).unwrap();
    }
    panic!("module service did not settle within bounded driving");
}

pub(super) fn sequence() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    SEQUENCE.fetch_add(1, Ordering::Relaxed)
}
