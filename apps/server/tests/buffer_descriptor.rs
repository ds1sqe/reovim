use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    buffer::{self, ModelOps, ModelProvider, State},
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
fn model(unavailable: Rc<Cell<bool>>) -> ModelProvider {
    ModelProvider::new(
        unavailable,
        ModelOps {
            unload: None,
            inspect: None,
            initialize: |_, _| Box::pin(ready(Ok(State::new(false, 1)))),
            prepare: |_, _, _| Box::pin(ready(Ok(State::new(true, 1)))),
            observe: |unavailable, state, input| {
                if unavailable.get() {
                    return Err(Error::UNAVAILABLE);
                }
                Ok(Message::new(
                    input.contract,
                    &[u8::from(*state.get::<bool>().unwrap())],
                ))
            },
        },
        Contract::new("create", 1),
        Contract::new("logical", 1),
        Contract::new("change", 1),
        Rc::new(()),
    )
}

#[test]
fn descriptor_is_coherent_model_independent_and_does_not_grant_mutation() {
    let mut runtime = Runtime::new(212, Limits::default()).unwrap();
    runtime.start().unwrap();
    let unavailable = Rc::new(Cell::new(false));
    let provider = runtime
        .register_model("model", model(unavailable.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let owner = reovim_sdk::local(
        runtime.connect(&[(provider, Permissions::ALL)]).unwrap(),
    );
    let model = owner.resolve("model").into_inner().unwrap();
    let opened = buffer::observation(
        finish(
            &mut runtime,
            owner.write(model, &Message::new(Contract::new("create", 1), &[])),
        )
        .unwrap(),
    )
    .unwrap();
    unavailable.set(true);
    let metadata = buffer::descriptor(
        &finish(&mut runtime, buffer::describe(&owner, opened.state.buffer))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata.state, opened.state);
    assert_eq!(metadata.history, opened.history);
    assert_eq!(metadata.backing, None);
    assert_eq!(
        finish(&mut runtime, buffer::describe(&owner, model)),
        Err(RequestError::Rejected(Error::UNSUPPORTED))
    );
    assert_eq!(
        finish(
            &mut runtime,
            owner.read(
                opened.state.buffer,
                &Message::new(Contract::new("kernel.buffer.describe", 1), &[1])
            )
        ),
        Err(RequestError::Failed(Error::INVALID))
    );
    assert_eq!(
        finish(
            &mut runtime,
            owner.read(
                opened.state.buffer,
                &Message::new(Contract::new("logical", 1), &[])
            )
        ),
        Err(RequestError::Failed(Error::UNAVAILABLE))
    );
    unavailable.set(false);
    let changed = buffer::observation(
        finish(
            &mut runtime,
            owner.write(
                opened.state.buffer,
                &buffer::edit(
                    opened.state,
                    Message::new(Contract::new("change", 1), &[]),
                ),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let updated = buffer::descriptor(
        &finish(&mut runtime, buffer::describe(&owner, opened.state.buffer))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(updated.state, changed.state);
    assert_eq!(updated.history, opened.history);
    assert_eq!(updated.backing, None);
    readonly(&mut runtime, updated.state.revision);
    finish(
        &mut runtime,
        buffer::close(&owner, updated.state, buffer::ClosePolicy::DiscardLocal),
    )
    .unwrap();
    assert_eq!(
        finish(&mut runtime, buffer::describe(&owner, updated.state.buffer)),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    runtime.unregister(provider).unwrap();
    runtime.close().unwrap();
}

fn readonly(runtime: &mut Runtime, revision: u64) {
    let root = runtime.resource("buffers").unwrap();
    let reader = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::READ)]).unwrap(),
    );
    let root = reader.resolve("buffers").into_inner().unwrap();
    let entry =
        reader.children(root).into_inner().unwrap().into_iter().next().unwrap();
    let metadata = buffer::descriptor(
        &finish(runtime, buffer::describe(&reader, entry.resource)).unwrap(),
    )
    .unwrap();
    assert_eq!(metadata.state.buffer, entry.resource);
    assert_eq!(metadata.state.revision, revision);
    let undo = buffer::edit(
        metadata.state,
        Message::new(Contract::new("kernel.history.undo", 1), &[]),
    );
    assert_eq!(
        finish(runtime, reader.write(metadata.history, &undo)),
        Err(RequestError::Rejected(Error::DENIED))
    );
}
