use super::*;
use reovim_kernel_interface::buffer::{ModelOps, ModelProvider, State};

pub fn register(runtime: &mut Runtime) -> ResourceId {
    install(runtime, false)
}

pub fn backed(runtime: &mut Runtime) -> ResourceId {
    install(runtime, true)
}

fn install(runtime: &mut Runtime, backed: bool) -> ResourceId {
    let binding = runtime.bind(&[]).unwrap();
    let mut provider = ModelProvider::new(
        (reovim_sdk::local(binding.connection()), backed),
        ModelOps {
            unload: None,
            initialize,
            prepare: |_, _, input| {
                Box::pin(async move {
                    Ok(State::new(input.data.as_ref() == [1], 1))
                })
            },
            observe: |_, state, input| {
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
    if backed {
        provider.initial_input = Box::new([Permissions::READ]);
    }
    runtime
        .register_bound(
            &binding,
            vec![Contribution::Model { path: "model".into(), provider }],
        )
        .map_err(|(error, _)| error)
        .unwrap()[0]
}

fn initialize(
    context: &mut (Connection, bool),
    input: Message,
) -> reovim_kernel_interface::buffer::StateReply {
    let (kernel, backed) = context.clone();
    Box::pin(async move {
        if backed {
            let value = kernel
                .read(
                    input.resources[0],
                    &Message::new(Contract::new("logical.value", 1), &[]),
                )
                .await
                .map_err(|error| code(&error))?;
            return Ok(State::new(value.data.as_ref() == [1], 1));
        }
        Ok(State::new(input.data.as_ref() == [1], 1))
    })
}
