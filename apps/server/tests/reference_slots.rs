use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError, ResourceId,
    buffer::integer,
    provider::{
        Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::{Future, ready},
    rc::Rc,
};

struct State {
    reference: ResourceId,
    value: Rc<Cell<u64>>,
}
fn read(state: &mut State, _: ResourceId, input: Message) -> ProviderReply {
    Box::pin(ready(reply(state, input)))
}
fn write(state: &mut State, _: ResourceId, input: Message) -> ProviderReply {
    state.value.set(9);
    Box::pin(ready(reply(state, input)))
}
fn reply(state: &State, input: Message) -> Result<Message, Error> {
    if input.resources.as_ref() != [state.reference] {
        return Err(Error::INVALID);
    }
    Ok(Message::new(input.contract, &state.value.get().to_le_bytes()))
}
fn surface(methods: Permissions) -> Surface {
    Surface {
        contract: Contract::new("slots", 1),
        methods,
        input: Box::new([methods]),
        output: Box::new([]),
    }
}
fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
#[test]
fn reference_grants_follow_the_selected_method_of_the_contract() {
    let mut runtime =
        Runtime::new(103, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register(
            "source",
            reovim_kernel_interface_public_consumer::counter(Rc::new(
                Cell::new(0),
            )),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let value = Rc::new(Cell::new(0));
    let provider = ResourceProvider::new(
        State { reference: source, value: Rc::clone(&value) },
        ResourceOps { unload: None, read, write },
        Box::new([surface(Permissions::READ), surface(Permissions::WRITE)]),
        Rc::new(()),
    );
    let target = runtime
        .register("target", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[(target, Permissions::ALL), (source, Permissions::READ)])
            .unwrap(),
    );
    let reference = client.resolve("source").into_inner().unwrap();
    let resource = client.resolve("target").into_inner().unwrap();
    let input = Message::reference(Contract::new("slots", 1), reference);
    let observed = finish(&mut runtime, client.read(resource, &input)).unwrap();
    assert_eq!(integer(&observed.data), Ok(0));
    assert_eq!(
        finish(&mut runtime, client.write(resource, &input)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(value.get(), 0);
    let writer = reovim_sdk::local(
        runtime
            .connect(&[(target, Permissions::ALL), (source, Permissions::ALL)])
            .unwrap(),
    );
    let reference = writer.resolve("source").into_inner().unwrap();
    let resource = writer.resolve("target").into_inner().unwrap();
    let output = finish(
        &mut runtime,
        writer.write(
            resource,
            &Message::reference(Contract::new("slots", 1), reference),
        ),
    )
    .unwrap();
    assert_eq!(integer(&output.data), Ok(9));
    assert_eq!(value.get(), 9);
    runtime.close().unwrap();
}
