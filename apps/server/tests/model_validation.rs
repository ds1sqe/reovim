use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    buffer::{
        ModelOps, ModelProvider, State, StateReply, edit, observation, retain,
        retained,
    },
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
fn observe(
    mode: &mut Rc<Cell<u8>>,
    state: &State,
    _: Message,
) -> Result<Message, Error> {
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
        ModelOps { unload: None, initialize, prepare, observe, inspect: None },
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

fn window_model() -> ModelProvider {
    ModelProvider::new(
        (),
        ModelOps {
            inspect: None,
            unload: None,
            initialize: |(), _| {
                let mut bytes = vec![0; 4096];
                bytes[4090..].copy_from_slice(b"\xffend\r\n");
                Box::pin(ready(Ok(State::new(bytes, 4096))))
            },
            prepare: |(), _, _| Box::pin(ready(Err(Error::UNSUPPORTED))),
            observe: |(), state, input| {
                let bytes = state.get::<Vec<u8>>().unwrap();
                if input.data.is_empty() {
                    return Ok(Message::new(
                        input.contract,
                        &(bytes.len() as u64).to_le_bytes(),
                    ));
                }
                let [a, b, c, d] = *input.data else {
                    return Err(Error::INVALID);
                };
                let start = usize::from(u16::from_le_bytes([a, b]));
                let end = usize::from(u16::from_le_bytes([c, d]));
                let window = bytes.get(start..end).ok_or(Error::INVALID)?;
                Ok(Message::new(input.contract, window))
            },
        },
        Contract::new("window.create", 1),
        Contract::new("window.read", 1),
        Contract::new("window.edit", 1),
        Rc::new(()),
    )
}

#[test]
fn bounded_retained_reads_keep_admitted_work_after_access_release() {
    let limits = Limits { message_bytes: 48, ..Limits::default() };
    let mut runtime = Runtime::new(93, limits).unwrap();
    runtime.start().unwrap();
    let id = runtime
        .register_model("window", window_model())
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let model = client.resolve("window").into_inner().unwrap();
    let initial = observation(
        finish(
            &mut runtime,
            client.write(
                model,
                &Message::new(Contract::new("window.create", 1), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(initial.value.data.as_ref(), &4096_u64.to_le_bytes());
    let snapshot = retained(
        &finish(&mut runtime, retain(&client, initial.state)).unwrap(),
    )
    .unwrap();
    let window = Message::new(
        Contract::new("window.read", 1),
        &[4090_u16.to_le_bytes(), 4096_u16.to_le_bytes()].concat(),
    );
    assert_eq!(
        finish(&mut runtime, client.write(snapshot.id, &window)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let too_large = Message::new(
        window.contract.clone(),
        &[0_u16.to_le_bytes(), 4096_u16.to_le_bytes()].concat(),
    );
    assert_eq!(
        finish(&mut runtime, client.read(snapshot.id, &too_large)),
        Err(RequestError::Failed(Error::PROVIDER))
    );
    let admitted = client.read(snapshot.id, &window);
    client.close(snapshot.id).into_inner().unwrap();
    let bytes = finish(&mut runtime, admitted).unwrap();
    assert_eq!(bytes.data.as_ref(), b"\xffend\r\n");
    assert_eq!(
        finish(&mut runtime, client.read(snapshot.id, &window)),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    let next = retained(
        &finish(&mut runtime, retain(&client, initial.state)).unwrap(),
    )
    .unwrap();
    assert_ne!(next.id, snapshot.id);
    assert_eq!(
        finish(&mut runtime, client.read(next.id, &window))
            .unwrap()
            .data
            .as_ref(),
        b"\xffend\r\n"
    );
    runtime.close().unwrap();
}
