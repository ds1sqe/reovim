use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

fn module(
    state: Rc<Cell<Lifecycle>>,
    contributions: Vec<Contribution>,
) -> Module {
    Module::new(
        state,
        DriverOps {
            status: |state| state.get(),
            advance: |state, _| {
                Ok(if state.get() == Lifecycle::Closed {
                    Progress::Stopped
                } else {
                    Progress::Idle
                })
            },
            close: |state| {
                state.set(Lifecycle::Closed);
                Ok(())
            },
        },
        contributions,
        Rc::new(()),
    )
}

#[test]
fn readiness_regression_fences_new_and_queued_provider_calls() {
    let mut runtime = Runtime::new(125, Limits::default()).unwrap();
    runtime.start().unwrap();
    let state = Rc::new(Cell::new(Lifecycle::Ready));
    let value = Rc::new(Cell::new(0));
    runtime
        .activate_module(
            "modules/counter",
            module(
                Rc::clone(&state),
                vec![Contribution::Resource {
                    path: "counter".into(),
                    provider: reovim_module_counter::provider(Rc::clone(
                        &value,
                    )),
                }],
            ),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    let owner = runtime.resource("counter").unwrap();
    let client = runtime.connect(&[(owner, Permissions::ALL)]).unwrap();
    let subject = client.resolve("counter").into_inner().unwrap();
    let input = Message::new(
        Contract::new("external.counter", 1),
        &13_u64.to_le_bytes(),
    );
    let queued = client.write(subject, &input);
    state.set(Lifecycle::Starting);
    runtime.advance(&mut wake).unwrap();
    assert_eq!(value.get(), 0);
    assert_eq!(
        run_with(client.write(subject, &input), &mut runtime, &Runtime::DRIVER)
            .unwrap(),
        Err(RequestError::Rejected(Error::PROVIDER))
    );
    assert_eq!(
        run_with(queued, &mut runtime, &Runtime::DRIVER).unwrap(),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    assert_eq!(value.get(), 0);
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    runtime.close().unwrap();
}

#[test]
fn failed_model_cannot_edit_but_its_buffer_can_be_closed() {
    use reovim_kernel_interface::buffer::{self, ClosePolicy};
    let mut runtime = Runtime::new(126, Limits::default()).unwrap();
    runtime.start().unwrap();
    let state = Rc::new(Cell::new(Lifecycle::Ready));
    runtime
        .activate_module(
            "modules/model",
            module(
                Rc::clone(&state),
                vec![Contribution::Model {
                    path: "heading".into(),
                    provider: reovim_representation_basic::heading(),
                }],
            ),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    let owner = runtime.resource("heading").unwrap();
    let client = runtime.connect(&[(owner, Permissions::ALL)]).unwrap();
    let subject = client.resolve("heading").into_inner().unwrap();
    let reply = run_with(
        client.write(
            subject,
            &Message::new(Contract::new("heading.initial", 1), b"# retained"),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    let before = buffer::observation(reply).unwrap();
    state.set(Lifecycle::Failed(Error::PROVIDER));
    runtime.advance(&mut wake).unwrap();
    let edit = buffer::edit(
        before.state,
        Message::new(Contract::new("heading.replace", 1), b"changed"),
    );
    assert_eq!(
        run_with(
            client.write(before.state.buffer, &edit),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::PROVIDER))
    );
    run_with(
        buffer::close(&client, before.state, ClosePolicy::DiscardLocal),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    runtime.close().unwrap();
}
