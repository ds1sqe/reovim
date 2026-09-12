use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    buffer::{self, ModelOps, ModelProvider, State},
    provider::{Permissions, ProviderReply},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::RefCell,
    future::{Future, poll_fn, ready},
    rc::Rc,
    task::{Context, Poll, Waker},
};

#[derive(Default)]
struct Gate {
    result: Option<Result<(), Error>>,
    wake: Option<Waker>,
}

fn inspect(
    gate: &mut Rc<RefCell<Gate>>,
    state: State,
    input: Message,
) -> ProviderReply {
    let gate = gate.clone();
    Box::pin(poll_fn(move |context| {
        let mut gate = gate.borrow_mut();
        let Some(result) = gate.result else {
            gate.wake = Some(context.waker().clone());
            return Poll::Pending;
        };
        Poll::Ready(result.map(|()| {
            Message::new(
                input.contract.clone(),
                state.get::<Box<[u8]>>().unwrap(),
            )
        }))
    }))
}

fn model(gate: Rc<RefCell<Gate>>) -> ModelProvider {
    ModelProvider::new(
        gate,
        ModelOps {
            unload: None,
            initialize: |_, input| {
                Box::pin(ready(Ok(State::new(input.data, 4))))
            },
            prepare: |_, _, input| {
                Box::pin(ready(Ok(State::new(input.data, 4))))
            },
            observe: |_, state, input| {
                Ok(Message::new(
                    input.contract,
                    state.get::<Box<[u8]>>().unwrap(),
                ))
            },
            inspect: Some(inspect),
        },
        Contract::new("create", 1),
        Contract::new("value", 1),
        Contract::new("edit", 1),
        Rc::new(()),
    )
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn open_gate(gate: &Rc<RefCell<Gate>>, result: Result<(), Error>) {
    let wake = {
        let mut gate = gate.borrow_mut();
        gate.result = Some(result);
        gate.wake.take()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}

fn create(runtime: &mut Runtime, client: &Connection) -> buffer::Observation {
    let model = client.resolve("model").into_inner().unwrap();
    buffer::observation(
        finish(
            runtime,
            client.write(
                model,
                &Message::new(Contract::new("create", 1), b"old"),
            ),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn pending_model_reads_preserve_state_and_do_not_block_acceptance() {
    let gate = Rc::new(RefCell::new(Gate::default()));
    let mut runtime = Runtime::new(208, Limits::default()).unwrap();
    runtime.start().unwrap();
    let model = runtime
        .register_model("model", model(gate.clone()))
        .map_err(|(e, _)| e)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(model, Permissions::ALL)]).unwrap(),
    );
    let initial = create(&mut runtime, &client);
    let snapshot = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, initial.state)).unwrap(),
    )
    .unwrap();
    let query = Message::new(Contract::new("value", 1), &[]);
    let mut pending = client.read(snapshot.id, &query);
    let mut context = Context::from_waker(Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert!(gate.borrow().wake.is_some());
    let request = buffer::edit(
        initial.state,
        Message::new(Contract::new("edit", 1), b"new"),
    );
    let changed = buffer::observation(
        finish(&mut runtime, client.write(initial.state.buffer, &request))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(changed.value.data.as_ref(), b"new");
    open_gate(&gate, Err(Error::UNAVAILABLE));
    assert_eq!(
        finish(&mut runtime, pending),
        Err(RequestError::Failed(Error::UNAVAILABLE))
    );
    assert_eq!(
        buffer::observation(
            finish(&mut runtime, client.read(initial.state.buffer, &query))
                .unwrap()
        )
        .unwrap()
        .state,
        changed.state
    );
    open_gate(&gate, Ok(()));
    let pending = client.read(snapshot.id, &query);
    finish(
        &mut runtime,
        buffer::close(
            &client,
            changed.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    client.close(snapshot.id).into_inner().unwrap();
    assert_eq!(runtime.unregister(model), Err(Error::BUSY));
    assert_eq!(finish(&mut runtime, pending).unwrap().data.as_ref(), b"old");
    client.close(snapshot.id).into_inner().unwrap();
    runtime.unregister(model).unwrap();
    runtime.close().unwrap();
}
