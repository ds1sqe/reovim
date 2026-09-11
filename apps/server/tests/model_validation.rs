use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    buffer::{ModelOps, ModelProvider, State, StateReply, edit, observation},
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::{Future, ready},
    rc::Rc,
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
fn initialize(_: &mut Rc<Cell<u8>>, input: Message) -> StateReply {
    Box::pin(ready(Ok(State::new(input.data, 1))))
}
fn prepare(
    context: &mut Rc<Cell<u8>>,
    _: &State,
    input: Message,
) -> StateReply {
    initialize(context, input)
}
fn observe(mode: &mut Rc<Cell<u8>>, state: &State) -> Result<Message, Error> {
    let data = state.get::<Box<[u8]>>().unwrap();
    match mode.get() {
        1 => Err(Error::PROVIDER),
        2 => Ok(Message::new(Contract::new("wrong", 1), data)),
        3 => Ok(Message::new(Contract::new("value", 1), &[0; 64])),
        _ => Ok(Message::new(Contract::new("value", 1), data)),
    }
}
fn provider(mode: Rc<Cell<u8>>) -> ModelProvider {
    ModelProvider::new(
        mode,
        ModelOps { unload: None, initialize, prepare, observe },
        Contract::new("create", 1),
        Contract::new("value", 1),
        Contract::new("edit", 1),
        Rc::new(()),
    )
}
#[test]
fn failed_candidate_observation_preserves_state_and_history() {
    let mode = Rc::new(Cell::new(0));
    let limits = Limits { message_bytes: 48, ..Limits::default() };
    let mut runtime = Runtime::new(90, limits).unwrap();
    runtime.start().unwrap();
    let id = runtime
        .register_model("model", provider(Rc::clone(&mode)))
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let model = finish(&mut runtime, client.resolve("model")).unwrap();
    let initial = finish(
        &mut runtime,
        client
            .write(model, &Message::new(Contract::new("create", 1), b"before")),
    )
    .unwrap();
    let initial = observation(initial).unwrap();
    for fault in [1, 2, 3] {
        mode.set(fault);
        let request = edit(
            initial.state,
            Message::new(Contract::new("edit", 1), b"after"),
        );
        assert_eq!(
            finish(&mut runtime, client.write(initial.state.buffer, &request)),
            Err(RequestError::Failed(Error::PROVIDER))
        );
        assert_eq!(
            finish(
                &mut runtime,
                client.read(
                    initial.state.buffer,
                    &Message::new(Contract::new("value", 1), &[])
                )
            ),
            Err(RequestError::Failed(Error::PROVIDER))
        );
        mode.set(0);
        let actual = finish(
            &mut runtime,
            client.read(
                initial.state.buffer,
                &Message::new(Contract::new("value", 1), &[]),
            ),
        )
        .unwrap();
        assert_eq!(observation(actual).unwrap(), initial);
    }
    let request =
        edit(initial.state, Message::new(Contract::new("edit", 1), b"after"));
    let committed =
        finish(&mut runtime, client.write(initial.state.buffer, &request))
            .unwrap();
    let committed = observation(committed).unwrap();
    assert_eq!(committed.value.data.as_ref(), b"after");
    assert!(committed.state.revision > initial.state.revision);
    assert_ne!(committed.node, initial.node);
    runtime.close().unwrap();
}
#[test]
fn failed_initial_observation_does_not_publish_or_pin_a_buffer() {
    let mode = Rc::new(Cell::new(1));
    let mut runtime = Runtime::new(91, Limits::default()).unwrap();
    runtime.start().unwrap();
    let id = runtime
        .register_model("model", provider(mode))
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let model = finish(&mut runtime, client.resolve("model")).unwrap();
    assert_eq!(
        finish(
            &mut runtime,
            client.write(
                model,
                &Message::new(Contract::new("create", 1), b"before")
            )
        ),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    // A published Buffer would retain the model and make unregister busy.
    runtime.unregister(id).unwrap();
    runtime.close().unwrap();
}
