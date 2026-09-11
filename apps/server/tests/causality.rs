use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::integer,
    provider::{
        Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime, diagnostics::Stage};
use std::{cell::Cell, rc::Rc};

struct Forward {
    kernel: Connection,
    counter: ResourceId,
}
fn forward(
    state: &mut Forward,
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    let request =
        Message::new(Contract::new("external.counter", 1), &input.data);
    let waiting = state.kernel.write(state.counter, &request);
    Box::pin(async move {
        let mut result = waiting.await.map_err(|error| match error {
            RequestError::Rejected(error) | RequestError::Failed(error) => {
                error
            }
            RequestError::Unknown { cause, .. } => cause,
        })?;
        result.contract = input.contract;
        Ok(result)
    })
}
fn unsupported(_: &mut Forward, _: ResourceId, _: Message) -> ProviderReply {
    Box::pin(core::future::ready(Err(Error::UNSUPPORTED)))
}
#[test]
fn nested_module_requests_keep_their_original_parent_across_execution() {
    let mut runtime = Runtime::new(102, Limits::default()).unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let counter = runtime
        .register("counter", reovim_module_counter::provider(Rc::clone(&value)))
        .map_err(|(error, _)| error)
        .unwrap();
    let module = reovim_sdk::local(
        runtime.connect(&[(counter, Permissions::ALL)]).unwrap(),
    );
    let id = module.resolve("counter").into_inner().unwrap();
    let provider = ResourceProvider::new(
        Forward { kernel: module, counter: id },
        ResourceOps { unload: None, read: unsupported, write: forward },
        Box::new([Surface {
            contract: Contract::new("forward", 1),
            methods: Permissions::WRITE,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    );
    let forward = runtime
        .register("forward", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(forward, Permissions::WRITE)]).unwrap(),
    );
    let id = client.resolve("forward").into_inner().unwrap();
    let result = run(
        client.write(
            id,
            &Message::new(Contract::new("forward", 1), &9_u64.to_le_bytes()),
        ),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(integer(&result.data), Ok(9));
    assert_eq!(value.get(), 9);
    let events = runtime.events(0);
    let root = events
        .iter()
        .find(|event| {
            event.subject == forward.0 && event.stage == Stage::Admitted
        })
        .unwrap()
        .request;
    let child = events
        .iter()
        .find(|event| {
            event.subject == counter.0 && event.stage == Stage::Admitted
        })
        .unwrap()
        .request;
    assert_ne!(root, child);
    for event in events {
        if event.request == root {
            assert_eq!(event.parent, None);
        }
        if event.request == child {
            assert_eq!(event.parent, Some(root));
        }
    }
    runtime.close().unwrap();
}
