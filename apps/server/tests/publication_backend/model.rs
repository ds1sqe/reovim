use super::*;
use reovim_kernel_interface::buffer::{ModelOps, ModelProvider, State};

pub fn register(runtime: &mut Runtime) -> ResourceId {
    let binding = runtime.bind(&[]).unwrap();
    let provider = ModelProvider::new(
        (),
        ModelOps {
            unload: None,
            initialize: |(), input| {
                Box::pin(async move {
                    Ok(State::new(input.data.as_ref() == [1], 1))
                })
            },
            prepare: |(), _, input| {
                Box::pin(async move {
                    Ok(State::new(input.data.as_ref() == [1], 1))
                })
            },
            observe: |(), state, input| {
                Ok(Message::new(
                    input.contract,
                    &[u8::from(*state.get::<bool>().unwrap())],
                ))
            },
            inspect: None,
        },
        Contract::new("logical.initial", 1),
        Contract::new("logical.value", 1),
        Contract::new("logical.set", 1),
        Rc::new(()),
    );
    runtime
        .register_bound(
            &binding,
            vec![Contribution::Model { path: "model".into(), provider }],
        )
        .map_err(|(error, _)| error)
        .unwrap()[0]
}
