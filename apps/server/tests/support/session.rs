use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Message, RequestError, ResourceId,
    buffer::{ModelOps, ModelProvider, State},
    provider::Permissions,
    session::{self, Observation},
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{
    future::{Future, ready},
    rc::Rc,
};

pub fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

pub fn setup() -> (Runtime, Connection, Observation) {
    setup_with(Limits::default())
}

pub fn setup_with(limits: Limits) -> (Runtime, Connection, Observation) {
    let (runtime, binding, initial) = setup_binding(limits);
    (runtime, reovim_sdk::local(binding.connection()), initial)
}

pub fn setup_binding(limits: Limits) -> (Runtime, Binding, Observation) {
    let mut runtime = Runtime::new(232, limits).unwrap();
    runtime.start().unwrap();
    let model = ModelProvider::new(
        (),
        ModelOps {
            inspect: None,
            unload: None,
            initialize: |(), input| {
                Box::pin(ready(Ok(State::new(input.data, 1))))
            },
            prepare: |(), _, input| {
                Box::pin(ready(Ok(State::new(input.data, 1))))
            },
            observe: |(), state, input| {
                Ok(Message::new(
                    input.contract,
                    state.get::<Box<[u8]>>().unwrap(),
                ))
            },
        },
        Contract::new("initial", 1),
        Contract::new("focus", 1),
        Contract::new("set", 1),
        Rc::new(()),
    );
    let model = runtime
        .register_model("model", model)
        .map_err(|(error, _)| error)
        .unwrap();
    let binding = runtime.bind(&[(model, Permissions::ALL)]).unwrap();
    let client = reovim_sdk::local(binding.connection());
    let model = client.resolve("model").into_inner().unwrap();
    let initial = finish(
        &mut runtime,
        session::create(
            &client,
            model,
            &Message::new(Contract::new("initial", 1), &[1]),
        ),
    )
    .unwrap();
    (runtime, binding, initial)
}

pub fn attach(
    runtime: &mut Runtime,
    client: &Connection,
    session: ResourceId,
) -> ResourceId {
    session::attached(
        &finish(runtime, session::attach(client, session)).unwrap(),
    )
    .unwrap()
}

pub fn read(
    runtime: &mut Runtime,
    client: &Connection,
    subject: ResourceId,
) -> Result<Message, RequestError> {
    finish(
        runtime,
        client.read(subject, &Message::new(Contract::new("focus", 1), &[])),
    )
}

pub fn update(
    runtime: &mut Runtime,
    client: &Connection,
    observed: &Observation,
    value: u8,
) -> Observation {
    let input = session::update(
        observed,
        Message::new(Contract::new("set", 1), &[value]),
    )
    .unwrap();
    session::observation(
        finish(runtime, client.write(observed.session, &input)).unwrap(),
    )
    .unwrap()
}
