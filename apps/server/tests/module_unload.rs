#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Module},
    provider::{Contribution, Permissions},
    requests,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

struct Control {
    status: Cell<Lifecycle>,
    can_stop: Cell<bool>,
    close_calls: Cell<u32>,
    destroyed: Cell<bool>,
    close_error: Cell<Option<Error>>,
    advance_error: Cell<Option<Error>>,
}
struct State(Rc<Control>);
impl Drop for State {
    fn drop(&mut self) {
        self.0.destroyed.set(true);
    }
}

fn advance(state: &State) -> Result<Progress, Error> {
    let control = &state.0;
    if control.status.get() == Lifecycle::Constructed {
        control.status.set(Lifecycle::Ready);
    }
    if control.status.get() != Lifecycle::Closing {
        return Ok(Progress::Idle);
    }
    if let Some(error) = control.advance_error.take() {
        return Err(error);
    }
    if !control.can_stop.get() {
        return Ok(Progress::Idle);
    }
    control.status.set(Lifecycle::Closed);
    Ok(Progress::Stopped)
}

fn close(state: &mut State) -> Result<(), Error> {
    state.0.close_calls.set(state.0.close_calls.get() + 1);
    state.0.status.set(Lifecycle::Closing);
    state.0.close_error.take().map_or(Ok(()), Err)
}

fn setup() -> (Runtime, Connection, ResourceId, Rc<Control>) {
    let mut runtime = Runtime::new(161, Limits::default()).unwrap();
    runtime.start().unwrap();
    let control = Rc::new(Control {
        status: Cell::new(Lifecycle::Constructed),
        can_stop: Cell::new(false),
        close_calls: Cell::new(0),
        destroyed: Cell::new(false),
        close_error: Cell::new(None),
        advance_error: Cell::new(None),
    });
    let implementation = Module::new(
        State(Rc::clone(&control)),
        DriverOps {
            status: |s| s.0.status.get(),
            advance: |s, _| advance(s),
            close,
        },
        vec![Contribution::Resource {
            path: "service".into(),
            provider: memory::provider(b"live service"),
        }],
        Rc::new(()),
    );
    let instance = runtime
        .activate_module("module", implementation)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(instance, Permissions::ALL)]).unwrap(),
    );
    let local = client.resolve("module").into_inner().unwrap();
    run_with(module::ready(&client, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    (runtime, client, local, control)
}

fn drive_until(runtime: &mut Runtime, done: impl Fn() -> bool) {
    let mut wake = Context::from_waker(Waker::noop());
    for _ in 0..64 {
        if done() {
            return;
        }
        runtime.advance(&mut wake).unwrap();
    }
    assert!(done(), "expected transition within 64 turns");
}

fn unload_message() -> Message {
    Message::new(Contract::new("kernel.module.unload", 1), &[])
}

#[test]
fn polling_never_closes_and_concurrent_unload_joins_one_cleanup() {
    let (mut runtime, client, instance, control) = setup();
    let mut first = client.write(instance, &unload_message());
    let mut wake = Context::from_waker(Waker::noop());
    assert!(first.as_mut().poll(&mut wake).is_pending());
    assert_eq!(control.close_calls.get(), 0);
    drive_until(&mut runtime, || control.close_calls.get() == 1);
    assert!(!control.destroyed.get());
    assert_eq!(runtime.resource("service"), Err(Error::UNAVAILABLE));
    let second = client.write(instance, &unload_message());
    assert!(first.as_mut().poll(&mut wake).is_pending());
    control.can_stop.set(true);
    let result =
        run_with(first, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert_eq!(result, unload_message());
    assert_eq!(
        run_with(second, &mut runtime, &Runtime::DRIVER).unwrap().unwrap(),
        unload_message()
    );
    run_with(module::unload(&client, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(control.destroyed.get());
    assert_eq!(control.close_calls.get(), 1);
    runtime.close().unwrap();
}

#[test]
fn inspection_and_malformed_input_cannot_unload_a_live_instance() {
    let (mut runtime, owner, instance, control) = setup();
    let id = runtime.resource("module").unwrap();
    let observer = runtime.connect(&[(id, Permissions::READ)]).unwrap();
    let local = observer.resolve("module").into_inner().unwrap();
    assert_eq!(
        run_with(
            module::unload(&observer, local),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(
        run_with(
            owner.write(
                instance,
                &Message::new(Contract::new("kernel.module.unload", 1), b"bad")
            ),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::INVALID))
    );
    assert_eq!(control.close_calls.get(), 0);
    let service = runtime.resource("service").unwrap();
    let reader = runtime.connect(&[(service, Permissions::READ)]).unwrap();
    let local = reader.resolve("service").into_inner().unwrap();
    let value = run_with(
        reader.read(
            local,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(value.data.as_ref(), b"live service");
    control.can_stop.set(true);
    run_with(module::unload(&owner, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    runtime.close().unwrap();
}

#[test]
fn accepted_unload_survives_reconnect_and_cancellation() {
    let (mut runtime, client, instance, control) = setup();
    let waiting = client.write(instance, &unload_message());
    drive_until(&mut runtime, || control.close_calls.get() == 1);
    let next = client.reconnect().unwrap();
    let result = run_with(waiting, &mut runtime, &Runtime::DRIVER).unwrap();
    let Err(RequestError::Unknown { recovery, .. }) = result else {
        panic!("disconnected unload must retain its recovery locator");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    run_with(
        next.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert!(!control.destroyed.get());
    control.can_stop.set(true);
    let result =
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap()
            .unwrap();
    next.close(work).into_inner().unwrap();
    assert_eq!(result, unload_message());
    assert!(control.destroyed.get());
    assert_eq!(control.close_calls.get(), 1);
    runtime.close().unwrap();
}

#[test]
fn unload_can_join_cleanup_during_runtime_shutdown() {
    let (mut runtime, client, instance, control) = setup();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    assert_eq!(control.close_calls.get(), 1);
    let waiting = client.write(instance, &unload_message());
    control.can_stop.set(true);
    assert_eq!(
        run_with(waiting, &mut runtime, &Runtime::DRIVER).unwrap().unwrap(),
        unload_message()
    );
    assert!(control.destroyed.get());
    runtime.close().unwrap();
}

#[test]
fn cleanup_driver_failure_is_retained_after_destruction() {
    let (mut runtime, client, instance, control) = setup();
    control.advance_error.set(Some(Error::PROVIDER));
    control.can_stop.set(true);
    assert_eq!(
        run_with(
            module::unload(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    assert!(control.destroyed.get());
    assert_eq!(
        run_with(
            module::unload(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    assert_eq!(control.close_calls.get(), 1);
    runtime.close().unwrap();
}

#[test]
fn failed_activation_can_unload_without_erasing_its_original_error() {
    let (mut runtime, client, instance, control) = setup();
    control.status.set(Lifecycle::Failed(Error::INVALID));
    drive_until(&mut runtime, || control.close_calls.get() == 1);
    control.can_stop.set(true);
    run_with(module::unload(&client, instance), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    assert!(control.destroyed.get());
    assert_eq!(
        run_with(
            module::ready(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::INVALID))
    );
    runtime.close().unwrap();
}

#[test]
fn failed_close_hook_keeps_cleanup_owned_and_preserves_its_result() {
    let (mut runtime, client, instance, control) = setup();
    control.close_error.set(Some(Error::PROVIDER));
    assert_eq!(
        run_with(
            module::unload(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    assert!(!control.destroyed.get());
    assert_eq!(runtime.resource("service"), Err(Error::UNAVAILABLE));
    control.can_stop.set(true);
    assert_eq!(
        run_with(
            module::unload(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    assert!(control.destroyed.get());
    assert_eq!(control.close_calls.get(), 1);
    runtime.close().unwrap();
}

#[test]
fn cancelling_queued_unload_preserves_the_active_instance() {
    let (mut runtime, client, instance, control) = setup();
    let waiting = client.write(instance, &unload_message());
    let next = client.reconnect().unwrap();
    let result = run_with(waiting, &mut runtime, &Runtime::DRIVER).unwrap();
    let Err(RequestError::Unknown { recovery, .. }) = result else {
        panic!("unobserved request must remain recoverable");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    run_with(
        next.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        run_with(requests::result(&next, work), &mut runtime, &Runtime::DRIVER)
            .unwrap()
            .unwrap(),
        Err(RequestError::Failed(Error::CANCELLED))
    );
    next.close(work).into_inner().unwrap();
    assert_eq!(control.close_calls.get(), 0);
    let local = next.resolve("module").into_inner().unwrap();
    run_with(module::ready(&next, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    control.can_stop.set(true);
    run_with(module::unload(&next, local), &mut runtime, &Runtime::DRIVER)
        .unwrap()
        .unwrap();
    runtime.close().unwrap();
}
