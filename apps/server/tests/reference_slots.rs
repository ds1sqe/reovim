use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::integer,
    provider::{
        Contribution, Permissions, ProviderReply, ResourceOps,
        ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::Cell, future::Future, rc::Rc};

struct State {
    kernel: Connection,
}
fn read(state: &mut State, _: ResourceId, input: Message) -> ProviderReply {
    forward(state, input, false)
}
fn write(state: &mut State, _: ResourceId, input: Message) -> ProviderReply {
    forward(state, input, true)
}
fn forward(state: &State, input: Message, write: bool) -> ProviderReply {
    let kernel = state.kernel.clone();
    Box::pin(async move {
        let [source] = input.resources.as_ref() else {
            return Err(Error::INVALID);
        };
        let data = if write { 9_u64.to_le_bytes().to_vec() } else { vec![] };
        let query = Message::new(Contract::new("external.counter", 1), &data);
        let request = if write {
            kernel.write(*source, &query)
        } else {
            kernel.read(*source, &query)
        };
        let reply = request.await.map_err(|_| Error::PROVIDER)?;
        Ok(Message {
            contract: input.contract,
            data: reply.data,
            resources: input.resources,
        })
    })
}
fn surface(methods: Permissions) -> Surface {
    Surface {
        contract: Contract::new("slots", 1),
        methods,
        input: Box::new([methods]),
        output: Box::new([methods]),
    }
}
fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
#[test]
fn reference_grants_follow_the_selected_method_of_the_contract() {
    let mut runtime = Runtime::new(103, Limits::default()).unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let source = runtime
        .register("source", reovim_module_counter::provider(value.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let kernel = reovim_sdk::local(binding.connection());
    let provider = ResourceProvider::new(
        State { kernel: kernel.clone() },
        ResourceOps { unload: None, read, write },
        Box::new([surface(Permissions::READ), surface(Permissions::WRITE)]),
        Rc::new(()),
    );
    let target = runtime
        .register_bound(
            &binding,
            vec![Contribution::Resource { path: "target".into(), provider }],
        )
        .map_err(|(error, _)| error)
        .unwrap()[0];
    let client = reovim_sdk::local(
        runtime
            .connect(&[(target, Permissions::ALL), (source, Permissions::READ)])
            .unwrap(),
    );
    let reference = client.resolve("source").into_inner().unwrap();
    let resource = client.resolve("target").into_inner().unwrap();
    let input = Message::reference(Contract::new("slots", 1), reference);
    let pending = client.read(resource, &input);
    assert_eq!(runtime.unregister(source), Err(Error::BUSY));
    let observed = finish(&mut runtime, pending).unwrap();
    assert_eq!(integer(&observed.data), Ok(0));
    assert_eq!(observed.resources.as_ref(), [reference]);
    assert_eq!(
        kernel.resolve("source").into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
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
    assert_eq!(output.resources.as_ref(), [reference]);
    assert_eq!(value.get(), 9);
    runtime.close().unwrap();
}
