use reovim_kernel_interface::{
    Outcome, ResourceId, Value,
    caller::{Call, Operation, Status},
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, Limits};
use reovim_server::clients::Clients;
use reovim_service::protocol::{Action, Frame, Reply};
use std::{
    io::ErrorKind,
    rc::Rc,
    sync::mpsc,
    task::{Context, Waker},
    time::Duration,
};

#[path = "../src/service/transport/delivery.rs"]
mod delivery;

#[test]
fn dispatch_timeout_supplies_no_status_and_cannot_release_original_custody() {
    let mut runtime =
        reovim_server::assemble([0xb7; 16], Limits::default()).unwrap();
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
    let resource = runtime.register(b"/subject", provider, None).ok().unwrap();
    let binding = Rc::new(
        runtime
            .bind(&[Grant {
                resource,
                access: 1,
                create_access: 0,
                retain_loans: false,
            }])
            .unwrap(),
    );
    let token = [7; 16];
    let mut clients = Clients::new(1).unwrap();
    clients.admit(token, &binding).unwrap();
    let Reply::Reserved(id) =
        clients.dispatch(Frame { caller: token, action: Action::Reserve(1) })
    else {
        panic!("reservation failed")
    };
    let call = acquire(id, resource);
    let frame = Frame { caller: token, action: Action::Call(call) };
    let (send, receive) = mpsc::sync_channel(1);
    let error = delivery::response(&receive, Duration::ZERO).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    drop(receive);
    // The transport observer expired before the owner dispatched this frame.
    // Dispatch uses the reserved ID; failed delivery releases no custody.
    let reply = clients.dispatch(frame.clone());
    assert_eq!(reply, Reply::Accepted);
    assert!(send.send(reply.encode().unwrap()).is_err());
    let result = Frame { caller: token, action: Action::Result(id) };
    let original = clients.dispatch(result.clone());
    assert!(matches!(&original, Reply::Status(Status::Outcome(value))
        if matches!(value.as_ref(), Outcome::Ok(_))));
    assert_eq!(clients.dispatch(frame), Reply::Accepted);
    assert_eq!(clients.dispatch(result), original);
    assert_eq!(
        clients.dispatch(Frame { caller: token, action: Action::Adopt(id) }),
        Reply::Accepted
    );
    assert_eq!(
        clients.dispatch(Frame { caller: token, action: Action::Release(id) }),
        Reply::Accepted
    );
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    clients.retire(&mut runtime, token).unwrap();
    runtime.close().unwrap();
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
#[test]
fn only_an_actual_dispatch_reply_can_supply_pending_status() {
    let (send, receive) = mpsc::sync_channel(1);
    let pending = Reply::Status(Status::Pending).encode().unwrap();
    send.send(pending.clone()).unwrap();
    assert_eq!(delivery::response(&receive, Duration::ZERO).unwrap(), pending);
    drop(send);
    assert_eq!(
        delivery::response(&receive, Duration::ZERO).unwrap_err().kind(),
        ErrorKind::BrokenPipe
    );
}
