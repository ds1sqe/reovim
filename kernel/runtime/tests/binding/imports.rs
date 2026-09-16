use super::*;
use reovim_kernel_interface::{
    Outcome, Pending,
    call::{Cancellation, Completion, PendingOps},
    caller::Caller,
    provider::{IoInfo, IoOps},
};
use std::{cell::Cell, task::Poll};

#[derive(Default)]
struct Device {
    reconcile: Cell<bool>,
    finish: Cell<bool>,
}
fn pending(device: Rc<Device>) -> Pending {
    Pending::new(
        device,
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |device| {
                let outcome = if device.reconcile.get() {
                    Outcome::Ok(Value::copy(&1_u64.to_le_bytes(), &[]).unwrap())
                } else {
                    Outcome::Unknown {
                        error: Error::UNAVAILABLE,
                        progress: None,
                    }
                };
                Ok(Completion { outcome, activity: 0, owner: None })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |device| {
                device.reconcile.set(true);
                Ok(())
            },
            finish: |device| {
                if device.finish.get() { Ok(()) } else { Err(Error::BUSY) }
            },
        },
    )
}
fn common(
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    input: &[u8],
) -> Value {
    let id = caller.reserve().unwrap();
    caller
        .submit(Call {
            request: id,
            operation,
            subject,
            command: 0,
            expected: None,
            input: Value::copy(input, &[]).unwrap(),
        })
        .unwrap();
    let Status::Outcome(outcome) = caller.snapshot(id).unwrap() else {
        panic!("common operation was not completed");
    };
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("common failed") };
    caller.adopt(id).unwrap();
    caller.release(id).unwrap();
    value.clone()
}
fn drive(runtime: &mut Runtime) {
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
}

#[test]
fn imports_survive_another_callers_unknown_effect_and_busy_finish() {
    let mut runtime = Runtime::new([0xa9; 16], Limits::default()).unwrap();
    let imports = runtime.bind(&[]).unwrap();
    let device = Rc::new(Device::default());
    let provider = ResourceProvider::new(
        device.clone(),
        ResourceOps {
            open: None,
            namespace: None,
            io: Some(IoOps {
                receive: None,
                send: None,
                info: IoInfo {
                    read_modes: 0,
                    write_modes: 1,
                    alignment: 1,
                    transfer_limit: 1,
                },
                read: None,
                write: Some(|device, _| Ok(pending(device.clone()))),
            }),
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    let resource =
        runtime.register(b"/output", provider, Some(&imports)).ok().unwrap();
    let consumer = runtime
        .bind(&[Grant {
            resource,
            access: 2,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap();
    runtime.start().unwrap();
    let caller = consumer.caller();
    let locator = acquire(1, resource).input;
    let opened =
        common(&caller, Operation::Acquire, AccessId(0), &locator.data)
            .references[0];
    common(&caller, Operation::SetAccess, opened, &2_u32.to_le_bytes());
    let id = caller.reserve().unwrap();
    caller
        .submit(Call {
            request: id,
            operation: Operation::Write,
            subject: opened,
            command: 0,
            expected: None,
            input: Value::copy(&[1, 0, 0, 0, 0xff], &[]).unwrap(),
        })
        .unwrap();
    assert_eq!(runtime.retire_binding(&imports), Err(Error::BUSY));
    assert_eq!(imports.caller().reserve(), Err(Error::CLOSED));
    drive(&mut runtime);
    drive(&mut runtime);
    let Status::Outcome(outcome) = caller.snapshot(id).unwrap() else {
        panic!("missing uncertain result");
    };
    assert!(matches!(outcome.as_ref(), Outcome::Unknown { .. }));
    assert_eq!(runtime.retire_binding(&imports), Err(Error::INCOMPLETE));
    caller.reconcile(id).unwrap();
    drive(&mut runtime);
    assert_eq!(runtime.retire_binding(&imports), Err(Error::BUSY));
    let Status::Outcome(outcome) = caller.snapshot(id).unwrap() else {
        panic!("missing resolved result");
    };
    assert_eq!(
        outcome.as_ref(),
        &Outcome::Ok(Value::copy(&1_u64.to_le_bytes(), &[]).unwrap())
    );
    caller.adopt(id).unwrap();
    caller.release(id).unwrap();
    device.finish.set(true);
    drive(&mut runtime);
    runtime.retire_binding(&imports).unwrap();
    let reserved = caller.reserve().unwrap();
    caller.release(reserved).unwrap();
    drive(&mut runtime);
    runtime.retire_binding(&consumer).unwrap();
    runtime.close().unwrap();
}
