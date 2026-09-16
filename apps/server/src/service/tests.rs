use super::*;
use alloc::{boxed::Box, rc::Rc, vec};
use core::cell::Cell;
use reovim_kernel_interface::{
    Outcome, ResourceId, Value,
    caller::{Call, Operation, Status},
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, Limits};
use reovim_service::protocol::Action;
use std::{
    io::{BufReader, Read, Write},
    net::TcpStream,
};

const TOKEN: [u8; 16] = [0x51; 16];
struct Stop {
    flag: Arc<AtomicBool>,
    owner: thread::Thread,
}
impl Drop for Stop {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::Release);
        self.owner.unpark();
    }
}
fn exchange(address: std::net::SocketAddr, frame: &Frame) -> Reply {
    let mut stream = TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    reovim_service::transport::send(&mut stream, &frame.encode().unwrap())
        .unwrap();
    let line = reovim_service::transport::receive(&mut BufReader::new(stream))
        .unwrap();
    Reply::decode(&line).unwrap()
}
fn common(
    address: std::net::SocketAddr,
    sequence: u64,
    operation: Operation,
    subject: AccessId,
    input: Value,
) -> Value {
    let reserved = Frame {
        caller: TOKEN,
        action: reovim_service::protocol::Action::Reserve(sequence),
    };
    let Reply::Reserved(request) = exchange(address, &reserved) else {
        panic!("reservation failed")
    };
    let call = Frame {
        caller: TOKEN,
        action: reovim_service::protocol::Action::Call(Call {
            request,
            operation,
            subject,
            command: 0,
            expected: None,
            input,
        }),
    };
    assert_eq!(exchange(address, &call), Reply::Accepted);
    let value = outcome(address, request);
    assert_eq!(
        exchange(
            address,
            &Frame { caller: TOKEN, action: Action::Adopt(request) }
        ),
        Reply::Accepted
    );
    assert_eq!(
        exchange(
            address,
            &Frame { caller: TOKEN, action: Action::Release(request) }
        ),
        Reply::Accepted
    );
    value
}
fn outcome(address: std::net::SocketAddr, request: u64) -> Value {
    for _ in 0..128 {
        let reply = exchange(
            address,
            &Frame { caller: TOKEN, action: Action::Result(request) },
        );
        if let Some(value) = completed(reply) {
            return value;
        }
    }
    panic!("request did not settle");
}
fn resource(runtime: &mut Runtime, path: &[u8]) -> ResourceId {
    let provider = ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    runtime.register(path, provider, None).ok().unwrap()
}
fn identity_input(resource: ResourceId) -> Value {
    let mut bytes = 1_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&resource.epoch);
    bytes.extend_from_slice(&resource.serial.to_le_bytes());
    Value::copy(&bytes, &[]).unwrap()
}

#[test]
fn hosted_loop_routes_real_sockets_and_returns_custody_to_composition() {
    let mut runtime = crate::assemble([0xbb; 16], Limits::default()).unwrap();
    let subject = resource(&mut runtime, b"/subject");
    let binding = Rc::new(
        runtime
            .bind(&[Grant {
                resource: subject,
                access: 1,
                create_access: 0,
                retain_loans: false,
            }])
            .unwrap(),
    );
    let mut clients = Clients::new(2).unwrap();
    clients.admit(TOKEN, &binding).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let guard = Stop { flag: stop.clone(), owner: thread::current() };
    let client = thread::spawn(move || {
        let _stop = guard;
        assert_eq!(
            exchange(
                address,
                &Frame { caller: [0x52; 16], action: Action::Reserve(1) }
            ),
            Reply::Error(Error::DENIED)
        );
        let value = common(
            address,
            1,
            Operation::Acquire,
            AccessId(0),
            identity_input(subject),
        );
        assert_eq!(&value.data[..16], &subject.epoch);
        assert_eq!(&value.data[16..24], &subject.serial.to_le_bytes());
        common(
            address,
            2,
            Operation::SetAccess,
            value.references[0],
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        );
        let mut unfinished = TcpStream::connect(address).unwrap();
        unfinished.write_all(b"v1\t").unwrap();
        // Keep this peer open while the owner shuts down its workers.
        // Retain a receipt deliberately when stopping socket service.
        let Reply::Reserved(request) = exchange(
            address,
            &Frame { caller: TOKEN, action: Action::Reserve(3) },
        ) else {
            panic!("lost custody")
        };
        (request, unfinished)
    });
    serve(listener, &mut runtime, &clients, &mut [], &stop).unwrap();
    let (retained, mut unfinished) = client.join().unwrap();
    unfinished.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut bytes = vec![];
    let ended = unfinished.read_to_end(&mut bytes);
    assert!(
        ended.is_ok()
            || ended.unwrap_err().kind() == io::ErrorKind::ConnectionReset
    );
    assert_eq!(bytes.as_slice(), b"");
    assert_eq!(
        clients.dispatch(Frame {
            caller: TOKEN,
            action: Action::Result(retained)
        }),
        Reply::Status(Status::Reserved)
    );
    assert_eq!(clients.retire(&mut runtime, TOKEN), Err(Error::BUSY));
    assert_eq!(
        clients.dispatch(Frame {
            caller: TOKEN,
            action: Action::Release(retained)
        }),
        Reply::Accepted
    );
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    clients.retire(&mut runtime, TOKEN).unwrap();
    runtime.close().unwrap();
    // stop() joined the listener; its listening socket is no longer retained.
    assert!(TcpStream::connect(address).is_err());
}

#[test]
fn host_loop_drives_loader_work_queued_before_socket_service_starts() {
    use crate::loading::Target;
    let mut runtime = crate::assemble([0xbc; 16], Limits::default()).unwrap();
    let registry = resource(&mut runtime, b"/catalog");
    let recipient = Rc::new(runtime.bind(&[]).unwrap());
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    let loader = Loader::new(
        Target {
            registry,
            namespace: registry,
            imports: Rc::new(runtime.bind(&[]).unwrap()),
            recipient,
            approved: Box::new([]),
        },
        Rc::new(move |_| {
            seen.set(seen.get() + 1);
            Err(Error::UNAVAILABLE)
        }),
        1,
        Limits::default().values,
    )
    .unwrap();
    let mut provider = loader.provider().unwrap();
    let mut pending = provider
        .begin(
            0,
            reovim_kernel_interface::declaration::Operation::Exec,
            &Value::copy(&[1, 0, 0, 0, b'x'], &[]).unwrap(),
        )
        .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let done = stop.clone();
    let mut loaders = vec![loader];
    // A normal transport observation stops the loop after its owner turn.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let guard = Stop { flag: done, owner: thread::current() };
    let peer = thread::spawn(move || {
        let _stop = guard;
        assert_eq!(
            exchange(
                address,
                &Frame {
                    caller: TOKEN,
                    action: reovim_service::protocol::Action::Reserve(1),
                }
            ),
            Reply::Error(Error::DENIED)
        );
    });
    serve(
        listener,
        &mut runtime,
        &Clients::new(1).unwrap(),
        &mut loaders,
        &stop,
    )
    .unwrap();
    peer.join().unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        pending.take().unwrap().outcome,
        Outcome::Failed(Error::UNAVAILABLE)
    );
    pending.finish().unwrap();
    provider.stop().unwrap();
    runtime.close().unwrap();
}

fn completed(reply: Reply) -> Option<Value> {
    let Reply::Status(Status::Outcome(outcome)) = reply else {
        return None;
    };
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("call failed") };
    Some(value.clone())
}
