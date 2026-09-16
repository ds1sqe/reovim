use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Call, Operation, Status},
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, Limits, Runtime};
use reovim_server::clients::Clients;
use reovim_service::protocol::{Action, Frame, Reply};
use std::{
    rc::Rc,
    task::{Context, Waker},
};

const FIRST: [u8; 16] = [1; 16];
const SECOND: [u8; 16] = [2; 16];

fn send(clients: &Clients, token: [u8; 16], action: Action) -> Reply {
    clients.dispatch(Frame { caller: token, action })
}
fn reserve(clients: &Clients, token: [u8; 16]) -> u64 {
    let Reply::Reserved(id) = send(clients, token, next_reservation()) else {
        panic!("reservation failed");
    };
    id
}
fn resource(runtime: &mut Runtime) -> ResourceId {
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
    runtime.register(b"/subject", provider, None).ok().unwrap()
}
fn acquire(request: u64, resource: ResourceId) -> Call {
    let mut input = 1_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&resource.epoch);
    input.extend_from_slice(&resource.serial.to_le_bytes());
    Call {
        request,
        operation: Operation::Acquire,
        subject: AccessId(0),
        command: 0,
        expected: None,
        input: Value::copy(&input, &[]).unwrap(),
    }
}
fn release(clients: &Clients, token: [u8; 16], id: u64) {
    assert_eq!(send(clients, token, Action::Release(id)), Reply::Accepted);
}
fn drive(runtime: &mut Runtime) {
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
}

#[test]
fn retirement_keeps_original_custody_and_isolates_other_credentials() {
    let mut runtime =
        reovim_server::assemble([0xb3; 16], Limits::default()).unwrap();
    let subject = resource(&mut runtime);
    let first = Rc::new(
        runtime
            .bind(&[Grant {
                resource: subject,
                access: 1,
                create_access: 0,
                retain_loans: false,
            }])
            .unwrap(),
    );
    let second = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(4).unwrap();
    clients.admit(FIRST, &first).unwrap();
    clients.admit(SECOND, &second).unwrap();
    let id = reserve(&clients, FIRST);
    let denied = reserve(&clients, SECOND);
    let call = acquire(id, subject);
    assert_eq!(
        send(&clients, FIRST, Action::Call(call.clone())),
        Reply::Accepted
    );
    let original = send(&clients, FIRST, Action::Result(id));
    assert!(matches!(&original, Reply::Status(Status::Outcome(value))
        if matches!(value.as_ref(), Outcome::Ok(_))));
    assert_eq!(
        send(&clients, SECOND, Action::Call(acquire(denied, subject))),
        Reply::Error(Error::DENIED)
    );
    assert_eq!(clients.retire(&mut runtime, FIRST), Err(Error::BUSY));
    assert_eq!(
        send(&clients, FIRST, next_reservation()),
        Reply::Error(Error::CLOSED)
    );
    assert_eq!(send(&clients, FIRST, Action::Call(call)), Reply::Accepted);
    assert_eq!(send(&clients, FIRST, Action::Result(id)), original);
    assert_eq!(send(&clients, FIRST, Action::Adopt(id)), Reply::Accepted);
    release(&clients, FIRST, id);
    let alive = reserve(&clients, SECOND);
    release(&clients, SECOND, alive);
    release(&clients, SECOND, denied);
    drive(&mut runtime);
    clients.retire(&mut runtime, FIRST).unwrap();
    clients.retire(&mut runtime, FIRST).unwrap();
    assert_eq!(
        send(&clients, FIRST, Action::Result(id)),
        Reply::Error(Error::CLOSED)
    );
    let replacement = Rc::new(runtime.bind(&[]).unwrap());
    assert_eq!(clients.admit(FIRST, &replacement), Err(Error::CLOSED));
    clients.admit([3; 16], &replacement).unwrap();
    clients.retire(&mut runtime, SECOND).unwrap();
    clients.retire(&mut runtime, [3; 16]).unwrap();
    runtime.close().unwrap();
}

#[test]
fn credentials_cannot_retarget_bindings_or_alias_an_existing_caller() {
    let mut runtime =
        reovim_server::assemble([0xb4; 16], Limits::default()).unwrap();
    let first = Rc::new(runtime.bind(&[]).unwrap());
    let second = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(2).unwrap();
    clients.admit(FIRST, &first).unwrap();
    let id = reserve(&clients, FIRST);
    clients.admit(FIRST, &first).unwrap();
    assert_eq!(clients.admit(FIRST, &second), Err(Error::CONFLICT));
    assert_eq!(clients.admit(SECOND, &first), Err(Error::CONFLICT));
    assert_eq!(clients.admit([0; 16], &second), Err(Error::INVALID));
    assert_eq!(
        send(&clients, FIRST, Action::Result(id)),
        Reply::Status(Status::Reserved)
    );
    assert_eq!(
        send(&clients, SECOND, next_reservation()),
        Reply::Error(Error::DENIED)
    );
    release(&clients, FIRST, id);
    drive(&mut runtime);
    clients.retire(&mut runtime, FIRST).unwrap();
    clients.admit(SECOND, &second).unwrap();
    runtime.close().unwrap();
}

#[test]
fn retired_credentials_remain_bounded_without_consuming_runtime_caller_slots() {
    let mut runtime = reovim_server::assemble(
        [0xb5; 16],
        Limits { callers: 1, ..Limits::default() },
    )
    .unwrap();
    let first = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(1).unwrap();
    clients.admit(FIRST, &first).unwrap();
    clients.retire(&mut runtime, FIRST).unwrap();
    let replacement = Rc::new(runtime.bind(&[]).unwrap());
    assert_eq!(clients.admit(SECOND, &replacement), Err(Error::PRESSURE));
    assert_eq!(clients.admit(FIRST, &replacement), Err(Error::CLOSED));
    assert_eq!(
        send(&clients, FIRST, next_reservation()),
        Reply::Error(Error::CLOSED)
    );
    let caller = replacement.caller();
    let id = caller.reserve().unwrap();
    caller.release(id).unwrap();
    drive(&mut runtime);
    runtime.close().unwrap();
}

#[test]
fn foreign_retirement_preserves_the_authenticated_original_caller() {
    let mut runtime =
        reovim_server::assemble([0xb6; 16], Limits::default()).unwrap();
    let mut foreign =
        reovim_server::assemble([0xb6; 16], Limits::default()).unwrap();
    let binding = Rc::new(runtime.bind(&[]).unwrap());
    let mut clients = Clients::new(1).unwrap();
    clients.admit(FIRST, &binding).unwrap();
    assert_eq!(clients.retire(&mut foreign, FIRST), Err(Error::DENIED));
    let id = reserve(&clients, FIRST);
    release(&clients, FIRST, id);
    drive(&mut runtime);
    clients.retire(&mut runtime, FIRST).unwrap();
    runtime.close().unwrap();
    foreign.close().unwrap();
}

fn next_reservation() -> Action {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    Action::Reserve(SEQUENCE.fetch_add(1, Ordering::Relaxed))
}
