use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    buffer::{self, ClosePolicy, ModelOps, ModelProvider, State},
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{Contribution, Permissions},
    session,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::ready,
    rc::Rc,
    task::{Context, Waker},
};

fn model(read: Contract) -> ModelProvider {
    ModelProvider::new(
        read.clone(),
        ModelOps {
            unload: None,
            initialize: |_, _| Box::pin(ready(Ok(State::new(7_u8, 1)))),
            prepare: |_, state, _| Box::pin(ready(Ok(state.clone()))),
            observe: |read, state| {
                let value = *state.get::<u8>().unwrap();
                Ok(Message::new(read.clone(), &[value]))
            },
        },
        Contract::new("test.initial", 1),
        read,
        Contract::new("test.edit", 1),
        Rc::new(()),
    )
}

fn module(state: Rc<Cell<Lifecycle>>, contract: Contract) -> Module {
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
        vec![Contribution::Model {
            path: "schema".into(),
            provider: model(contract),
        }],
        Rc::new(()),
    )
}

#[test]
fn reading_a_session_close_named_schema_preserves_the_session() {
    let mut runtime = Runtime::new(184, Limits::default()).unwrap();
    runtime.start().unwrap();
    let contract = Contract::new("kernel.session.close", 1);
    let state = Rc::new(Cell::new(Lifecycle::Ready));
    runtime
        .activate_module("module", module(Rc::clone(&state), contract.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    let schema = runtime.resource("schema").unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(schema, Permissions::ALL)]).unwrap(),
    );
    let schema = client.resolve("schema").into_inner().unwrap();
    let observed = run_with(
        session::create(
            &client,
            schema,
            &Message::new(Contract::new("test.initial", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    for _ in 0..2 {
        let reply = run_with(
            client.read(observed.session, &Message::new(contract.clone(), &[])),
            &mut runtime,
            &Runtime::DRIVER,
        )
        .unwrap()
        .unwrap();
        let read = session::observation(reply).unwrap();
        assert_eq!(read.session, observed.session);
        assert_eq!(read.value.data.as_ref(), &[7]);
    }
    state.set(Lifecycle::Failed(Error::PROVIDER));
    runtime.advance(&mut wake).unwrap();
    assert_eq!(
        run_with(
            client.read(observed.session, &Message::new(contract, &[])),
            &mut runtime,
            &Runtime::DRIVER,
        )
        .unwrap(),
        Err(RequestError::Rejected(Error::PROVIDER))
    );
    run_with(
        session::close(&client, observed.session),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    runtime.close().unwrap();
}

#[test]
fn reading_a_buffer_close_named_schema_preserves_the_buffer() {
    let mut runtime = Runtime::new(185, Limits::default()).unwrap();
    runtime.start().unwrap();
    let contract = Contract::new("kernel.buffer.close", 1);
    let schema = runtime
        .register_model("schema", model(contract.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(schema, Permissions::ALL)]).unwrap(),
    );
    let schema = client.resolve("schema").into_inner().unwrap();
    let initial = run_with(
        client.write(
            schema,
            &Message::new(Contract::new("test.initial", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    let observed = buffer::observation(initial).unwrap();
    for _ in 0..2 {
        let reply = run_with(
            client.read(
                observed.state.buffer,
                &Message::new(contract.clone(), &[]),
            ),
            &mut runtime,
            &Runtime::DRIVER,
        )
        .unwrap()
        .unwrap();
        let read = buffer::observation(reply).unwrap();
        assert_eq!(read.state, observed.state);
        assert_eq!(read.value.data.as_ref(), &[7]);
    }
    run_with(
        buffer::close(&client, observed.state, ClosePolicy::DiscardLocal),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    runtime.close().unwrap();
}
